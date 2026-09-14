use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::handlers::{get_current_user_id, internal_error_response, ApiResponse};

fn fail(fail_code: &str) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse {
        success: false,
        fail_code: Some(fail_code.to_string()),
    })
}

/// base_url 为供应商根地址（genai 按适配器拼接协议路径），为空时使用预设默认地址
fn is_valid_base_url(base_url: &str) -> bool {
    base_url.starts_with("http://") || base_url.starts_with("https://")
}

/// 代理仅支持 http proxy；为空表示直连。
fn is_valid_proxy(proxy: &str) -> bool {
    proxy.is_empty() || proxy.starts_with("http://")
}

fn valid_bool(value: Option<bool>, default: bool) -> bool {
    value.unwrap_or(default)
}

fn valid_len(value: Option<i64>, default: i64) -> i64 {
    value.unwrap_or(default).max(0)
}

#[derive(Debug, Serialize)]
pub struct AiModelItem {
    pub id: String,
    pub model_id: String,
    pub supports_vision: bool,
    pub context_length: i64,
    pub max_output_tokens: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct AiProviderItem {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub proxy: String,
    pub has_api_key: bool,
    pub created_at: String,
    pub updated_at: String,
    pub models: Vec<AiModelItem>,
}

#[derive(Debug, Serialize)]
pub struct ListAiProvidersResponse {
    pub success: bool,
    pub providers: Vec<AiProviderItem>,
}

pub async fn list_ai_providers(
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    if let Err(resp) = get_current_user_id(&http_req, &app_state) {
        return resp;
    }

    let providers = match app_state.ai_provider_model.list() {
        Ok(list) => list,
        Err(e) => return internal_error_response("/api/ai/provider/list", &e),
    };

    let mut items = Vec::with_capacity(providers.len());
    for p in providers {
        let models = match app_state.ai_model_model.list_by_provider(&p.id) {
            Ok(list) => list,
            Err(e) => return internal_error_response("/api/ai/provider/list", &e),
        };
        let model_items: Vec<AiModelItem> = models
            .into_iter()
            .map(|m| AiModelItem {
                id: m.id,
                model_id: m.model_id,
                supports_vision: m.supports_vision,
                context_length: m.context_length,
                max_output_tokens: m.max_output_tokens,
                created_at: m.created_at,
                updated_at: m.updated_at,
            })
            .collect();
        items.push(AiProviderItem {
            id: p.id,
            name: p.name,
            provider_type: p.provider_type,
            base_url: p.base_url,
            proxy: p.proxy,
            has_api_key: !p.api_key.is_empty(),
            created_at: p.created_at,
            updated_at: p.updated_at,
            models: model_items,
        });
    }

    HttpResponse::Ok().json(ListAiProvidersResponse {
        success: true,
        providers: items,
    })
}

#[derive(Debug, Serialize)]
pub struct ProviderPresetItem {
    pub type_id: &'static str,
    pub name: &'static str,
    pub default_base_url: &'static str,
    pub requires_api_key: bool,
}

#[derive(Debug, Serialize)]
pub struct ListProviderPresetsResponse {
    pub success: bool,
    pub presets: Vec<ProviderPresetItem>,
}

pub async fn list_provider_presets(
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    if let Err(resp) = get_current_user_id(&http_req, &app_state) {
        return resp;
    }
    let presets = crate::ai::PROVIDER_PRESETS
        .iter()
        .map(|p| ProviderPresetItem {
            type_id: p.type_id,
            name: p.name,
            default_base_url: p.default_base_url,
            requires_api_key: p.requires_api_key,
        })
        .collect();
    HttpResponse::Ok().json(ListProviderPresetsResponse {
        success: true,
        presets,
    })
}

#[derive(Debug, Deserialize)]
pub struct CreateAiProviderRequest {
    pub name: String,
    pub provider_type: String,
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key: String,
    pub proxy: Option<String>,
    pub models: Option<Vec<NewAiModelData>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NewAiModelData {
    pub model_id: String,
    pub supports_vision: Option<bool>,
    pub context_length: Option<i64>,
    pub max_output_tokens: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct CreateAiProviderResponse {
    pub success: bool,
    pub provider_id: String,
}

pub async fn create_ai_provider(
    body: web::Json<CreateAiProviderRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    if let Err(resp) = get_current_user_id(&http_req, &app_state) {
        return resp;
    }

    let name = body.name.trim().to_string();
    let provider_type = body.provider_type.trim().to_string();
    let api_key = body.api_key.trim().to_string();
    let proxy = body.proxy.as_deref().unwrap_or("").trim().to_string();

    let preset = match crate::ai::preset_by_type(&provider_type) {
        Some(p) => p,
        None => return fail("PROVIDER_TYPE_INVALID"),
    };
    let base_url = body
        .base_url
        .as_deref()
        .unwrap_or("")
        .trim()
        .trim_end_matches('/')
        .to_string();
    let base_url = if base_url.is_empty() {
        preset.default_base_url.trim_end_matches('/').to_string()
    } else {
        base_url
    };

    if name.is_empty() {
        return fail("PARAM_INVALID");
    }
    if preset.requires_api_key && api_key.is_empty() {
        return fail("PARAM_INVALID");
    }
    if !is_valid_base_url(&base_url) {
        return fail("BASE_URL_INVALID");
    }
    if !is_valid_proxy(&proxy) {
        return fail("PROXY_INVALID");
    }

    let new_models: Vec<crate::models::NewAiModelData> = body
        .models
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|m| crate::models::NewAiModelData {
            model_id: m.model_id,
            supports_vision: valid_bool(m.supports_vision, false),
            context_length: valid_len(m.context_length, 200000),
            max_output_tokens: valid_len(m.max_output_tokens, 65536),
        })
        .collect();

    match app_state
        .ai_provider_model
        .create_with_models(&name, &provider_type, &base_url, &api_key, &proxy, &new_models)
    {
        Ok(provider_id) => HttpResponse::Ok().json(CreateAiProviderResponse {
            success: true,
            provider_id,
        }),
        Err(e) if e == "MODEL_DUPLICATE" => fail("MODEL_DUPLICATE"),
        Err(e) => internal_error_response("/api/ai/provider/create", &e),
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateAiProviderRequest {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub proxy: Option<String>,
    pub models: Option<Vec<UpdateModelDataReq>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct UpdateModelDataReq {
    pub id: Option<String>,
    pub model_id: String,
    pub supports_vision: Option<bool>,
    pub context_length: Option<i64>,
    pub max_output_tokens: Option<i64>,
}

pub async fn update_ai_provider(
    body: web::Json<UpdateAiProviderRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    if let Err(resp) = get_current_user_id(&http_req, &app_state) {
        return resp;
    }

    let name = body.name.trim().to_string();
    let provider_type = body.provider_type.trim().to_string();
    let proxy = body.proxy.as_deref().unwrap_or("").trim().to_string();

    let preset = match crate::ai::preset_by_type(&provider_type) {
        Some(p) => p,
        None => return fail("PROVIDER_TYPE_INVALID"),
    };
    let base_url = body
        .base_url
        .as_deref()
        .unwrap_or("")
        .trim()
        .trim_end_matches('/')
        .to_string();
    let base_url = if base_url.is_empty() {
        preset.default_base_url.trim_end_matches('/').to_string()
    } else {
        base_url
    };

    if name.is_empty() {
        return fail("PARAM_INVALID");
    }
    if !is_valid_base_url(&base_url) {
        return fail("BASE_URL_INVALID");
    }
    if !is_valid_proxy(&proxy) {
        return fail("PROXY_INVALID");
    }

    match app_state
        .ai_provider_model
        .get_by_id(&body.id)
    {
        Ok(Some(_)) => {}
        Ok(None) => return fail("PROVIDER_NOT_FOUND"),
        Err(e) => return internal_error_response("/api/ai/provider/update", &e),
    }

    let api_key = body.api_key.as_deref().map(|k| k.trim().to_string());

    if let Some(models) = &body.models {
        let new_models: Vec<crate::models::UpdateModelData> = models
            .iter()
            .map(|m| crate::models::UpdateModelData {
                id: m.id.clone(),
                model_id: m.model_id.clone(),
                supports_vision: valid_bool(m.supports_vision, false),
                context_length: valid_len(m.context_length, 200000),
                max_output_tokens: valid_len(m.max_output_tokens, 65536),
            })
            .collect();
        match app_state.ai_provider_model.update_with_models(
            &body.id,
            &name,
            &provider_type,
            &base_url,
            &proxy,
            api_key.as_deref(),
            &new_models,
        ) {
            Ok(()) => HttpResponse::Ok().json(ApiResponse {
                success: true,
                fail_code: None,
            }),
            Err(e) if e == "PROVIDER_NOT_FOUND" => fail("PROVIDER_NOT_FOUND"),
            Err(e) if e == "MODEL_DUPLICATE" => fail("MODEL_DUPLICATE"),
            Err(e) if e == "MODEL_NOT_FOUND" => fail("MODEL_NOT_FOUND"),
            Err(e) => internal_error_response("/api/ai/provider/update", &e),
        }
    } else {
        match app_state
            .ai_provider_model
            .update(&body.id, &name, &provider_type, &base_url, &proxy, api_key.as_deref())
        {
            Ok(()) => HttpResponse::Ok().json(ApiResponse {
                success: true,
                fail_code: None,
            }),
            Err(e) => internal_error_response("/api/ai/provider/update", &e),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct DeleteAiProviderRequest {
    pub id: String,
}

pub async fn delete_ai_provider(
    body: web::Json<DeleteAiProviderRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    if let Err(resp) = get_current_user_id(&http_req, &app_state) {
        return resp;
    }

    match app_state.ai_provider_model.delete(&body.id) {
        Ok(true) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            fail_code: None,
        }),
        Ok(false) => fail("PROVIDER_NOT_FOUND"),
        Err(e) => internal_error_response("/api/ai/provider/delete", &e),
    }
}

#[derive(Debug, Deserialize)]
pub struct FetchAiModelsRequest {
    pub provider_type: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub proxy: Option<String>,
    pub provider_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FetchAiModelsResponse {
    pub success: bool,
    pub models: Vec<String>,
}

/// 各供应商模型列表接口差异较大，按适配器风格分派
fn normalize_preset_base(preset_type: &str, base_url: &str) -> String {
    // 兼容旧配置：剥掉遗留的 /responses 后缀
    let base_url = base_url.trim().trim_end_matches('/').trim_end_matches("/responses");
    match crate::ai::preset_by_type(preset_type) {
        Some(p) if base_url.is_empty() => p.default_base_url.to_string(),
        _ => format!("{}/", base_url),
    }
}

pub async fn fetch_ai_models(
    body: web::Json<FetchAiModelsRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    if let Err(resp) = get_current_user_id(&http_req, &app_state) {
        return resp;
    }

    let provider_type = body.provider_type.trim().to_string();
    if crate::ai::preset_by_type(&provider_type).is_none() {
        return fail("PROVIDER_TYPE_INVALID");
    }

    // api_key / proxy / base_url 为空且提供了 provider_id 时，回退使用已存储的值（编辑供应商时用户可能未改）
    let mut api_key = body.api_key.as_deref().unwrap_or("").trim().to_string();
    let mut proxy = body.proxy.as_deref().unwrap_or("").trim().to_string();
    let mut base_url = body.base_url.as_deref().unwrap_or("").trim().to_string();
    if (api_key.is_empty() || proxy.is_empty() || base_url.is_empty()) && body.provider_id.is_some() {
        if let Some(pid) = &body.provider_id {
            match app_state.ai_provider_model.get_by_id(pid) {
                Ok(Some(p)) => {
                    if api_key.is_empty() {
                        api_key = p.api_key;
                    }
                    if proxy.is_empty() {
                        proxy = p.proxy;
                    }
                    if base_url.is_empty() {
                        base_url = p.base_url;
                    }
                }
                Ok(None) => return fail("PROVIDER_NOT_FOUND"),
                Err(e) => {
                    return internal_error_response("/api/ai/provider/fetch_models", &e)
                }
            }
        }
    }
    if !is_valid_proxy(&proxy) {
        return fail("PROXY_INVALID");
    }

    let base = normalize_preset_base(&provider_type, &base_url);
    if !base.starts_with("http://") && !base.starts_with("https://") {
        return fail("BASE_URL_INVALID");
    }
    let client = match crate::ai::client::build_http_client(&proxy) {
        Ok(c) => c,
        Err(_) => return fail("PROXY_INVALID"),
    };

    let models = match provider_type.as_str() {
        "anthropic" => {
            if api_key.is_empty() {
                return fail("PARAM_INVALID");
            }
            let resp = match client
                .get(format!("{base}models"))
                .header("x-api-key", api_key.as_str())
                .header("anthropic-version", "2023-06-01")
                .send()
                .await
            {
                Ok(r) => r,
                Err(_) => return fail("AI_MODEL_LIST_FAILED"),
            };
            if !resp.status().is_success() {
                return fail("AI_MODEL_LIST_FAILED");
            }
            let json: serde_json::Value = match resp.json().await {
                Ok(j) => j,
                Err(_) => return fail("AI_MODEL_LIST_FAILED"),
            };
            json["data"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default()
        }
        "gemini" => {
            if api_key.is_empty() {
                return fail("PARAM_INVALID");
            }
            let resp = match client
                .get(format!("{base}models"))
                .header("x-goog-api-key", api_key.as_str())
                .send()
                .await
            {
                Ok(r) => r,
                Err(_) => return fail("AI_MODEL_LIST_FAILED"),
            };
            if !resp.status().is_success() {
                return fail("AI_MODEL_LIST_FAILED");
            }
            let json: serde_json::Value = match resp.json().await {
                Ok(j) => j,
                Err(_) => return fail("AI_MODEL_LIST_FAILED"),
            };
            json["models"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| {
                            m["name"]
                                .as_str()
                                .map(|s| s.trim_start_matches("models/").to_string())
                        })
                        .collect()
                })
                .unwrap_or_default()
        }
        "ollama" | "ollama_cloud" => {
            let resp = match client.get(format!("{base}api/tags")).send().await {
                Ok(r) => r,
                Err(_) => return fail("AI_MODEL_LIST_FAILED"),
            };
            if !resp.status().is_success() {
                return fail("AI_MODEL_LIST_FAILED");
            }
            let json: serde_json::Value = match resp.json().await {
                Ok(j) => j,
                Err(_) => return fail("AI_MODEL_LIST_FAILED"),
            };
            json["models"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default()
        }
        "cohere" => {
            if api_key.is_empty() {
                return fail("PARAM_INVALID");
            }
            let resp = match client
                .get(format!("{base}models"))
                .bearer_auth(api_key.as_str())
                .send()
                .await
            {
                Ok(r) => r,
                Err(_) => return fail("AI_MODEL_LIST_FAILED"),
            };
            if !resp.status().is_success() {
                return fail("AI_MODEL_LIST_FAILED");
            }
            let json: serde_json::Value = match resp.json().await {
                Ok(j) => j,
                Err(_) => return fail("AI_MODEL_LIST_FAILED"),
            };
            json["models"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default()
        }
        _ => {
            // OpenAI 兼容族（openai/openai_responses/deepseek/openrouter/groq/自定义等）
            if api_key.is_empty() {
                return fail("PARAM_INVALID");
            }
            let resp = match client
                .get(format!("{base}models"))
                .bearer_auth(api_key.as_str())
                .send()
                .await
            {
                Ok(r) => r,
                Err(_) => return fail("AI_MODEL_LIST_FAILED"),
            };
            if !resp.status().is_success() {
                return fail("AI_MODEL_LIST_FAILED");
            }
            let json: serde_json::Value = match resp.json().await {
                Ok(j) => j,
                Err(_) => return fail("AI_MODEL_LIST_FAILED"),
            };
            json["data"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default()
        }
    };

    HttpResponse::Ok().json(FetchAiModelsResponse { success: true, models })
}

#[derive(Debug, Deserialize)]
pub struct DeleteAiModelRequest {
    pub id: String,
}

pub async fn delete_ai_model(
    body: web::Json<DeleteAiModelRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    if let Err(resp) = get_current_user_id(&http_req, &app_state) {
        return resp;
    }

    let model = match app_state.ai_model_model.get_by_id(&body.id) {
        Ok(Some(m)) => m,
        Ok(None) => return fail("MODEL_NOT_FOUND"),
        Err(e) => return internal_error_response("/api/ai/model/delete", &e),
    };

    match app_state
        .ai_provider_model
        .get_by_id(&model.provider_id)
    {
        Ok(Some(_)) => {}
        Ok(None) => return fail("MODEL_NOT_FOUND"),
        Err(e) => return internal_error_response("/api/ai/model/delete", &e),
    }

    match app_state.ai_model_model.delete(&body.id) {
        Ok(true) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            fail_code: None,
        }),
        Ok(false) => fail("MODEL_NOT_FOUND"),
        Err(e) => internal_error_response("/api/ai/model/delete", &e),
    }
}
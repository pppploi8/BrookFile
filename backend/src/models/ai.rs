use crate::database::Pool;
use std::collections::HashSet;
use rusqlite::params;
use uuid::Uuid;

pub struct AiProviderInfo {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub api_key: String,
    pub proxy: String,
    pub created_at: String,
    pub updated_at: String,
}

pub struct AiModelInfo {
    pub id: String,
    pub provider_id: String,
    pub model_id: String,
    pub supports_vision: bool,
    pub context_length: i64,
    pub max_output_tokens: i64,
    pub created_at: String,
    pub updated_at: String,
}

fn row_to_provider(row: &rusqlite::Row) -> rusqlite::Result<AiProviderInfo> {
    Ok(AiProviderInfo {
        id: row.get(0)?,
        name: row.get(1)?,
        provider_type: row.get(2)?,
        base_url: row.get(3)?,
        api_key: row.get(4)?,
        proxy: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn row_to_model(row: &rusqlite::Row) -> rusqlite::Result<AiModelInfo> {
    Ok(AiModelInfo {
        id: row.get(0)?,
        provider_id: row.get(1)?,
        model_id: row.get(2)?,
        supports_vision: row.get(3)?,
        context_length: row.get(4)?,
        max_output_tokens: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

const PROVIDER_COLUMNS: &str = "id, name, provider_type, base_url, api_key, proxy, created_at, updated_at";
const MODEL_COLUMNS: &str =
    "id, provider_id, model_id, supports_vision, context_length, max_output_tokens, created_at, updated_at";

pub struct AiProviderModel {
    pool: Pool,
}

pub struct NewAiModelData {
    pub model_id: String,
    pub supports_vision: bool,
    pub context_length: i64,
    pub max_output_tokens: i64,
}

#[derive(Clone)]
pub struct UpdateModelData {
    pub id: Option<String>,
    pub model_id: String,
    pub supports_vision: bool,
    pub context_length: i64,
    pub max_output_tokens: i64,
}

impl AiProviderModel {
    pub fn new(pool: &Pool) -> Self {
        AiProviderModel { pool: pool.clone() }
    }

    pub fn list(&self) -> Result<Vec<AiProviderInfo>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {} FROM ai_providers ORDER BY created_at ASC",
                PROVIDER_COLUMNS
            ))
            .map_err(|e| e.to_string())?;
        let providers = stmt
            .query_map([], row_to_provider)
            .map_err(|e| e.to_string())?;
        providers
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<AiProviderInfo>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let result: Result<AiProviderInfo, _> = conn.query_row(
            &format!("SELECT {} FROM ai_providers WHERE id = ?1", PROVIDER_COLUMNS),
            params![id],
            row_to_provider,
        );
        match result {
            Ok(info) => Ok(Some(info)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn create_with_models(
        &self,
        name: &str,
        provider_type: &str,
        base_url: &str,
        api_key: &str,
        proxy: &str,
        models: &[NewAiModelData],
    ) -> Result<String, String> {
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;

        let provider_id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO ai_providers (id, name, provider_type, base_url, api_key, proxy) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![provider_id, name, provider_type, base_url, api_key, proxy],
        )
        .map_err(|e| e.to_string())?;

        for m in models {
            let model_id = m.model_id.trim().to_string();
            if model_id.is_empty() {
                continue;
            }
            let id = Uuid::new_v4().to_string();
            match tx.execute(
                "INSERT INTO ai_models (id, provider_id, model_id, supports_vision, context_length, max_output_tokens) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, provider_id, model_id, m.supports_vision, m.context_length, m.max_output_tokens],
            ) {
                Ok(_) => {}
                Err(_) => return Err("MODEL_DUPLICATE".to_string()),
            }
        }

        tx.commit().map_err(|e| e.to_string())?;
        Ok(provider_id)
    }

    pub fn update(
        &self,
        id: &str,
        name: &str,
        provider_type: &str,
        base_url: &str,
        proxy: &str,
        api_key: Option<&str>,
    ) -> Result<(), String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        if let Some(key) = api_key {
            if !key.is_empty() {
                conn.execute(
                    "UPDATE ai_providers SET name = ?1, provider_type = ?2, base_url = ?3, api_key = ?4, proxy = ?5, updated_at = datetime('now') WHERE id = ?6",
                    params![name, provider_type, base_url, key, proxy, id],
                )
                .map_err(|e| e.to_string())?;
                return Ok(());
            }
        }
        conn.execute(
            "UPDATE ai_providers SET name = ?1, provider_type = ?2, base_url = ?3, proxy = ?4, updated_at = datetime('now') WHERE id = ?5",
            params![name, provider_type, base_url, proxy, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// 更新供应商并整体对账模型列表：删除被移除的、更新已存在的、新增没有 id 的。
    /// 全程在单个事务内完成，任一失败（如模型重复）则整体回滚。
    pub fn update_with_models(
        &self,
        id: &str,
        name: &str,
        provider_type: &str,
        base_url: &str,
        proxy: &str,
        api_key: Option<&str>,
        models: &[UpdateModelData],
    ) -> Result<(), String> {
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;

        let changed_key = matches!(api_key, Some(k) if !k.is_empty());
        let n = if changed_key {
            tx.execute(
                "UPDATE ai_providers SET name = ?1, provider_type = ?2, base_url = ?3, api_key = ?4, proxy = ?5, updated_at = datetime('now') WHERE id = ?6",
                params![name, provider_type, base_url, api_key.unwrap(), proxy, id],
            )
            .map_err(|e| e.to_string())?
        } else {
            tx.execute(
                "UPDATE ai_providers SET name = ?1, provider_type = ?2, base_url = ?3, proxy = ?4, updated_at = datetime('now') WHERE id = ?5",
                params![name, provider_type, base_url, proxy, id],
            )
            .map_err(|e| e.to_string())?
        };
        if n == 0 {
            return Err("PROVIDER_NOT_FOUND".to_string());
        }

        let mut stmt = tx
            .prepare("SELECT id, model_id FROM ai_models WHERE provider_id = ?1")
            .map_err(|e| e.to_string())?;
        let current: Vec<(String, String)> = stmt
            .query_map(params![id], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        drop(stmt);

        let desired_ids: HashSet<&str> =
            models.iter().filter_map(|m| m.id.as_deref()).collect();
        for (cid, _) in &current {
            if !desired_ids.contains(cid.as_str()) {
                tx.execute("DELETE FROM ai_models WHERE id = ?1", params![cid])
                    .map_err(|e| e.to_string())?;
            }
        }

        for m in models {
            let model_id = m.model_id.trim().to_string();
            if model_id.is_empty() {
                return Err("PARAM_INVALID".to_string());
            }
            match &m.id {
                Some(mid) => {
                    match tx.execute(
                        "UPDATE ai_models SET model_id = ?1, supports_vision = ?2, context_length = ?3, max_output_tokens = ?4, updated_at = datetime('now') WHERE id = ?5 AND provider_id = ?6",
                        params![model_id, m.supports_vision, m.context_length, m.max_output_tokens, mid, id],
                    ) {
                        Ok(n) if n == 0 => return Err("MODEL_NOT_FOUND".to_string()),
                        Ok(_) => {}
                        Err(_) => return Err("MODEL_DUPLICATE".to_string()),
                    }
                }
                None => {
                    let new_id = Uuid::new_v4().to_string();
                    match tx.execute(
                        "INSERT INTO ai_models (id, provider_id, model_id, supports_vision, context_length, max_output_tokens) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![new_id, id, model_id, m.supports_vision, m.context_length, m.max_output_tokens],
                    ) {
                        Ok(_) => {}
                        Err(_) => return Err("MODEL_DUPLICATE".to_string()),
                    }
                }
            }
        }

        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<bool, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let affected = conn
            .execute("DELETE FROM ai_providers WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(affected > 0)
    }
}

pub struct AiModelModel {
    pool: Pool,
}

impl AiModelModel {
    pub fn new(pool: &Pool) -> Self {
        AiModelModel { pool: pool.clone() }
    }

    pub fn list_by_provider(&self, provider_id: &str) -> Result<Vec<AiModelInfo>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {} FROM ai_models WHERE provider_id = ?1 ORDER BY created_at ASC",
                MODEL_COLUMNS
            ))
            .map_err(|e| e.to_string())?;
        let models = stmt
            .query_map(params![provider_id], row_to_model)
            .map_err(|e| e.to_string())?;
        models
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<AiModelInfo>, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let result: Result<AiModelInfo, _> = conn.query_row(
            &format!("SELECT {} FROM ai_models WHERE id = ?1", MODEL_COLUMNS),
            params![id],
            row_to_model,
        );
        match result {
            Ok(info) => Ok(Some(info)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn delete(&self, id: &str) -> Result<bool, String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;
        let affected = conn
            .execute("DELETE FROM ai_models WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(affected > 0)
    }
}
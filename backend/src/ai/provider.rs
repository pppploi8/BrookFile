use genai::adapter::AdapterKind;

pub struct ProviderPreset {
    pub type_id: &'static str,
    pub name: &'static str,
    pub adapter: AdapterKind,
    pub default_base_url: &'static str,
    pub requires_api_key: bool,
}

macro_rules! openai_compat_preset {
    ($id:literal, $name:literal, $adapter:expr, $url:literal) => {
        ProviderPreset {
            type_id: $id,
            name: $name,
            adapter: $adapter,
            default_base_url: $url,
            requires_api_key: true,
        }
    };
}

pub const PROVIDER_PRESETS: &[ProviderPreset] = &[
    openai_compat_preset!(
        "openai_completions",
        "OpenAI (Chat Completions)",
        AdapterKind::OpenAI,
        "https://api.openai.com/v1/"
    ),
    openai_compat_preset!(
        "openai_responses",
        "OpenAI (Responses)",
        AdapterKind::OpenAIResp,
        "https://api.openai.com/v1/"
    ),
    openai_compat_preset!("deepseek", "DeepSeek", AdapterKind::DeepSeek, "https://api.deepseek.com/v1/"),
    openai_compat_preset!("openrouter", "OpenRouter", AdapterKind::OpenRouter, "https://openrouter.ai/api/v1/"),
    openai_compat_preset!("groq", "Groq", AdapterKind::Groq, "https://api.groq.com/openai/v1/"),
    openai_compat_preset!("xai", "xAI", AdapterKind::Xai, "https://api.x.ai/v1/"),
    openai_compat_preset!("moonshot", "Moonshot", AdapterKind::Moonshot, "https://api.moonshot.cn/v1/"),
    openai_compat_preset!("kimi", "Kimi", AdapterKind::Kimi, "https://api.moonshot.ai/v1/"),
    openai_compat_preset!("zai", "ZAI", AdapterKind::Zai, "https://api.z.ai/api/paas/v4/"),
    openai_compat_preset!(
        "fireworks",
        "Fireworks",
        AdapterKind::Fireworks,
        "https://api.fireworks.ai/inference/v1/"
    ),
    openai_compat_preset!("together", "Together", AdapterKind::Together, "https://api.together.xyz/v1/"),
    openai_compat_preset!("nebius", "Nebius", AdapterKind::Nebius, "https://api.studio.nebius.ai/v1/"),
    openai_compat_preset!("mimo", "Mimo", AdapterKind::Mimo, "https://api.mimo.com/openai/v1/"),
    ProviderPreset {
        type_id: "anthropic",
        name: "Anthropic",
        adapter: AdapterKind::Anthropic,
        default_base_url: "https://api.anthropic.com/v1/",
        requires_api_key: true,
    },
    ProviderPreset {
        type_id: "gemini",
        name: "Gemini",
        adapter: AdapterKind::Gemini,
        default_base_url: "https://generativelanguage.googleapis.com/v1beta/",
        requires_api_key: true,
    },
    ProviderPreset {
        type_id: "ollama",
        name: "Ollama",
        adapter: AdapterKind::Ollama,
        default_base_url: "http://localhost:11434/",
        requires_api_key: false,
    },
    ProviderPreset {
        type_id: "ollama_cloud",
        name: "Ollama Cloud",
        adapter: AdapterKind::OllamaCloud,
        default_base_url: "https://ollama.com/",
        requires_api_key: true,
    },
    ProviderPreset {
        type_id: "cohere",
        name: "Cohere",
        adapter: AdapterKind::Cohere,
        default_base_url: "https://api.cohere.com/v1/",
        requires_api_key: true,
    },
    openai_compat_preset!("custom", "OpenAI Compatible", AdapterKind::OpenAI, ""),
];

pub fn preset_by_type(type_id: &str) -> Option<&'static ProviderPreset> {
    PROVIDER_PRESETS.iter().find(|p| p.type_id == type_id)
}

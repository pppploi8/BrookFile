use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Argon2Config {
    #[serde(default = "Argon2Config::default_m_cost")]
    pub m_cost: u32,
    #[serde(default = "Argon2Config::default_t_cost")]
    pub t_cost: u32,
    #[serde(default = "Argon2Config::default_p_cost")]
    pub p_cost: u32,
}

impl Argon2Config {
    fn default_m_cost() -> u32 { 19456 }
    fn default_t_cost() -> u32 { 2 }
    fn default_p_cost() -> u32 { 1 }
}

impl Default for Argon2Config {
    fn default() -> Self {
        Argon2Config {
            m_cost: Self::default_m_cost(),
            t_cost: Self::default_t_cost(),
            p_cost: Self::default_p_cost(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "Config::default_port")]
    pub port: u16,
    #[serde(default)]
    pub argon2: Argon2Config,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            port: Self::default_port(),
            argon2: Argon2Config::default(),
        }
    }
}

static CONFIG: OnceLock<Config> = OnceLock::new();

impl Config {
    fn default_port() -> u16 { 3000 }

    pub fn load() -> Config {
        let config_path = Path::new("config.json");
        
        if !config_path.exists() {
            return Config::default();
        }

        match fs::read_to_string(config_path) {
            Ok(content) => {
                match serde_json::from_str(&content) {
                    Ok(config) => config,
                    Err(e) => {
                        eprintln!("Warning: Failed to parse config.json: {}. Using defaults.", e);
                        Config::default()
                    }
                }
            }
            Err(_) => Config::default(),
        }
    }

    pub fn init() -> &'static Config {
        CONFIG.get_or_init(Self::load)
    }

    pub fn global() -> &'static Config {
        CONFIG.get().expect("Config not initialized")
    }

    pub fn create_argon2_params(&self) -> argon2::Params {
        argon2::Params::new(self.argon2.m_cost, self.argon2.t_cost, self.argon2.p_cost, Some(32))
            .expect("Invalid Argon2 params")
    }

    pub fn create_argon2(&self) -> argon2::Argon2<'static> {
        argon2::Argon2::new(
            argon2::Algorithm::Argon2id,
            argon2::Version::V0x13,
            self.create_argon2_params(),
        )
    }
}

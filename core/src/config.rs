use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    pub vault_path: PathBuf,
    pub ai_endpoint: String,
    pub opa_endpoint: String,
    pub port: u16,
    pub fallback_model: String,
    pub embed_fallback_model: String,
    pub web_root: PathBuf,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            vault_path: Self::path_from_env("VAULT_PATH", "vault"),
            ai_endpoint: std::env::var("AI_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            opa_endpoint: std::env::var("OPA_ENDPOINT")
                .unwrap_or_else(|_| "http://opa:8181".to_string()),
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            fallback_model: std::env::var("AETHERIS_FALLBACK_MODEL")
                .unwrap_or_else(|_| "qwen2.5:7b".to_string()),
            embed_fallback_model: std::env::var("AETHERIS_EMBED_FALLBACK_MODEL")
                .unwrap_or_else(|_| "nomic-embed-text".to_string()),
            web_root: Self::path_from_env("WEB_ROOT", "/opt/aetheris/web"),
        }
    }

    pub fn new() -> Self {
        Self::from_env()
    }

    fn path_from_env(env_var: &str, default: &str) -> PathBuf {
        std::env::var(env_var)
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(default))
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults_and_env_overrides() {
        unsafe {
            std::env::remove_var("AETHERIS_FALLBACK_MODEL");
            std::env::remove_var("AETHERIS_EMBED_FALLBACK_MODEL");
        }
        let config = Config::new();
        assert_eq!(config.port, 8080);
        assert_eq!(config.ai_endpoint, "http://localhost:11434");
        assert_eq!(config.opa_endpoint, "http://opa:8181");
        assert_eq!(config.fallback_model, "qwen2.5:7b");
        assert_eq!(config.embed_fallback_model, "nomic-embed-text");
        assert!(!config.vault_path.to_string_lossy().starts_with('/'));
        assert!(!config.vault_path.to_string_lossy().starts_with("C:\\"));

        unsafe {
            std::env::set_var("AETHERIS_FALLBACK_MODEL", "qwen2.5-coder:7b");
            std::env::set_var("AETHERIS_EMBED_FALLBACK_MODEL", "bge-m3");
        }
        let config = Config::new();
        assert_eq!(config.fallback_model, "qwen2.5-coder:7b");
        assert_eq!(config.embed_fallback_model, "bge-m3");
        unsafe {
            std::env::remove_var("AETHERIS_FALLBACK_MODEL");
            std::env::remove_var("AETHERIS_EMBED_FALLBACK_MODEL");
        }
    }
}

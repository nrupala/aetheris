use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    pub vault_path: PathBuf,
    pub store_path: PathBuf,
    pub ai_endpoint: String,
    pub opa_endpoint: String,
    #[allow(dead_code)] // enforcement wiring lands in Phase 3
    pub opa_enforce: bool,
    pub opa_fail_open: bool,
    pub port: u16,
    pub fallback_model: String,
    pub embed_fallback_model: String,
    pub web_root: PathBuf,
    pub cf_access_team_domain: String,
    pub cf_access_aud: Vec<String>,
    pub cf_access_jwks_path: PathBuf,
    pub cf_jwt_verify: bool,
}

impl Config {
    pub fn from_env() -> Self {
        let vault_path = Self::path_from_env("VAULT_PATH", "vault");
        Self {
            store_path: std::env::var("AETHERIS_STORE_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| vault_path.join("aetheris.db")),
            vault_path: vault_path.clone(),
            ai_endpoint: std::env::var("AI_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            opa_endpoint: std::env::var("OPA_ENDPOINT")
                .unwrap_or_else(|_| "http://127.0.0.1:8181".to_string()),
            opa_enforce: std::env::var("OPA_ENFORCE")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            opa_fail_open: std::env::var("OPA_FAIL_OPEN")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(true),
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            fallback_model: std::env::var("AETHERIS_FALLBACK_MODEL")
                .unwrap_or_else(|_| "qwen2.5:7b".to_string()),
            embed_fallback_model: std::env::var("AETHERIS_EMBED_FALLBACK_MODEL")
                .unwrap_or_else(|_| "nomic-embed-text".to_string()),
            web_root: Self::path_from_env("WEB_ROOT", "/opt/aetheris/web"),
            cf_access_team_domain: std::env::var("CF_ACCESS_TEAM_DOMAIN")
                .unwrap_or_else(|_| "https://nrupal.cloudflareaccess.com".to_string()),
            cf_access_aud: std::env::var("CF_ACCESS_AUD")
                .unwrap_or_default()
                .split(',')
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.trim().to_string())
                .collect(),
            cf_access_jwks_path: Self::path_from_env(
                "CF_ACCESS_JWKS_PATH",
                "/etc/aetheris/cf_access_jwks.json",
            ),
            cf_jwt_verify: std::env::var("CF_JWT_VERIFY")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
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
            std::env::remove_var("OPA_ENFORCE");
            std::env::remove_var("OPA_FAIL_OPEN");
        }
        let config = Config::new();
        assert_eq!(config.port, 8080);
        assert_eq!(config.ai_endpoint, "http://localhost:11434");
        assert_eq!(config.opa_endpoint, "http://127.0.0.1:8181");
        assert_eq!(config.fallback_model, "qwen2.5:7b");
        assert_eq!(config.embed_fallback_model, "nomic-embed-text");
        assert!(!config.opa_enforce);
        assert!(config.opa_fail_open);
        assert!(!config.vault_path.to_string_lossy().starts_with('/'));
        assert!(!config.vault_path.to_string_lossy().starts_with("C:\\"));

        unsafe {
            std::env::set_var("AETHERIS_FALLBACK_MODEL", "qwen2.5-coder:7b");
            std::env::set_var("AETHERIS_EMBED_FALLBACK_MODEL", "bge-m3");
            std::env::set_var("OPA_ENFORCE", "true");
            std::env::set_var("OPA_FAIL_OPEN", "0");
        }
        let config = Config::new();
        assert_eq!(config.fallback_model, "qwen2.5-coder:7b");
        assert_eq!(config.embed_fallback_model, "bge-m3");
        assert!(config.opa_enforce);
        assert!(!config.opa_fail_open);
        unsafe {
            std::env::remove_var("AETHERIS_FALLBACK_MODEL");
            std::env::remove_var("AETHERIS_EMBED_FALLBACK_MODEL");
            std::env::remove_var("OPA_ENFORCE");
            std::env::remove_var("OPA_FAIL_OPEN");
        }
    }
}

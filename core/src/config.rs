use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    pub vault_path: PathBuf,
    pub ai_endpoint: String,
    pub opa_endpoint: String,
    pub port: u16,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            vault_path: Self::path_from_env("VAULT_PATH", "vault"),
            ai_endpoint: std::env::var("AI_ENDPOINT")
                .unwrap_or_else(|_| "http://host.docker.internal:1234".to_string()),
            opa_endpoint: std::env::var("OPA_ENDPOINT")
                .unwrap_or_else(|_| "http://opa:8181".to_string()),
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
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
    fn test_default_config_has_sensible_defaults() {
        let config = Config::new();
        assert_eq!(config.port, 8080);
        assert_eq!(config.ai_endpoint, "http://host.docker.internal:1234");
        assert_eq!(config.opa_endpoint, "http://opa:8181");
        assert!(!config.vault_path.to_string_lossy().starts_with('/'));
        assert!(!config.vault_path.to_string_lossy().starts_with("C:\\"));
    }

    #[test]
    fn test_config_is_cloneable() {
        let config = Config::new();
        let cloned = config.clone();
        assert_eq!(config.port, cloned.port);
        assert_eq!(config.ai_endpoint, cloned.ai_endpoint);
        assert_eq!(config.opa_endpoint, cloned.opa_endpoint);
    }
}

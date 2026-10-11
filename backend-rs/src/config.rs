//! Boot settings (dotenvy + envy). Full key set lands in T05;
//! T03 owns `hash_cost` + the production cost floor.

use serde::Deserialize;

/// Application settings. Precedence: real environment > `.env` > defaults.
#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    /// One of `development|test|production`.
    #[serde(default = "default_node_env")]
    pub node_env: String,
    /// Argon2id `t` cost. Tests pin `t=1` for speed; default `2`.
    #[serde(default = "default_hash_cost")]
    pub hash_cost: u32,
    /// Serve port. `0` is rejected.
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_node_env() -> String {
    "development".to_string()
}

fn default_hash_cost() -> u32 {
    2
}

fn default_port() -> u16 {
    3001
}

/// Boot-time configuration failure.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Env deserialization failure.
    #[error("invalid configuration: {0}")]
    Env(#[from] envy::Error),
    /// Validation failure with an actionable message.
    #[error("{0}")]
    Invalid(String),
}

impl Settings {
    /// Load once at boot: `.env` (if present), then environment, then validate.
    pub fn load() -> Result<Self, ConfigError> {
        dotenvy::dotenv().ok();
        let settings: Settings = envy::from_env()?;
        settings.validate()?;
        Ok(settings)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        match self.node_env.as_str() {
            "development" | "test" | "production" => {}
            other => {
                return Err(ConfigError::Invalid(format!(
                    "NODE_ENV must be one of development|test|production, got {other:?}"
                )));
            }
        }
        if self.port == 0 {
            return Err(ConfigError::Invalid("PORT must be non-zero".to_string()));
        }
        if self.node_env == "production" && self.hash_cost < 2 {
            return Err(ConfigError::Invalid(
                "HASH_COST must be >= 2 in production".to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct SavedEnv(Vec<(&'static str, Option<String>)>);

    impl SavedEnv {
        fn set(vars: &[(&'static str, &str)]) -> Self {
            let saved = vars
                .iter()
                .map(|(key, _)| (*key, std::env::var(key).ok()))
                .collect();
            for (key, value) in vars {
                std::env::set_var(key, value);
            }
            Self(saved)
        }
    }

    impl Drop for SavedEnv {
        fn drop(&mut self) {
            for (key, old) in self.0.drain(..) {
                match old {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }

    #[test]
    fn hash_cost_floor() {
        let _guard = ENV_LOCK.lock().unwrap();
        // Production with HASH_COST=1 must fail boot.
        {
            let _env = SavedEnv::set(&[("NODE_ENV", "production"), ("HASH_COST", "1")]);
            let err = Settings::load().expect_err("production HASH_COST=1 must fail boot");
            assert!(
                err.to_string().contains("HASH_COST"),
                "unexpected error: {err}"
            );
        }
        // Tests pin t=1 and must load outside production.
        {
            let _env = SavedEnv::set(&[("NODE_ENV", "test"), ("HASH_COST", "1")]);
            let settings = Settings::load().expect("test HASH_COST=1 must load");
            assert_eq!(settings.hash_cost, 1);
        }
    }
}

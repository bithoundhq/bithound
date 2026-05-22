//! Inline-secret detection + env-var resolution helpers.
//!
//! The TOML schema only ever references secrets by env-var name
//! (fields suffixed `_env`). Any field whose name ends in `password`,
//! `token`, or `secret` (other than the `*_env` variants) is rejected
//! upfront so a copy-paste of a real credential into a tracked config
//! file becomes a parse error rather than a leak.

use std::env;

use secrecy::SecretString;
use toml::Value;

use super::ConfigError;

/// Recursively walks a parsed TOML document and rejects any key whose
/// name suggests it carries an inline secret (`password`, `token`,
/// `secret`) without the mandatory `_env` suffix.
///
/// Returned errors include the dotted path to the offending key so the
/// operator can locate it without grepping.
pub(super) fn reject_inline_secrets(value: &Value) -> Result<(), ConfigError> {
    fn walk(value: &Value, path: &mut Vec<String>) -> Result<(), ConfigError> {
        match value {
            Value::Table(map) => {
                for (key, child) in map {
                    if looks_like_inline_secret(key) {
                        let here = if path.is_empty() {
                            key.clone()
                        } else {
                            format!("{}.{}", path.join("."), key)
                        };
                        return Err(ConfigError::InlineSecret(here));
                    }
                    path.push(key.clone());
                    walk(child, path)?;
                    path.pop();
                }
            }
            Value::Array(arr) => {
                for (idx, item) in arr.iter().enumerate() {
                    path.push(format!("[{}]", idx));
                    walk(item, path)?;
                    path.pop();
                }
            }
            _ => {}
        }
        Ok(())
    }

    walk(value, &mut Vec::new())
}

fn looks_like_inline_secret(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with("_env") {
        return false;
    }
    lower == "password"
        || lower == "token"
        || lower == "secret"
        || lower.ends_with("_password")
        || lower.ends_with("_token")
        || lower.ends_with("_secret")
}

/// Returns the value of an env var, wrapped in `SecretString` so it
/// never leaks through `Debug` / logging. `MissingEnv` if the var
/// isn't set; the variable's presence should also be checked at
/// validation time so callers fail before any work is done.
pub(super) fn read_env_secret(name: &str) -> Result<SecretString, ConfigError> {
    let raw = env::var(name).map_err(|_| ConfigError::MissingEnv(name.to_string()))?;
    Ok(SecretString::from(raw))
}

/// Presence-check an env var without exposing its value. Used by the
/// upfront validation pass so config errors surface before secrets
/// are read into memory.
pub(super) fn require_env_set(name: &str) -> Result<(), ConfigError> {
    if env::var_os(name).is_none() {
        return Err(ConfigError::MissingEnv(name.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bare_password_at_root() {
        let v: Value = toml::from_str(r#"password = "hunter2""#).unwrap();
        let err = reject_inline_secrets(&v).unwrap_err();
        match err {
            ConfigError::InlineSecret(p) => assert_eq!(p, "password"),
            other => panic!("expected InlineSecret, got {:?}", other),
        }
    }

    #[test]
    fn rejects_nested_token_with_dotted_path() {
        let v: Value = toml::from_str(
            r#"
            [notifications.discord]
            bot_token = "abc"
            "#,
        )
        .unwrap();
        let err = reject_inline_secrets(&v).unwrap_err();
        match err {
            ConfigError::InlineSecret(p) => assert_eq!(p, "notifications.discord.bot_token"),
            other => panic!("expected InlineSecret, got {:?}", other),
        }
    }

    #[test]
    fn rejects_secret_inside_array_of_tables() {
        let v: Value = toml::from_str(
            r#"
            [[bitcoin_nodes]]
            id = "a"
            password = "x"
            "#,
        )
        .unwrap();
        let err = reject_inline_secrets(&v).unwrap_err();
        match err {
            ConfigError::InlineSecret(p) => {
                assert!(p.contains("bitcoin_nodes"), "got {p}");
                assert!(p.ends_with("password"), "got {p}");
            }
            other => panic!("expected InlineSecret, got {:?}", other),
        }
    }

    #[test]
    fn accepts_env_suffixed_fields() {
        let v: Value = toml::from_str(
            r#"
            password_env = "BITHOUND_X_PASSWORD"
            bot_token_env = "BITHOUND_Y_TOKEN"
            client_secret_env = "BITHOUND_Z_SECRET"
            "#,
        )
        .unwrap();
        reject_inline_secrets(&v).expect("env-suffixed fields are fine");
    }

    #[test]
    fn accepts_non_secret_keys_named_similarly() {
        let v: Value = toml::from_str(
            r#"
            user = "bithound"
            id_file = "/var/lib/bithound/id"
            password_policy = "rotate"
            "#,
        )
        .unwrap();
        reject_inline_secrets(&v).expect("non-secret keys must be allowed");
    }
}

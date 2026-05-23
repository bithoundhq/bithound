//! Operator HTTP API configuration.
//!
//! `[api]` is optional in `bithound.toml`. When omitted, the API
//! defaults to enabled and binds to `127.0.0.1:8487` — see the table
//! defaults below.

use std::net::SocketAddr;

use serde::Deserialize;

/// `[api]` block. Omitting the block entirely yields
/// `ApiConfig::default()` — enabled, loopback bind on port 8487.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiConfig {
    /// Local socket the HTTP server listens on. Defaults to
    /// `127.0.0.1:8487`. V0 is loopback-only by design — there is no
    /// auth, no TLS, no CORS; the loopback default is the safety
    /// mechanism. Operators wanting to bind elsewhere must set this
    /// explicitly.
    #[serde(default = "default_bind")]
    pub bind: SocketAddr,

    /// Set false to skip spawning the API task entirely. Useful for
    /// embedded deployments and for tests that exercise the rest of
    /// the runtime without binding a port.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            bind: default_bind(),
            enabled: default_enabled(),
        }
    }
}

fn default_bind() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 8487))
}

fn default_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_loopback_on_8487_and_enabled() {
        let c = ApiConfig::default();
        assert_eq!(c.bind, SocketAddr::from(([127, 0, 0, 1], 8487)));
        assert!(c.enabled);
    }

    #[test]
    fn parses_explicit_bind_and_disabled_flag() {
        let toml = r#"
            bind = "0.0.0.0:9999"
            enabled = false
        "#;
        let c: ApiConfig = toml::from_str(toml).expect("parse");
        assert_eq!(c.bind.port(), 9999);
        assert!(!c.enabled);
    }

    #[test]
    fn rejects_invalid_bind_string() {
        let toml = "bind = \"not-a-socket-addr\"\n";
        assert!(toml::from_str::<ApiConfig>(toml).is_err());
    }

    #[test]
    fn unknown_field_is_rejected() {
        let toml = "extra = true\n";
        assert!(toml::from_str::<ApiConfig>(toml).is_err());
    }
}

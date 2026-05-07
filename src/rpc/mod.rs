mod auth;
mod types;

use std::{path::PathBuf, sync::atomic};

use auth::*;
use types::*;

#[derive(Debug, thiserror::Error)]
pub enum RpcError {
    #[error(transparent)]
    Authentication(#[from] AuthenticationError),
    #[error("RPC error: {0}")]
    Rpc(String),
    #[error("Request error: {0}")]
    Request(#[from] reqwest::Error),
    #[error("Result unavailable.")]
    ResultUnavailable,
    #[error("Http status: {0} - {1}")]
    HttpStatus(u16, String),
    #[error("Json error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug)]
pub struct RpcConfig {
    pub authentication_method: AuthenticationMethod,
    pub endpoint: String,
}

#[derive(Debug)]
pub struct RpcClient {
    client: reqwest::Client,
    endpoint: String,
    token: String,
    next_id: atomic::AtomicI64,
}

impl RpcClient {
    pub fn new(config: RpcConfig) -> Result<Self, RpcError> {
        let token = match config.authentication_method {
            AuthenticationMethod::Cookie { cookie_file_path } => {
                CookieAuthenticator::new(&cookie_file_path)?.get_authentication_token()
            }
            AuthenticationMethod::Password { user, password } => {
                UserAuthenticator::new(user, password).get_authentication_token()
            }
        };

        let client = reqwest::Client::new();
        let next_id = atomic::AtomicI64::new(1);

        Ok(Self {
            client,
            endpoint: config.endpoint,
            token,
            next_id,
        })
    }

    pub async fn call(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<Response, RpcError> {
        let id = self.next_id.fetch_add(1, atomic::Ordering::Relaxed);

        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });

        let http_res = self
            .client
            .post(&self.endpoint)
            .header("Authorization", format!("Basic {}", self.token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        let status = http_res.status();
        if !status.is_success() {
            let body = http_res.text().await.unwrap_or_default();
            return Err(RpcError::HttpStatus(status.as_u16(), body));
        }

        let res: Response = http_res.json().await?;

        if let Some(error) = res.error {
            return Err(RpcError::Rpc(error.message));
        }

        Ok(res)
    }
}

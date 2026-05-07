use std::path::{Path, PathBuf};

use base64::Engine;

#[derive(Debug, thiserror::Error)]
pub enum AuthenticationError {
    #[error("Cookie file missing.")]
    CookieFileMissing,
    #[error("Cookie malformed.")]
    CookieMalformed,
}

#[derive(Debug)]
pub enum AuthenticationMethod {
    Cookie { cookie_file_path: PathBuf },
    Password { user: String, password: String },
}

pub trait Authenticator {
    fn get_authentication_token(&self) -> String;
}

pub struct CookieAuthenticator {
    pub cookie: String,
}

impl CookieAuthenticator {
    pub fn new(cookie_file: &Path) -> Result<Self, AuthenticationError> {
        let contents = std::fs::read_to_string(cookie_file)
            .map_err(|_| AuthenticationError::CookieFileMissing)?;

        if contents.find(":").is_none() {
            return Err(AuthenticationError::CookieMalformed);
        }

        Ok(Self { cookie: contents })
    }
}

impl Authenticator for CookieAuthenticator {
    fn get_authentication_token(&self) -> String {
        let engine = base64::engine::general_purpose::STANDARD;
        let token = engine.encode(&self.cookie);

        token
    }
}

pub struct UserAuthenticator {
    user: String,
    pass: String,
}

impl UserAuthenticator {
    pub fn new(user: String, pass: String) -> Self {
        Self { user, pass }
    }
}

impl Authenticator for UserAuthenticator {
    fn get_authentication_token(&self) -> String {
        let payload = format!("{}:{}", self.user, self.pass);
        let engine = base64::engine::general_purpose::STANDARD;
        let token = engine.encode(&payload);

        token
    }
}

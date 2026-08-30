//! Service-layer error type.
//!
//! Services return `Result<T, ServiceError>` and know nothing about HTTP:
//! no `StatusCode`, no `serde_json::Value` bodies. The mapping from variants
//! to HTTP status codes / JSON bodies happens in exactly ONE place —
//! `impl ResponseError for ServiceError` in `src/controllers/mod.rs`.

use std::fmt;

#[derive(Debug)]
pub enum ServiceError {
    /// 422 — value is not a parseable UUID
    MalformedUuid,
    /// 422 — name validation failure; carries the client-safe message
    InvalidName(String),
    /// 422 — password validation failure; carries the client-safe message
    InvalidPassword(String),
    /// 400
    AlreadyRegistered,
    /// 404
    NotFound,
    /// 401
    InvalidCredentials,
    /// 400 — required request parameter missing; carries client-safe message
    MissingParam(String),
    /// 500 — stored password hash cannot be parsed
    CorruptedHash,
    /// 500 — unexpected failure. The string is a *context label only*.
    /// Never leak `DbErr`/infra details to clients — log them instead
    /// (see [`internal`]).
    Internal(String),
}

/// Log the underlying error server-side and build a 500 that is safe to return.
pub fn internal(context: &str, err: impl fmt::Display) -> ServiceError {
    tracing::error!("{context}: {err}");
    ServiceError::Internal(context.to_owned())
}

impl ServiceError {
    /// HTTP status code. Returns a plain `u16` to keep this module
    /// framework-free; the controller layer converts it to `StatusCode`.
    pub fn http_status(&self) -> u16 {
        match self {
            Self::MalformedUuid | Self::InvalidName(_) | Self::InvalidPassword(_) => 422,
            Self::AlreadyRegistered | Self::MissingParam(_) => 400,
            Self::NotFound => 404,
            Self::InvalidCredentials => 401,
            Self::CorruptedHash | Self::Internal(_) => 500,
        }
    }

    /// Message that is safe to send to clients (no DB / infra details).
    pub fn client_message(&self) -> String {
        match self {
            Self::MalformedUuid => "Malformed uuid".to_owned(),
            Self::InvalidName(msg) | Self::InvalidPassword(msg) | Self::MissingParam(msg) => {
                msg.clone()
            }
            Self::AlreadyRegistered => "already registered".to_owned(),
            Self::NotFound => "not registered".to_owned(),
            Self::InvalidCredentials => "invalid password".to_owned(),
            Self::CorruptedHash | Self::Internal(_) => "internal server error".to_owned(),
        }
    }
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.client_message())
    }
}

impl std::error::Error for ServiceError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_codes_match_spec() {
        assert_eq!(ServiceError::MalformedUuid.http_status(), 422);
        assert_eq!(
            ServiceError::InvalidName("Name too short".into()).http_status(),
            422
        );
        assert_eq!(ServiceError::AlreadyRegistered.http_status(), 400);
        assert_eq!(ServiceError::NotFound.http_status(), 404);
        assert_eq!(ServiceError::InvalidCredentials.http_status(), 401);
        assert_eq!(ServiceError::Internal("x".into()).http_status(), 500);
    }

    #[test]
    fn internal_errors_never_leak_details() {
        let e = ServiceError::Internal("relation \"users\" does not exist".to_owned());
        assert_eq!(e.client_message(), "internal server error");
        assert!(!e.to_string().contains("relation"));
    }
}

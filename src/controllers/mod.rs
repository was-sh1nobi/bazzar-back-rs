pub mod auth;
pub mod balance;
pub mod health;
pub mod items;
pub mod list;
pub mod shop;
pub mod stats;

use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use serde_json::json;

use crate::services::error::ServiceError;

/// The ONE place where service errors become HTTP responses.
/// Services stay transport-agnostic; status codes + JSON error shape live here.
impl ResponseError for ServiceError {
    fn status_code(&self) -> StatusCode {
        StatusCode::from_u16(self.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).json(json!({ "message": self.client_message() }))
    }
}

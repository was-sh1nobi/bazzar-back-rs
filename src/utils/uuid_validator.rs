use std::sync::LazyLock;
use actix_web::http::StatusCode;
use regex::Regex;
use serde_json::{json, Value};

static UUID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$").unwrap()
});

/// just validates uuid with regex, nothing else
pub fn validate_uuid(uuid: &str) -> bool {
    if UUID_RE.is_match(uuid) {
        true
    } else {
        false
    }
}
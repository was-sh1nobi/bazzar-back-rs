//! Shared request validation. Called once at the service boundary so the
//! same rules are not copy-pasted across every service function.

use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;

use crate::services::error::ServiceError;

/// Canonical UUID check (replaces the old hand-rolled regex — the `uuid`
/// crate was already a dependency and does this correctly).
pub fn validate_uuid(uuid: &str) -> bool {
    uuid::Uuid::parse_str(uuid).is_ok()
}

/// Trim + validate a username. Returns the trimmed name on success.
pub fn validate_name(name: &str) -> Result<String, ServiceError> {
    let trimmed = name.trim();
    if trimmed.chars().count() < 2 {
        return Err(ServiceError::InvalidName("Name too short".to_owned()));
    }
    if trimmed.chars().any(char::is_whitespace) {
        return Err(ServiceError::InvalidName(
            "Name must not contain spaces".to_owned(),
        ));
    }
    Ok(trimmed.to_owned())
}

/// Password length policy: 4..=64 chars (trimmed) — guide §14.
pub fn validate_password(password: &str) -> Result<(), ServiceError> {
    let len = password.trim().len();
    if !(4..=64).contains(&len) {
        return Err(ServiceError::InvalidPassword(
            "password is too short or too long".to_owned(),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Listing body validation (`ItemsAddBody` — guide §10.1 / §14)
// ---------------------------------------------------------------------------

/// Untagged "string or number" — accepts BOTH plugin request shapes (§10.1):
/// legacy `{"amount": "3", "price": "1500.0"}` and modern `{"amount": 3, "price": 1500.0}`.
/// (Note: std `Result<String, i64>` would NOT do this — serde deserializes
/// `Result` as the externally tagged enum `{"Ok":..}|{"Err":..}`, which no
/// client sends. Untagged is the correct mechanism for string-or-number.)
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum StrNum {
    Text(String),
    Number(f64),
}

impl StrNum {
    /// Parse to f64 like Node `Number(trim())` — `None` on failure.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Text(s) => s.trim().parse::<f64>().ok(),
            Self::Number(n) => Some(*n),
        }
    }

    /// Raw text length (only meaningful for the legacy string shape).
    fn text_len(&self) -> Option<usize> {
        match self {
            Self::Text(s) => Some(s.trim().len()),
            Self::Number(_) => None,
        }
    }
}

/// `^[A-Z0-9_]+$` — uppercase Minecraft material names only (Node `materialRe`).
static MATERIAL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Z0-9_]+$").expect("static regex must compile"));

/// Material: trimmed, must match `^[A-Z0-9_]+$` (rejects lowercase, spaces,
/// symbols). Returns the trimmed name for storage.
pub fn validate_material(material: &str) -> Result<String, ServiceError> {
    let trimmed = material.trim();
    if trimmed.is_empty() || !MATERIAL_RE.is_match(trimmed) {
        return Err(ServiceError::InvalidInput("invalid material".to_owned()));
    }
    Ok(trimmed.to_owned())
}

fn bad_amount() -> ServiceError {
    ServiceError::InvalidInput("invalid amount (1..64)".to_owned())
}

/// Listing amount: string or numeric (see [`StrNum`]). Parsed like Node
/// `Number(trim())` + `Math.floor`, must be finite and 1..=64 (AH listing cap, §14).
pub fn parse_listing_amount(amount: &StrNum) -> Result<i32, ServiceError> {
    let raw = amount.as_f64().ok_or_else(bad_amount)?;
    if !raw.is_finite() {
        return Err(bad_amount());
    }
    let floored = raw.floor();
    if !(1.0..=64.0).contains(&floored) {
        return Err(bad_amount());
    }
    Ok(floored as i32)
}

fn bad_price() -> ServiceError {
    ServiceError::InvalidInput("invalid price".to_owned())
}

/// Listing price: string or numeric (see [`StrNum`]). Legacy strings must be
/// at most 20 chars (Node `price.length > 20` reject). Must be finite and > 0
/// (§14). Returns the normalized TEXT form for the `items.price` column
/// (Node stores `String(p)`).
pub fn parse_listing_price(price: &StrNum) -> Result<String, ServiceError> {
    if price.text_len().is_some_and(|len| len > 20) {
        return Err(ServiceError::InvalidInput("price too long".to_owned()));
    }
    let p = price.as_f64().ok_or_else(bad_price)?;
    if !p.is_finite() || p <= 0.0 {
        return Err(bad_price());
    }
    Ok(format!("{p}"))
}

/// Shop buy amount: 1..=320 (up to 5 stacks). Floored, finite.
pub fn parse_shop_buy_amount(amount: Option<&StrNum>) -> Result<i32, ServiceError> {
    let raw = amount
        .and_then(|a| a.as_f64())
        .ok_or_else(|| ServiceError::InvalidInput("invalid amount (1..320)".to_owned()))?;
    if !raw.is_finite() {
        return Err(ServiceError::InvalidInput(
            "invalid amount (1..320)".to_owned(),
        ));
    }
    let floored = raw.floor();
    if !(1.0..=320.0).contains(&floored) {
        return Err(ServiceError::InvalidInput(
            "invalid amount (1..320)".to_owned(),
        ));
    }
    Ok(floored as i32)
}

/// Shop sell amount: 1..=2304 (full inventory). Floored, finite.
pub fn parse_shop_sell_amount(amount: Option<&StrNum>) -> Result<i32, ServiceError> {
    let raw = amount
        .and_then(|a| a.as_f64())
        .ok_or_else(|| ServiceError::InvalidInput("invalid amount".to_owned()))?;
    if !raw.is_finite() {
        return Err(ServiceError::InvalidInput("invalid amount".to_owned()));
    }
    let floored = raw.floor();
    if !(1.0..=2304.0).contains(&floored) {
        return Err(ServiceError::InvalidInput("invalid amount".to_owned()));
    }
    Ok(floored as i32)
}

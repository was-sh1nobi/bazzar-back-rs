use std::future::{Ready, ready};
use std::sync::LazyLock;

use actix_web::{Error, FromRequest, HttpRequest, dev::Payload, error::ErrorUnauthorized};
use chrono::{Duration, Utc};
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation, decode, encode,
};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

const ISSUER: &str = "bazzar-back";
const AUDIENCE: &str = "bazzar-client";
/// Token TTL: 24h
pub const TTL_SECONDS: i64 = 24 * 60 * 60;
/// Leeway for clock skew when validating `exp`/`nbf`
const LEEWAY_SECONDS: u64 = 60;

static JWT_SECRET: LazyLock<String> = LazyLock::new(|| {
    // Load .env if present (idempotent)
    let _ = dotenvy::dotenv();
    match std::env::var("JWT_SECRET") {
        Ok(s) if s.len() >= 32 => s,
        Ok(s) if !s.is_empty() => {
            if std::env::var("APP_ENV").as_deref() == Ok("production")
                || std::env::var("ENVIRONMENT").as_deref() == Ok("production")
            {
                panic!("FATAL: JWT_SECRET must be >= 32 bytes in production");
            }
            eprintln!(
                "[WARN] JWT_SECRET is too short ({} bytes). Use >= 32 bytes for HS256 security.",
                s.len()
            );
            s
        }
        _ => {
            if std::env::var("APP_ENV").as_deref() == Ok("production")
                || std::env::var("ENVIRONMENT").as_deref() == Ok("production")
            {
                panic!("FATAL: JWT_SECRET must be set to >= 32 bytes in production");
            }
            eprintln!(
                "[WARN] JWT_SECRET not set — using dev fallback. \
                 Set JWT_SECRET env var to a random 32+ byte string in production!"
            );
            "dev-only-please-change-this-to-32+byte-random-secret!!".to_string()
        }
    }
});

fn encoding_key() -> EncodingKey {
    EncodingKey::from_secret(JWT_SECRET.as_bytes())
}

fn decoding_key() -> DecodingKey {
    DecodingKey::from_secret(JWT_SECRET.as_bytes())
}

// ---------------------------------------------------------------------------
// Claims
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Claims {
    /// Subject: player UUID (canonical)
    pub sub: String,
    pub username: String,
    /// Issued-at (seconds since epoch)
    pub iat: usize,
    /// Expiration (seconds since epoch)
    pub exp: usize,
    pub iss: String,
    pub aud: String,
    /// Optional unique token ID to prevent replay if you store/revoke JTIs
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jti: Option<String>,
}

impl Claims {
    pub fn new(player_uuid: &str, username: &str) -> Self {
        let now = Utc::now();
        let exp = now + Duration::seconds(TTL_SECONDS);
        Self {
            sub: player_uuid.to_owned(),
            username: username.to_owned(),
            iat: now.timestamp() as usize,
            exp: exp.timestamp() as usize,
            iss: ISSUER.to_owned(),
            aud: AUDIENCE.to_owned(),
            jti: Some(uuid::Uuid::new_v4().to_string()),
        }
    }

    /// True if token is expired w.r.t. current time (with leeway)
    pub fn is_expired(&self) -> bool {
        let now = Utc::now().timestamp() as usize;
        self.exp + LEEWAY_SECONDS as usize <= now
    }
}

// ---------------------------------------------------------------------------
// Builder / Verifier  (prevents forgery via HS256 signature + strict validation)
// ---------------------------------------------------------------------------

/// Build a signed HS256 JWT for `player_uuid` + `username`.
/// Uses `JWT_SECRET` (HMAC-SHA256) — forgery requires the secret.
pub fn create_jwt(
    player_uuid: &str,
    username: &str,
) -> Result<String, jsonwebtoken::errors::Error> {
    let claims = Claims::new(player_uuid, username);
    let header = Header::new(Algorithm::HS256);
    encode(&header, &claims, &encoding_key())
}

/// Alias — same as `create_jwt` for ergonomics where `builder` naming is preferred.
pub fn build_token(
    player_uuid: &str,
    username: &str,
) -> Result<String, jsonwebtoken::errors::Error> {
    create_jwt(player_uuid, username)
}

/// Verify the JWT signature + exp/iss/aud/alg. Returns `Claims` on success.
///
/// # Forgery prevention
/// - Algorithm is pinned to `HS256`; `none` and `RS256` etc. are rejected.
/// - `exp` is required and validated (with small leeway).
/// - `iss` and `aud` are validated against expected values.
/// - Signature is verified with `JWT_SECRET` HMAC (no public-key confusion).
pub fn verify_jwt(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    validation.leeway = LEEWAY_SECONDS;
    validation.set_issuer(&[ISSUER]);
    validation.set_audience(&[AUDIENCE]);
    // jsonwebtoken already rejects `alg: none` and enforces `validation.algorithms`
    validation.required_spec_claims.insert("exp".to_string());
    validation.required_spec_claims.insert("sub".to_string());

    let data: TokenData<Claims> = decode(token, &decoding_key(), &validation)?;
    Ok(data.claims)
}

// ---------------------------------------------------------------------------
// Extractor: `auth: Auth`
// ---------------------------------------------------------------------------

/// Actix extractor that pulls `Authorization: Bearer <JWT>` from the request,
/// verifies the signature + claims, and exposes the authenticated identity.
///
/// ```ignore
/// use crate::auth::Auth;
///
/// async fn protected(auth: Auth) -> impl Responder {
///     format!("hello {}", auth.username)
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Auth {
    pub claims: Claims,
}

impl Auth {
    pub fn player_uuid(&self) -> &str {
        &self.claims.sub
    }

    pub fn username(&self) -> &str {
        &self.claims.username
    }

    pub fn claims(&self) -> &Claims {
        &self.claims
    }

    /// Verify a raw token string and construct `Auth` (useful outside extractor).
    pub fn from_token(token: &str) -> Result<Self, Error> {
        verify_jwt(token)
            .map(|claims| Self { claims })
            .map_err(|e| {
                // Do NOT leak internal error details to client; log internally if needed.
                // Distinguish expiry for better UX without leaking signature info.
                use jsonwebtoken::errors::ErrorKind;
                match e.kind() {
                    ErrorKind::ExpiredSignature => {
                        ErrorUnauthorized(serde_json::json!({"success": false, "message": "token expired"}))
                    }
                    _ => ErrorUnauthorized(serde_json::json!({"success": false, "message": "invalid or expired token"})),
                }
            })
    }
}

impl FromRequest for Auth {
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        // 1. Extract header
        let header_val = match req.headers().get("Authorization") {
            Some(v) => match v.to_str() {
                Ok(s) => s,
                Err(_) => {
                    return ready(Err(ErrorUnauthorized(serde_json::json!({
                        "success": false,
                        "message": "invalid Authorization header encoding"
                    }))));
                }
            },
            None => {
                return ready(Err(ErrorUnauthorized(serde_json::json!({
                    "success": false,
                    "message": "missing Authorization header"
                }))));
            }
        };

        // 2. Enforce `Bearer <token>` scheme (case-sensitive per RFC 6750, but also
        //    accept case-insensitive "bearer" to be lenient — then validate strictly).
        let token = if let Some(stripped) = header_val.strip_prefix("Bearer ") {
            stripped.trim()
        } else if let Some(stripped) = header_val.strip_prefix("bearer ") {
            // Non-canonical but common; still accept then verify normally.
            stripped.trim()
        } else {
            return ready(Err(ErrorUnauthorized(serde_json::json!({
                "success": false,
                "message": "invalid Authorization scheme, expected Bearer"
            }))));
        };

        if token.is_empty() {
            return ready(Err(ErrorUnauthorized(serde_json::json!({
                "success": false,
                "message": "empty Bearer token"
            }))));
        }

        // Basic structural sanity before crypto (3 segments)
        if token.matches('.').count() != 2 {
            return ready(Err(ErrorUnauthorized(serde_json::json!({
                "success": false,
                "message": "malformed token"
            }))));
        }

        // 3. Verify signature + claims (forgery prevention)
        ready(Auth::from_token(token))
    }
}

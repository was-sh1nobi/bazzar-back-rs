mod common;

use bazzar_back::auth::{Auth, create_jwt_with_role, is_revoked, revoke_player, revoke_token};
use bazzar_back::auth::{create_jwt, verify_jwt};
use bazzar_back::services::error::ServiceError;
use bazzar_back::services::items::version::{bump_version, get_version};
use bazzar_back::utils::password_verifier::{hash_password, verify_password};
use bazzar_back::utils::rate_limit::InMemoryRateLimiter;
use bazzar_back::utils::validation::{
    StrNum, parse_listing_amount, parse_listing_price, validate_material, validate_name,
    validate_password, validate_uuid,
};

#[test]
fn uuid_validation() {
    assert!(validate_uuid("123e4567-e89b-12d3-a456-426614174000"));
    assert!(!validate_uuid("not-a-uuid"));
    assert!(!validate_uuid(""));
}

#[test]
fn name_validation_trims_and_rejects_spaces() {
    assert_eq!(validate_name("  Steve ").unwrap(), "Steve");
    assert!(validate_name("a").is_err());
    assert!(validate_name("has space").is_err());
}

#[test]
fn password_length_policy() {
    assert!(validate_password("1234").is_ok());
    assert!(validate_password(&"x".repeat(64)).is_ok());
    assert!(validate_password("123").is_err());
    assert!(validate_password(&"x".repeat(65)).is_err());
}

#[test]
fn material_rules() {
    assert_eq!(validate_material(" DIAMOND ").unwrap(), "DIAMOND");
    assert!(validate_material("GOLD_INGOT").is_ok());
    assert!(validate_material("DIAMOND_2").is_ok());
    assert!(validate_material("diamond").is_err());
    assert!(validate_material("HAS SPACE").is_err());
    assert!(validate_material("HAS-DASH").is_err());
    assert!(validate_material("").is_err());
}

#[test]
fn amount_rules_both_shapes() {
    use StrNum::{Number, Text};
    assert_eq!(parse_listing_amount(&Text(" 3 ".into())).unwrap(), 3);
    assert_eq!(parse_listing_amount(&Number(5.0)).unwrap(), 5);
    assert_eq!(parse_listing_amount(&Text("1.9".into())).unwrap(), 1);
    assert_eq!(parse_listing_amount(&Text("64".into())).unwrap(), 64);
    assert!(parse_listing_amount(&Text("0".into())).is_err());
    assert!(parse_listing_amount(&Number(65.0)).is_err());
    assert!(parse_listing_amount(&Text("abc".into())).is_err());
    assert!(parse_listing_amount(&Text("".into())).is_err());
    assert!(parse_listing_amount(&Text("NaN".into())).is_err());
}

#[test]
fn price_rules_both_shapes() {
    use StrNum::{Number, Text};
    assert_eq!(
        parse_listing_price(&Text(" 1500.0 ".into())).unwrap(),
        "1500"
    );
    assert_eq!(parse_listing_price(&Number(99.5)).unwrap(), "99.5");
    assert!(parse_listing_price(&Text("1".repeat(21))).is_err());
    assert!(parse_listing_price(&Text("abc".into())).is_err());
    assert!(parse_listing_price(&Text("0".into())).is_err());
    assert!(parse_listing_price(&Number(0.0)).is_err());
    assert!(parse_listing_price(&Number(-5.0)).is_err());
    assert!(parse_listing_price(&Number(f64::NAN)).is_err());
    assert!(parse_listing_price(&Number(f64::INFINITY)).is_err());
}

#[test]
fn strnum_deserializes_both_json_shapes() {
    assert!(matches!(
        serde_json::from_str::<StrNum>("\"5\"").unwrap(),
        StrNum::Text(_)
    ));
    assert!(matches!(
        serde_json::from_str::<StrNum>("5").unwrap(),
        StrNum::Number(_)
    ));
    assert!(serde_json::from_str::<StrNum>("[1]").is_err());
}

#[test]
fn hash_and_verify_roundtrip() {
    let hash = hash_password("hunter2").unwrap();
    assert!(hash.starts_with("$argon2id$"));
    assert!(verify_password(&hash, "hunter2"));
    assert!(!verify_password(&hash, "hunter3"));
    assert!(!verify_password("not-a-hash", "hunter2"));
}

#[test]
fn bump_increments_and_get_reads() {
    let before = get_version();
    let after = bump_version();
    assert_eq!(after, before + 1);
    assert_eq!(get_version(), after);
}

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
    assert_eq!(
        ServiceError::Unauthorized("denied".into()).http_status(),
        401
    );
    assert_eq!(
        ServiceError::TooManyRequests("too many".into()).http_status(),
        429
    );
    assert_eq!(ServiceError::Internal("x".into()).http_status(), 500);
}

#[test]
fn internal_errors_never_leak_details() {
    let error = ServiceError::Internal("relation \"users\" does not exist".to_owned());
    assert_eq!(error.client_message(), "internal server error");
    assert!(!error.to_string().contains("relation"));
}

#[test]
fn validation_rejects_bad_input_before_any_db_work() {
    assert_eq!(validate_name("a").unwrap_err().http_status(), 422);
    assert!(matches!(
        validate_password("123").unwrap_err(),
        ServiceError::InvalidPassword(_)
    ));
}

#[test]
fn jwt_roundtrip_build_and_verify() {
    let token = create_jwt("123e4567-e89b-12d3-a456-426614174000", "Steve").expect("build token");
    let claims = verify_jwt(&token).expect("verify token");
    assert_eq!(claims.sub, "123e4567-e89b-12d3-a456-426614174000");
    assert_eq!(claims.username, "Steve");
}

#[test]
fn jwt_with_role_roundtrip() {
    let token = create_jwt_with_role(
        "123e4567-e89b-12d3-a456-426614174000",
        "AdminUser",
        Some("admin".to_string()),
    )
    .expect("build admin token");
    let claims = verify_jwt(&token).expect("verify token");
    assert_eq!(claims.role.as_deref(), Some("admin"));
}

#[test]
fn token_revocation_checks() {
    let player_uuid = "999e4567-e89b-12d3-a456-426614174000";
    let token = create_jwt(player_uuid, "RevokedPlayer").expect("token");
    let auth = Auth::from_token(&token).expect("valid before revocation");
    let jti = auth.claims.jti.clone().expect("has jti");

    assert!(!is_revoked(Some(&jti), player_uuid));
    revoke_token(&jti);
    assert!(is_revoked(Some(&jti), player_uuid));
    assert!(Auth::from_token(&token).is_err());

    let token2 = create_jwt(player_uuid, "RevokedPlayer").expect("token2");
    revoke_player(player_uuid);
    assert!(is_revoked(None, player_uuid));
    assert!(Auth::from_token(&token2).is_err());
}

#[test]
fn rate_limiter_throttles_after_threshold() {
    let limiter = InMemoryRateLimiter::new();
    let key = "test_user_key";
    let window = std::time::Duration::from_secs(60);

    assert!(limiter.check_and_record(key, 2, window).is_ok());
    assert!(limiter.check_and_record(key, 2, window).is_ok());
    let err = limiter.check_and_record(key, 2, window).unwrap_err();
    assert_eq!(err.http_status(), 429);

    limiter.reset(key);
    assert!(limiter.check_and_record(key, 2, window).is_ok());
}

#[test]
fn jwt_rejects_tampered_signature() {
    let mut token = create_jwt("123e4567-e89b-12d3-a456-426614174000", "Steve").unwrap();
    let last = token.pop().unwrap();
    token.push(if last == 'a' { 'b' } else { 'a' });
    assert!(verify_jwt(&token).is_err());
}

#[test]
fn jwt_rejects_wrong_alg_none() {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    let header = URL_SAFE_NO_PAD.encode(serde_json::json!({"alg":"none","typ":"JWT"}).to_string());
    let payload = URL_SAFE_NO_PAD.encode(
        serde_json::json!({
            "sub":"123e4567-e89b-12d3-a456-426614174000",
            "username":"Steve",
            "iat": chrono::Utc::now().timestamp(),
            "exp": (chrono::Utc::now()+chrono::Duration::hours(1)).timestamp(),
            "iss":"bazzar-back",
            "aud":"bazzar-client"
        })
        .to_string(),
    );
    assert!(verify_jwt(&format!("{header}.{payload}.")).is_err());
}

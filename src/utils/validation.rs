//! Shared request validation. Called once at the service boundary so the
//! same rules are not copy-pasted across every service function.

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

#[cfg(test)]
mod tests {
    use super::*;

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
}

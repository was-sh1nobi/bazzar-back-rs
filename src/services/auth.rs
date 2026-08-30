//! Auth business logic.
//!
//! Contract: every function returns `Result<TypedPayload, ServiceError>`.
//! No `StatusCode`, no `serde_json::Value` — HTTP mapping lives in
//! `src/controllers/mod.rs`. Argon2 work is offloaded to the blocking pool
//! so the async runtime is never stalled.

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::Serialize;
use tokio::task::spawn_blocking;

use crate::auth::{TTL_SECONDS, create_jwt};
use crate::entities::users;
use crate::services::error::{ServiceError, internal};
use crate::utils::password_verifier::{hash_password, verify_password};
use crate::utils::validation::{validate_name, validate_password, validate_uuid};

// ---------------------------------------------------------------------------
// Inputs
// ---------------------------------------------------------------------------

pub struct RegisterInput {
    pub uuid: String,
    pub name: String,
    pub password: String,
}

pub struct LoginInput {
    pub uuid: String,
    pub password: String,
}

pub struct ChangeInput {
    pub uuid: String,
    pub old_password: String,
    pub new_password: String,
}

// ---------------------------------------------------------------------------
// Outputs (serialized exactly as the client expects them)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterOutput {
    pub uuid: String,
    pub username: String,
    pub message: &'static str,
}

#[derive(Serialize)]
pub struct TokenOutput {
    pub token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusOutput {
    pub registered: bool,
    pub has_password: bool,
    pub username: Option<String>,
}

#[derive(Serialize)]
pub struct ChangeOutput {
    pub success: bool,
    pub message: &'static str,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn find_user_by_uuid(
    uuid: &str,
    db: &DatabaseConnection,
) -> Result<Option<users::Model>, ServiceError> {
    users::Entity::find()
        .filter(users::Column::Playeruuid.eq(uuid))
        .one(db)
        .await
        .map_err(|e| internal("auth: database query failed", e))
}

/// Run a CPU-bound closure on the blocking thread pool so Argon2 never
/// stalls the async runtime.
async fn blocking<T, F>(context: &'static str, f: F) -> Result<T, ServiceError>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    spawn_blocking(f)
        .await
        .map_err(|e| internal(context, e))
}

// ---------------------------------------------------------------------------
// Register
// ---------------------------------------------------------------------------

pub async fn register_service(
    input: RegisterInput,
    db: &DatabaseConnection,
) -> Result<RegisterOutput, ServiceError> {
    if !validate_uuid(&input.uuid) {
        return Err(ServiceError::MalformedUuid);
    }
    let name = validate_name(&input.name)?;
    validate_password(&input.password)?;

    if find_user_by_uuid(&input.uuid, db).await?.is_some() {
        return Err(ServiceError::AlreadyRegistered);
    }

    // Argon2id off the async runtime
    let pw = input.password.clone();
    let password_hash = blocking("register: hashing task failed", move || hash_password(&pw))
        .await?
        .map_err(|e| internal("register: hashing failed", e))?;

    let stmt = users::ActiveModel {
        playeruuid: Set(input.uuid.clone()),
        username: Set(name.clone()),
        balance: Set(0.0),
        password_hash: Set(Some(password_hash)),
        ..Default::default()
    };
    users::Entity::insert(stmt)
        .exec(db)
        .await
        .map_err(|e| internal("register: insert failed", e))?;

    Ok(RegisterOutput {
        uuid: input.uuid,
        username: name,
        message: "Created new user",
    })
}

// ---------------------------------------------------------------------------
// Login — now issues the JWT that the `Auth` extractor consumes
// ---------------------------------------------------------------------------

pub async fn login_service(
    input: LoginInput,
    db: &DatabaseConnection,
) -> Result<TokenOutput, ServiceError> {
    if !validate_uuid(&input.uuid) {
        return Err(ServiceError::MalformedUuid);
    }
    validate_password(&input.password)?;

    let user = find_user_by_uuid(&input.uuid, db)
        .await?
        .ok_or(ServiceError::NotFound)?;

    let Some(stored_hash) = user.password_hash.clone() else {
        return Err(ServiceError::InvalidCredentials);
    };

    // Argon2 verify off the async runtime
    let pw = input.password.clone();
    let ok = blocking("login: verify task failed", move || {
        verify_password(&stored_hash, &pw)
    })
    .await?;
    if !ok {
        return Err(ServiceError::InvalidCredentials);
    }

    let token = create_jwt(&user.playeruuid, &user.username)
        .map_err(|e| internal("login: token generation failed", e))?;

    Ok(TokenOutput {
        token,
        token_type: "Bearer",
        expires_in: TTL_SECONDS,
    })
}

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

pub async fn status_service(
    uuid: &str,
    db: &DatabaseConnection,
) -> Result<StatusOutput, ServiceError> {
    if !validate_uuid(uuid) {
        return Err(ServiceError::MalformedUuid);
    }
    Ok(match find_user_by_uuid(uuid, db).await? {
        None => StatusOutput {
            registered: false,
            has_password: false,
            username: None,
        },
        Some(u) => StatusOutput {
            registered: true,
            has_password: u.password_hash.is_some(),
            username: Some(u.username),
        },
    })
}

// ---------------------------------------------------------------------------
// Change password (requires `Auth` — uuid comes from the JWT claims)
// ---------------------------------------------------------------------------

pub async fn change_service(
    input: ChangeInput,
    db: &DatabaseConnection,
) -> Result<ChangeOutput, ServiceError> {
    if !validate_uuid(&input.uuid) {
        return Err(ServiceError::MalformedUuid);
    }
    validate_password(&input.new_password)?;

    let user = find_user_by_uuid(&input.uuid, db)
        .await?
        .ok_or(ServiceError::NotFound)?;

    // 404 if the user exists but never set a password
    let Some(stored_hash) = user.password_hash.clone() else {
        return Err(ServiceError::NotFound);
    };
    if input.old_password.is_empty() {
        return Err(ServiceError::InvalidCredentials);
    }

    // Verify old password (off the async runtime)
    let old = input.old_password.clone();
    let ok = blocking("change: verify task failed", move || {
        verify_password(&stored_hash, &old)
    })
    .await?;
    if !ok {
        return Err(ServiceError::InvalidCredentials);
    }

    // Hash new password (off the async runtime)
    let new_pw = input.new_password.clone();
    let new_hash = blocking("change: hashing task failed", move || hash_password(&new_pw))
        .await?
        .map_err(|e| internal("change: hashing failed", e))?;

    let mut am: users::ActiveModel = user.into();
    am.password_hash = Set(Some(new_hash));
    users::Entity::update(am)
        .exec(db)
        .await
        .map_err(|e| internal("change: update failed", e))?;

    Ok(ChangeOutput {
        success: true,
        message: "password changed",
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_rejects_bad_input_before_any_db_work() {
        // Pure validation — no DB connection needed.
        let err = validate_name("a").unwrap_err();
        assert_eq!(err.http_status(), 422);
        assert!(matches!(
            validate_password("123").unwrap_err(),
            ServiceError::InvalidPassword(_)
        ));
    }
}

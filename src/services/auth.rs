use actix_web::http::StatusCode;
use argon2::{
    Argon2,
    password_hash::{SaltString, PasswordHasher, rand_core::OsRng},
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde_json::{Value, json};
use crate::entities::users;
use crate::utils::uuid_validator::validate_uuid;

/// service returns status + any json — controller just forwards it
pub async fn login_service(username: &str) -> (StatusCode, Value) {
    if username.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            json!({"error": true, "me": "y"}),
        );
    }
    // success — any shape you want
    (StatusCode::OK, json!({"message": "hi"}))
    // other example: (StatusCode::UNAUTHORIZED, json!({"success": false, "message": "invalid password"}))
}

pub async fn register_service(uuid : &str , name : &str , password : &str , db : &DatabaseConnection) -> (StatusCode, Value) {
    // uuid validator
    if !validate_uuid(uuid) {
        return (StatusCode::UNPROCESSABLE_ENTITY, json!({"message" : "Malformed uuid"}));
    }
    // len validator
    let trimmed = name.trim();
    if trimmed.len() < 2 {
        return (StatusCode::UNPROCESSABLE_ENTITY, json!({"message" : "Name too short"}));
    }
   // whitespace validator
    if trimmed.chars().any(|c| c.is_whitespace()) {
        return (StatusCode::UNPROCESSABLE_ENTITY, json!({"message" : "Name must not contain spaces"}));
    }
   // password validator
    if password.trim().len() < 4 || password.trim().len() > 64 {
        return (StatusCode::UNPROCESSABLE_ENTITY, json!({"message" : "password is too short or too long"}))
    }
    // check for already registered
    let existing = match users::Entity::find()
        .filter(users::Column::Playeruuid.eq(uuid))
        .one(db)
        .await
    {
        Ok(v) => v,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, json!({"message": e.to_string()})),
    };
    if existing.is_some() {
        return (StatusCode::BAD_REQUEST, json!({"message" : "already registered"}));
    }
    // hasher
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default(); // Argon2id v19
    let password_hash = match argon2.hash_password(password.as_bytes(), &salt) {
        Ok(h) => h.to_string(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, json!({"message":"hashing failed"})),
    };
    // adding the new user
    let stmt = users::ActiveModel {
        playeruuid: Set(uuid.to_owned()),
        username: Set(trimmed.to_owned()),
        balance: Set(0.0),
        password_hash: Set(Some(password_hash)),
        ..Default::default()
    };
    if let Err(e) = users::Entity::insert(stmt).exec(db).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, json!({"message": e.to_string()}));
    }
    (StatusCode::CREATED, json!({"message" : "Created new user"}))
}



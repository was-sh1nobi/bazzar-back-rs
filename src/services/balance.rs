//! Balance business logic (guide §9).
//!
//! `GET /balance` per spec: always 200 for a valid uuid; auto-registers the
//! player at balance 0 if missing. Returns a typed payload — never the whole
//! user row (the old version serialized `password_hash` to clients!).

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use serde::Serialize;

use crate::services::error::{ServiceError, internal};
use crate::utils::register_if_absent::register_if_absent;
use crate::utils::validation::validate_uuid;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceOutput {
    pub player_uuid: String,
    pub balance: f64,
}

/// `GET /balance?player=uuid` — `getBalance` per guide §9.1.
pub async fn get_balance_service(
    uuid: &str,
    db: &DatabaseConnection,
) -> Result<BalanceOutput, ServiceError> {
    if !validate_uuid(uuid) {
        return Err(ServiceError::MalformedUuid);
    }

    // Spec: auto-register at 0 when the player is unknown.
    register_if_absent(db, uuid, None)
        .await
        .map_err(|e| internal("balance: auto-register failed", e))?;

    let stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"SELECT balance FROM users WHERE playeruuid = $1"#,
        [uuid.into()],
    );
    let balance = db
        .query_one(stmt)
        .await
        .map_err(|e| internal("balance: database query failed", e))?
        .and_then(|r| r.try_get::<f64>("", "balance").ok())
        .unwrap_or(0.0);
    Ok(BalanceOutput {
        player_uuid: uuid.to_owned(),
        balance,
    })
}

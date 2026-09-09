//! `POST /items/add` — create an auction listing (guide §10.1, Node `routes/items.ts:84`).
//!
//! SECURITY: unlike the Node original, the body carries **no
//! `playeruuid`/`playerName`** — seller identity comes ONLY from the verified
//! JWT (`auth.claims.sub`), so callers cannot list on behalf of someone else.
//!
//! The seller's username is resolved INSIDE the INSERT itself
//! (`INSERT ... SELECT u.username FROM users u`) — one round-trip instead of
//! the Node SELECT-name-then-INSERT. Raw `Statement` is used because SeaORM's
//! `ActiveModel` insert cannot express `INSERT ... SELECT` (same raw-SQL style
//! as `items_main`/`stats`/`items_owned`).

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use serde::Deserialize;

use crate::services::error::{ServiceError, internal};
use crate::services::items::version::bump_version;
use crate::utils::provenance::{is_blocked, normalize_origin, normalize_source};
use crate::utils::validation::{
    StrNum, parse_listing_amount, parse_listing_price, validate_material, validate_uuid,
};

/// Dual-shape body (guide §10.1): `amount`/`price` accept legacy strings
/// (`"5"` / `"120.5"`) OR modern numerics (`5` / `80`) via the untagged
/// `StrNum` — validated by `parse_listing_amount` / `parse_listing_price`.
#[derive(Deserialize)]
pub struct ItemsAddBody {
    material: String,
    amount: StrNum,
    price: StrNum,
    /// Optional + defaulted like Node: missing/unknown → "player"/"survival",
    /// then the provenance gate (guide §7) rejects blocked combos.
    #[serde(default)]
    source: String,
    #[serde(default)]
    origin: String,
}

/// Validate the whole body and insert the listing. Returns the new listing id.
///
/// Round-trips: **1** (INSERT..SELECT..RETURNING — username join included).
pub async fn create_listing(
    body: ItemsAddBody,
    db: &DatabaseConnection,
    uuid: &str,
) -> Result<i64, ServiceError> {
    // JWT `sub` is server-signed; the check is defensive (non-uuid sub must
    // never reach the DB query).
    if !validate_uuid(uuid) {
        return Err(ServiceError::MalformedUuid);
    }

    let material = validate_material(&body.material)?;
    let amount = parse_listing_amount(&body.amount)?;
    let price = parse_listing_price(&body.price)?;

    // Provenance / anti-dupe gate (guide §7) — checked on write here AND on
    // every read in the feed queries.
    let source = normalize_source(Some(&body.source));
    let origin = normalize_origin(Some(&body.origin));
    if is_blocked(&source, &origin) {
        // Same message as Node `routes/items.ts:96`
        return Err(ServiceError::InvalidInput(format!(
            "blocked origin: {origin} (source={source})"
        )));
    }

    // ONE round-trip: resolve the seller's username from `users` inside the
    // INSERT (inner-join style `INSERT ... SELECT`). If no users row exists for
    // this JWT sub, 0 rows are returned -> NotFound ("not registered").
    // status defaults to 'AVAILABLE' per schema; stated explicitly for clarity.
    let stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"INSERT INTO items (seller, seller_playeruuid, material, amount, price, source, origin, status)
           SELECT u.username, u.playeruuid, $2, $3, $4, $5, $6, 'AVAILABLE'
           FROM users u
           WHERE u.playeruuid = $1
           RETURNING id"#,
        vec![
            uuid.into(),
            material.into(),
            amount.into(),
            price.into(),
            source.into(),
            origin.into(),
        ],
    );

    let row = db
        .query_one(stmt)
        .await
        .map_err(|e| internal("items add: insert failed", e))?
        .ok_or(ServiceError::NotFound)?;

    // id is INTEGER (int4) — read as i32 then widen
    let id: i32 = row
        .try_get("", "id")
        .map_err(|e| internal("items add: decode id", e))?;

    // Every AH mutation bumps the global version (guide §6) — wakes /items/poll.
    bump_version();

    Ok(id as i64)
}

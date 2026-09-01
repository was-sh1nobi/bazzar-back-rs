//! JWT-scoped ownership feeds: `/items/active`, `/items/bought`, `/items/history`
//! (spec §10 #4-6, Node `routes/items.ts:57/66/75` + `services/items/items.ts:48/70/89`).
//!
//! SECURITY: unlike the Node original, there is **no `player` query param** —
//! the caller's identity comes ONLY from the verified JWT (`auth.claims.sub`),
//! so no caller can ever read another player's data (guide §8.3/§16).
//!
//! All three return the same envelope as `GET /items` (`ItemsMainOutput` shape)
//! with the real in-memory mutation version, so the plugin's poll/refresh logic
//! works unchanged against these feeds too.

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement};
use serde::Serialize;

use crate::services::error::{internal, ServiceError};
use crate::services::items::version::get_version;
use crate::utils::validation::validate_uuid;

#[derive(Serialize)]
pub struct OwnedItem {
    id: i64,
    seller_name: String,
    /// UUID of the buyer — `Some` only for history/bought rows (history.buyer is
    /// a UUID, unlike items.seller which is a name — guide §5.3).
    #[serde(skip_serializing_if = "Option::is_none")]
    buyer: Option<String>,
    material: String,
    amount: i64,
    price: f64,
}

#[derive(Serialize)]
pub struct ItemsOwnedOutput {
    page: i64,
    page_size: i64,
    total: i64,
    total_pages: i64,
    version: i64,
    items: Vec<OwnedItem>,
}

/// Provenance gate — must be ported exactly (guide §7, anti-dupe).
const PROVENANCE_SQL: &str =
    "status = 'AVAILABLE' \
     AND LOWER(COALESCE(source,'player')) NOT IN ('creative','admin','command') \
     AND LOWER(COALESCE(origin,'survival')) NOT IN ('creative','command','unknown')";

/// Owner-match fragment. `history.seller`/`items.seller` store a NAME (guide
/// §5.3), but identity here is strictly the JWT uuid: we match the direct
/// `seller_playeruuid` column (2026-09-01 migration, same as `stats.rs`) FIRST,
/// and fall back to the legacy uuid-as-seller / uuid->username join ONLY for
/// rows whose `seller_playeruuid` was never backfilled. This prevents two
/// accounts sharing a username from seeing each other's data — unlike the Node
/// original, where the name-join alone would leak across same-named accounts.
const OWNER_MATCH_SQL: &str =
    "(seller_playeruuid = $1 \
      OR (seller_playeruuid IS NULL AND seller = $1) \
      OR (seller_playeruuid IS NULL AND seller IN (SELECT username FROM users WHERE playeruuid = $1)))";

/// Shared pagination + envelope runner (mirrors `items_main`: page 0-based,
/// pageSize default 45 clamped 1..100, `COUNT(*) OVER()` window for total).
async fn run_page(
    db: &DatabaseConnection,
    sql: String,
    mut values: Vec<sea_orm::Value>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<ItemsOwnedOutput, ServiceError> {
    let page = page.unwrap_or(0).max(0);
    let limit = page_size.unwrap_or(45).clamp(1, 100);
    values.push(limit.into());
    values.push((page * limit).into());

    let stmt = Statement::from_sql_and_values(DbBackend::Postgres, sql, values);
    let rows = db
        .query_all(stmt)
        .await
        .map_err(|e| internal("items owned: query failed", e))?;

    let total: i64 = rows
        .first()
        .map(|r| r.try_get("", "total").unwrap_or(0))
        .unwrap_or(0);
    let total_pages = if total == 0 { 0 } else { (total + limit - 1) / limit };

    let items: Vec<OwnedItem> = rows.into_iter().map(map_row).collect();

    Ok(ItemsOwnedOutput {
        page,
        page_size: limit,
        total,
        total_pages,
        version: get_version(),
        items,
    })
}

fn map_row(r: QueryResult) -> OwnedItem {
    OwnedItem {
        // id is INTEGER (int4) — read as i32 then widen
        id: r.try_get::<Option<i32>>("", "id").unwrap_or(None).unwrap_or(0) as i64,
        seller_name: r.try_get("", "seller_name").unwrap_or_default(),
        // buyer is a nullable TEXT uuid column (NULL for /items/active rows)
        buyer: r.try_get::<Option<String>>("", "buyer").unwrap_or(None),
        material: r.try_get("", "material").unwrap_or_default(),
        amount: r.try_get::<Option<i32>>("", "amount").unwrap_or(None).unwrap_or(0) as i64,
        // price stored as TEXT, CAST to DOUBLE PRECISION on read (guide §5.3)
        price: r.try_get::<Option<f64>>("", "price").unwrap_or(None).unwrap_or(0.0),
    }
}

fn require_uuid(player_uuid: &str) -> Result<(), ServiceError> {
    if !validate_uuid(player_uuid) {
        // JWT `sub` is server-signed, so this is defensive only — a tampered or
        // legacy token with a non-uuid sub must not widen the query.
        return Err(ServiceError::MalformedUuid);
    }
    Ok(())
}

/// `GET /items/active` — the caller's AVAILABLE, provenance-clean listings
/// (Node `getBySeller`, `items.ts:48`).
pub async fn get_items_active_service(
    page: Option<i64>,
    page_size: Option<i64>,
    player_uuid: &str,
    db: &DatabaseConnection,
) -> Result<ItemsOwnedOutput, ServiceError> {
    require_uuid(player_uuid)?;

    let sql = format!(
        r#"SELECT id, seller AS seller_name, NULL::text AS buyer, material, amount,
                  CAST(price AS DOUBLE PRECISION) AS price, COUNT(*) OVER() AS total
           FROM items
           WHERE {PROVENANCE_SQL} AND {OWNER_MATCH_SQL}
           ORDER BY id
           LIMIT $2 OFFSET $3"#
    );

    run_page(db, sql, vec![player_uuid.into()], page, page_size).await
}

/// `GET /items/bought` — history rows where the caller was the buyer
/// (Node `getByBuyer`, `items.ts:70`). `buyer` is already a UUID (guide §5.3).
pub async fn get_items_bought_service(
    page: Option<i64>,
    page_size: Option<i64>,
    player_uuid: &str,
    db: &DatabaseConnection,
) -> Result<ItemsOwnedOutput, ServiceError> {
    require_uuid(player_uuid)?;

    let sql = format!(
        r#"SELECT id, seller AS seller_name, buyer, material, amount,
                  CAST(price AS DOUBLE PRECISION) AS price, COUNT(*) OVER() AS total
           FROM history
           WHERE buyer = $1
           ORDER BY id DESC
           LIMIT $2 OFFSET $3"#
    );

    run_page(db, sql, vec![player_uuid.into()], page, page_size).await
}

/// `GET /items/history` — everything the caller sold or bought, newest first
/// (Node `getHistory`, `items.ts:89`).
pub async fn get_items_history_service(
    page: Option<i64>,
    page_size: Option<i64>,
    player_uuid: &str,
    db: &DatabaseConnection,
) -> Result<ItemsOwnedOutput, ServiceError> {
    require_uuid(player_uuid)?;

    let sql = format!(
        r#"SELECT id, seller AS seller_name, buyer, material, amount,
                  CAST(price AS DOUBLE PRECISION) AS price, COUNT(*) OVER() AS total
           FROM history
           WHERE buyer = $1 OR {OWNER_MATCH_SQL}
           ORDER BY id DESC
           LIMIT $2 OFFSET $3"#
    );

    run_page(db, sql, vec![player_uuid.into()], page, page_size).await
}


use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use serde::Serialize;

use crate::services::error::{internal, ServiceError};
use crate::utils::validation::validate_uuid;

#[derive(Debug, Serialize)]
pub struct StatsOutput {
    pub total_listings: i64,
    pub active_listings: i64,
    pub total_sold: i64,
    pub my_listings: i64,
    pub my_sold: i64,
    pub my_purchases: i64,
}

/// Single-query stats — 1 round-trip, 6 scalar sub-selects (mirrors Node `getStats` in `services/items/items.ts:112`).
/// Uses `seller_playeruuid` direct column (added 2026-09-01) with fallback to legacy `seller` name join.
pub async fn get_stats_service(
    player_uuid: &str,
    db: &DatabaseConnection,
    
) -> Result<StatsOutput, ServiceError> {
    if !validate_uuid(player_uuid) {
        return Err(ServiceError::MalformedUuid);
    }

    // All counts in ONE query -> atomic snapshot, ~6x less latency than 6 separate COUNTs.
    // Provenance filter matches Node: `status='AVAILABLE' AND LOWER(COALESCE(source,'player')) NOT IN (...)`
    let stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        SELECT
            (SELECT COUNT(*) FROM items)::bigint AS total_listings,
            (SELECT COUNT(*) FROM items WHERE status = 'AVAILABLE'
                AND LOWER(COALESCE(source,'player')) NOT IN ('creative','admin','command')
                AND LOWER(COALESCE(origin,'survival')) NOT IN ('creative','command','unknown'))::bigint AS active_listings,
            (SELECT COUNT(*) FROM history)::bigint AS total_sold,
            (SELECT COUNT(*) FROM items WHERE seller_playeruuid = $1 OR seller IN (SELECT username FROM users WHERE playeruuid = $1))::bigint AS my_listings,
            (SELECT COUNT(*) FROM history WHERE seller_playeruuid = $1 OR seller = $1 OR seller IN (SELECT username FROM users WHERE playeruuid = $1))::bigint AS my_sold,
            (SELECT COUNT(*) FROM history WHERE buyer = $1)::bigint AS my_purchases
        "#,
        [player_uuid.into()],
    );

    let row = db
        .query_one(stmt)
        .await
        .map_err(|e| internal("stats: query failed", e))?
        .ok_or_else(|| internal("stats: empty row", "no row"))?;

    let total_listings: i64 = row.try_get("", "total_listings").map_err(|e| internal("stats: decode total_listings", e))?;
    let active_listings: i64 = row.try_get("", "active_listings").map_err(|e| internal("stats: decode active_listings", e))?;
    let total_sold: i64 = row.try_get("", "total_sold").map_err(|e| internal("stats: decode total_sold", e))?;
    let my_listings: i64 = row.try_get("", "my_listings").map_err(|e| internal("stats: decode my_listings", e))?;
    let my_sold: i64 = row.try_get("", "my_sold").map_err(|e| internal("stats: decode my_sold", e))?;
    let my_purchases: i64 = row.try_get("", "my_purchases").map_err(|e| internal("stats: decode my_purchases", e))?;

    Ok(StatsOutput {
        total_listings,
        active_listings,
        total_sold,
        my_listings,
        my_sold,
        my_purchases,
    })
}

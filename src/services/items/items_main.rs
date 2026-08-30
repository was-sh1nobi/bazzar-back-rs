use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TryGetable};
use serde::{Deserialize, Serialize};
use crate::services::error::{internal, ServiceError};
#[derive(Deserialize , Serialize)]
struct Items{
    id : i64 ,
    seller_name : String,
    material : String,
    amount : i64,
    price : f64,
}
#[derive(Deserialize , Serialize)]
pub struct ItemsMainOutput {
    page : i64,
    page_size : i64,
    total : i64,
    total_pages: i64,
    version : i64,
    items : Vec<Items>
}

/// Defaults mirror Node `items.ts:19 parsePage` — page 0, pageSize 45, clamped 1..100, 0-based offset.
/// Single query with `COUNT(*) OVER()` gives total + page in 1 round-trip (faster than 2x COUNT + SELECT).
pub async fn get_items_main_service(
    page: Option<i64>,
    page_size: Option<i64>,
    uuid: Option<&str>,
    db: &DatabaseConnection,
) -> Result<ItemsMainOutput, ServiceError> {
    let page = page.unwrap_or(0).max(0);
    let limit = page_size.unwrap_or(45).clamp(1, 100);
    let offset = page * limit;

    // Provenance + status filter (Node `items.ts:20`)
    let mut where_sql = String::from(
        "status = 'AVAILABLE' \
         AND LOWER(COALESCE(source,'player')) NOT IN ('creative','admin','command') \
         AND LOWER(COALESCE(origin,'survival')) NOT IN ('creative','command','unknown')",
    );
    let mut values: Vec<sea_orm::Value> = Vec::new();

    if let Some(u) = uuid {
        // Exclude own listings: seller NOT IN (SELECT username ...) AND seller <> $1
        // $1 will be uuid
        where_sql.push_str(" AND seller NOT IN (SELECT username FROM users WHERE playeruuid = $1) AND seller <> $1");
        values.push(u.into());
    }

    // Single-query: data + total via window function — no separate COUNT(*)
    // If total=0, window returns no rows, so we handle empty case below.
    let next_idx = values.len() + 1; // for LIMIT
    let sql = format!(
        r#"SELECT id, seller AS seller_name, material, amount, CAST(price AS REAL) AS price, COUNT(*) OVER() AS total
           FROM items WHERE {where_sql} ORDER BY id LIMIT ${next_idx} OFFSET ${next_idx_plus1}"#,
        next_idx_plus1 = next_idx + 1
    );
    values.push(limit.into());
    values.push(offset.into());

    let stmt = Statement::from_sql_and_values(DbBackend::Postgres, sql, values);

    let rows = db
        .query_all(stmt)
        .await
        .map_err(|e| internal("items main: query failed", e))?;

    // total from first row's window count, or 0 if no rows
    let total: i64 = if let Some(first) = rows.first() {
        first.try_get("", "total").unwrap_or(0)
    } else {
        0
    };
    // total_pages = ceil(total / limit), at least 1 if you want Node's Math.ceil(total/pageSize) with fallback 1
    let total_pages = if total == 0 { 0 } else { (total + limit - 1) / limit };

    let items: Vec<Items> = rows
        .into_iter()
        .map(|r| Items {
            id: r.try_get("", "id").unwrap_or(0),
            seller_name: r.try_get("", "seller_name").unwrap_or_default(),
            material: r.try_get("", "material").unwrap_or_default(),
            amount: r.try_get::<Option<i32>>("", "amount").unwrap_or(None).unwrap_or(0) as i64,
            price: r.try_get::<Option<String>>("", "price")
                .unwrap_or(None)
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0),
        })
        .collect();

    // version is in-memory counter in Node `db.ts:125`; Rust keeps 0 until you add mutationVersion table
    let version = 0;

    Ok(ItemsMainOutput {
        page,
        page_size: limit,
        total,
        total_pages,
        version,
        items,
    })





}
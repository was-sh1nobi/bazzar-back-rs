//! Shop catalog, buy, and sell services (guide §10.2, §12.3, §13).

use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use serde::{Deserialize, Serialize};

use crate::services::error::{ServiceError, internal};
use crate::utils::register_if_absent::register_if_absent;
use crate::utils::validation::{
    StrNum, parse_shop_buy_amount, parse_shop_sell_amount, validate_uuid,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShopCatalogItem {
    pub id: String,
    pub material: String,
    pub category: String,
    pub buy: f64,
    pub sell: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ShopCatalogResponse {
    pub version: i64,
    #[serde(rename = "lastUpdated")]
    pub last_updated: Option<String>,
    pub items: Vec<ShopCatalogItem>,
}

#[derive(Debug, Deserialize)]
pub struct ShopBuyBody {
    #[serde(default, rename = "itemId")]
    pub item_id: Option<String>,
    #[serde(default)]
    pub material: Option<String>,
    pub amount: Option<StrNum>,
}

#[derive(Debug, Serialize)]
pub struct ShopBuyOutput {
    pub success: bool,
    pub balance: f64,
    pub cost: f64,
    pub amount: i32,
    pub material: String,
    #[serde(rename = "itemId")]
    pub item_id: String,
}

#[derive(Debug, Deserialize)]
pub struct ShopSellBody {
    #[serde(default, rename = "itemId")]
    pub item_id: Option<String>,
    #[serde(default)]
    pub material: Option<String>,
    pub amount: Option<StrNum>,
}

#[derive(Debug, Serialize)]
pub struct ShopSellOutput {
    pub success: bool,
    pub balance: f64,
    pub gain: f64,
    pub amount: i32,
    pub material: String,
    #[serde(rename = "itemId")]
    pub item_id: String,
}

const ITEMS_FILE_PATH: &str = "src/data/items.json";

/// Seed shop_items from src/data/items.json if the table is empty (guide §13).
pub async fn seed_shop_items_if_needed(db: &DatabaseConnection) -> Result<(), ServiceError> {
    let stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT COUNT(*)::bigint AS cnt FROM shop_items",
        [],
    );
    match db.query_one(stmt).await {
        Ok(Some(row)) => {
            let count: i64 = row.try_get("", "cnt").unwrap_or(0);
            if count > 0 {
                return Ok(());
            }
        }
        _ => return Ok(()),
    }

    let items = load_items_from_file()?;
    for item in items {
        let insert_stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"INSERT INTO shop_items (id, material, category, buy, sell)
               VALUES ($1, $2, $3, $4, $5)
               ON CONFLICT (id) DO NOTHING"#,
            [
                item.id.into(),
                item.material.into(),
                item.category.into(),
                item.buy.into(),
                item.sell.into(),
            ],
        );
        let _ = db.execute(insert_stmt).await;
    }
    Ok(())
}

fn load_items_from_file() -> Result<Vec<ShopCatalogItem>, ServiceError> {
    let raw = std::fs::read_to_string(ITEMS_FILE_PATH)
        .map_err(|e| internal("shop catalog: failed to read items file", e))?;
    let parsed: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| internal("shop catalog: failed to parse items JSON", e))?;
    let items_array = parsed
        .get("items")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            internal(
                "shop catalog: missing items array in items.json",
                "format error",
            )
        })?;

    let items: Vec<ShopCatalogItem> = items_array
        .iter()
        .filter_map(|val| serde_json::from_value::<ShopCatalogItem>(val.clone()).ok())
        .collect();
    Ok(items)
}

/// GET /shop/catalog — returns catalog with version and lastUpdated timestamp (guide §13).
pub async fn get_shop_catalog_service(
    db: &DatabaseConnection,
) -> Result<ShopCatalogResponse, ServiceError> {
    let _ = seed_shop_items_if_needed(db).await;

    let metadata = std::fs::metadata(ITEMS_FILE_PATH).ok();
    let mtime_secs = metadata
        .as_ref()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or_else(|| Utc::now().timestamp());

    let last_updated = DateTime::from_timestamp(mtime_secs, 0).map(|dt| dt.to_rfc3339());

    let items = load_items_from_file()?;

    Ok(ShopCatalogResponse {
        version: mtime_secs,
        last_updated,
        items,
    })
}

/// POST /shop/catalog/reload — reloads DB from src/data/items.json (guide §13).
pub async fn reload_shop_catalog_service(db: &DatabaseConnection) -> Result<usize, ServiceError> {
    let items = load_items_from_file()?;
    let total = items.len();

    let delete_stmt =
        Statement::from_sql_and_values(DbBackend::Postgres, "DELETE FROM shop_items", []);
    db.execute(delete_stmt)
        .await
        .map_err(|e| internal("shop reload: delete failed", e))?;

    for item in &items {
        let insert_stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"INSERT INTO shop_items (id, material, category, buy, sell)
               VALUES ($1, $2, $3, $4, $5)
               ON CONFLICT (id) DO NOTHING"#,
            [
                item.id.clone().into(),
                item.material.clone().into(),
                item.category.clone().into(),
                item.buy.into(),
                item.sell.into(),
            ],
        );
        db.execute(insert_stmt)
            .await
            .map_err(|e| internal("shop reload: insert failed", e))?;
    }

    Ok(total)
}

/// POST /shop/buy — buy from server shop (guide §10.2, §12.3).
pub async fn shop_buy_service(
    uuid: &str,
    body: ShopBuyBody,
    db: &DatabaseConnection,
) -> Result<ShopBuyOutput, ServiceError> {
    if !validate_uuid(uuid) {
        return Err(ServiceError::MalformedUuid);
    }

    let amount = parse_shop_buy_amount(body.amount.as_ref())?;

    let identifier = body
        .item_id
        .as_deref()
        .or(body.material.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ServiceError::InvalidInput("itemId or material required".to_owned()))?;

    let stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT id, material, buy, sell FROM shop_items WHERE id = $1 OR UPPER(material) = UPPER($1) LIMIT 1",
        [identifier.into()],
    );
    let item_row = db
        .query_one(stmt)
        .await
        .map_err(|e| internal("shop buy: item lookup failed", e))?
        .ok_or(ServiceError::ShopItemNotFound)?;

    let item_id: String = item_row
        .try_get("", "id")
        .map_err(|e| internal("shop buy: decode id", e))?;
    let material: String = item_row
        .try_get("", "material")
        .map_err(|e| internal("shop buy: decode material", e))?;
    let buy_price: f64 = item_row
        .try_get("", "buy")
        .map_err(|e| internal("shop buy: decode buy", e))?;

    let cost = buy_price * amount as f64;
    if !cost.is_finite() || cost <= 0.0 {
        return Err(ServiceError::InvalidItemPrice);
    }

    register_if_absent(db, uuid, None)
        .await
        .map_err(|e| internal("shop buy: auto-register failed", e))?;

    let deduct_stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"UPDATE users SET balance = balance - $2
           WHERE playeruuid = $1 AND balance - $2 >= 0
           RETURNING balance AS balance"#,
        [uuid.into(), cost.into()],
    );

    let new_balance: f64 = match db
        .query_one(deduct_stmt)
        .await
        .map_err(|e| internal("shop buy: deduction failed", e))?
    {
        Some(row) => row
            .try_get("", "balance")
            .map_err(|e| internal("shop buy: decode balance", e))?,
        None => {
            let bal_stmt = Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"SELECT balance FROM users WHERE playeruuid = $1"#,
                [uuid.into()],
            );
            let current_bal = db
                .query_one(bal_stmt)
                .await
                .map_err(|e| internal("shop buy: balance check failed", e))?
                .and_then(|r| r.try_get::<f64>("", "balance").ok())
                .unwrap_or(0.0);
            return Err(ServiceError::InvalidInput(format!(
                "insufficient funds (balance {current_bal}, cost {cost})"
            )));
        }
    };

    let now = Utc::now().timestamp();
    let audit_stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"INSERT INTO shop_purchases (playeruuid, "itemId", material, amount, "priceEach", "totalCost", created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        [
            uuid.into(),
            item_id.clone().into(),
            material.clone().into(),
            amount.into(),
            buy_price.into(),
            cost.into(),
            now.into(),
        ],
    );
    let _ = db.execute(audit_stmt).await;

    Ok(ShopBuyOutput {
        success: true,
        balance: new_balance,
        cost,
        amount,
        material,
        item_id,
    })
}

/// POST /shop/sell — sell to server shop (guide §10.2, §12.3).
pub async fn shop_sell_service(
    uuid: &str,
    body: ShopSellBody,
    db: &DatabaseConnection,
) -> Result<ShopSellOutput, ServiceError> {
    if !validate_uuid(uuid) {
        return Err(ServiceError::MalformedUuid);
    }

    let amount = parse_shop_sell_amount(body.amount.as_ref())?;

    let identifier = body
        .item_id
        .as_deref()
        .or(body.material.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ServiceError::InvalidInput("itemId or material required".to_owned()))?;

    let stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT id, material, buy, sell FROM shop_items WHERE id = $1 OR UPPER(material) = UPPER($1) LIMIT 1",
        [identifier.into()],
    );
    let item_row = db
        .query_one(stmt)
        .await
        .map_err(|e| internal("shop sell: item lookup failed", e))?
        .ok_or(ServiceError::ShopItemNotFound)?;

    let item_id: String = item_row
        .try_get("", "id")
        .map_err(|e| internal("shop sell: decode id", e))?;
    let material: String = item_row
        .try_get("", "material")
        .map_err(|e| internal("shop sell: decode material", e))?;
    let sell_price: f64 = item_row
        .try_get("", "sell")
        .map_err(|e| internal("shop sell: decode sell", e))?;

    if sell_price <= 0.0 {
        return Err(ServiceError::InvalidInput("not sellable".to_owned()));
    }

    let gain = sell_price * amount as f64;
    if !gain.is_finite() || gain <= 0.0 {
        return Err(ServiceError::InvalidInput("not sellable".to_owned()));
    }

    register_if_absent(db, uuid, None)
        .await
        .map_err(|e| internal("shop sell: auto-register failed", e))?;

    let credit_stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"UPDATE users SET balance = balance + $2
           WHERE playeruuid = $1
           RETURNING balance AS balance"#,
        [uuid.into(), gain.into()],
    );

    let new_balance: f64 = db
        .query_one(credit_stmt)
        .await
        .map_err(|e| internal("shop sell: credit failed", e))?
        .and_then(|r| r.try_get("", "balance").ok())
        .unwrap_or(gain);

    Ok(ShopSellOutput {
        success: true,
        balance: new_balance,
        gain,
        amount,
        material,
        item_id,
    })
}

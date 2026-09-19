use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use serde::Serialize;

use crate::services::error::{ServiceError, internal};
use crate::services::items::version::bump_version;
use crate::utils::validation::validate_uuid;

#[derive(Serialize)]
pub struct ItemsRetrieveOutput {
    pub success: bool,
    pub material: Option<String>,
    pub amount: Option<i32>,
    pub message: String,
    pub bump: i64,
}

pub async fn get_items_retrieve_service(
    db: &DatabaseConnection,
    wanted_id: i64,
    me_uuid: &str,
) -> Result<ItemsRetrieveOutput, ServiceError> {
    if !validate_uuid(me_uuid) {
        return Err(ServiceError::MalformedUuid);
    }
    if !(1..=i32::MAX as i64).contains(&wanted_id) {
        return Err(ServiceError::ItemNotFound);
    }

    let stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"SELECT id, seller, seller_playeruuid, material, amount, status::text AS status
           FROM items WHERE id = $1"#,
        [wanted_id.into()],
    );

    let row = db
        .query_one(stmt)
        .await
        .map_err(|e| internal("items retrieve: load item", e))?
        .ok_or(ServiceError::ItemNotFound)?;

    let status: String = row
        .try_get("", "status")
        .map_err(|e| internal("items retrieve: decode status", e))?;
    let seller: String = row
        .try_get("", "seller")
        .map_err(|e| internal("items retrieve: decode seller", e))?;
    let seller_uuid: Option<String> = row.try_get("", "seller_playeruuid").unwrap_or(None);
    let material: Option<String> = row.try_get("", "material").unwrap_or(None);
    let amount: Option<i32> = row.try_get("", "amount").unwrap_or(None);

    if status != "AVAILABLE" {
        return Err(ServiceError::InvalidInput(
            "item is not available to retrieve".to_owned(),
        ));
    }

    let is_owner = if let Some(ref s_uuid) = seller_uuid {
        s_uuid == me_uuid
    } else if seller == me_uuid {
        true
    } else {
        let user_stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"SELECT playeruuid FROM users WHERE username = $1"#,
            [seller.into()],
        );
        let user_row = db
            .query_one(user_stmt)
            .await
            .map_err(|e| internal("items retrieve: load owner", e))?;
        let owner_uuid: Option<String> = user_row.and_then(|r| r.try_get("", "playeruuid").ok());
        owner_uuid.as_deref() == Some(me_uuid)
    };

    if !is_owner {
        return Err(ServiceError::InvalidInput(
            "you can only retrieve your own listings".to_owned(),
        ));
    }

    let update_stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"UPDATE items SET status = 'REMOVED' WHERE id = $1 AND status = 'AVAILABLE'"#,
        [wanted_id.into()],
    );

    let res = db
        .execute(update_stmt)
        .await
        .map_err(|e| internal("items retrieve: remove item", e))?;

    if res.rows_affected() == 0 {
        return Err(ServiceError::InvalidInput(
            "item is not available to retrieve".to_owned(),
        ));
    }

    let bump = bump_version();
    Ok(ItemsRetrieveOutput {
        success: true,
        material,
        amount,
        message: format!("item {wanted_id} retrieved"),
        bump,
    })
}

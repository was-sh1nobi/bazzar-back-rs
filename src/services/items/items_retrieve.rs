use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::Serialize;

use crate::entities::items::{Column as ItemColumn, Entity as Items, ItemStatus};
use crate::entities::users::Entity as Users;
use crate::services::error::{ServiceError, internal};
use crate::services::items::version::bump_version;
use crate::utils::validation::validate_uuid;

#[derive(Serialize)]
pub struct ItemsRetrieveOutput {
    success: bool,
    material: Option<String>,
    amount: Option<i32>,
    message: String,
    bump: i64,
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

    let Some(item) = Items::find_by_id(wanted_id as i32)
        .one(db)
        .await
        .map_err(|e| internal("items retrieve: load item", e))?
    else {
        return Err(ServiceError::ItemNotFound);
    };

    let owner_uuid = match item.seller_playeruuid.as_deref() {
        Some(uuid) => Some(uuid.to_owned()),
        None => Users::find()
            .filter(crate::entities::users::Column::Username.eq(item.seller.as_str()))
            .one(db)
            .await
            .map_err(|e| internal("items retrieve: load owner", e))?
            .map(|owner| owner.playeruuid),
    };

    if owner_uuid.as_deref() != Some(me_uuid) {
        return Err(ServiceError::InvalidInput(
            "you can only retrieve your own listings".to_owned(),
        ));
    }

    let removed = Items::update_many()
        .col_expr(ItemColumn::Status, ItemStatus::Removed.into())
        .filter(
            ItemColumn::Id
                .eq(wanted_id as i32)
                .and(ItemColumn::Status.eq(ItemStatus::Available)),
        )
        .exec(db)
        .await
        .map_err(|e| internal("items retrieve: remove item", e))?;

    if removed.rows_affected == 0 {
        return Err(ServiceError::InvalidInput(
            "item is not available to retrieve".to_owned(),
        ));
    }

    let bump = bump_version();
    Ok(ItemsRetrieveOutput {
        success: true,
        material: item.material,
        amount: item.amount,
        message: format!("item {} retrieved", item.id),
        bump,
    })
}

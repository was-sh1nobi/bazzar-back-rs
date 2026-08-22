use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use crate::entities::users;

pub async fn register_if_absent(
    db: &DatabaseConnection,
    username: &str,
    player_uuid: &str,
) -> Result<(), sea_orm::DbErr> {
    let existing = users::Entity::find()
        .filter(users::Column::Playeruuid.eq(player_uuid))
        .one(db)
        .await?;
    if existing.is_some() {
        // upsert username
        let am = users::ActiveModel {
            playeruuid: Set(player_uuid.to_owned()),
            username: Set(username.to_owned()),
            ..Default::default()
        };
        let _ = users::Entity::update(am).exec(db).await;
        return Ok(());
    }
    // insert with balance 0
    let am = users::ActiveModel {
        playeruuid: Set(player_uuid.to_owned()),
        username: Set(username.to_owned()),
        balance: Set(0.0),
        ..Default::default()
    };
    let _ = users::Entity::insert(am).exec(db).await;
    Ok(())
}

use crate::entities::users;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};

/// Upsert a user with balance 0 (guide §9.1 `registerIfAbsentByUuid`).
/// When the user already exists and a `username` is provided, the stored
/// name is updated. Errors are propagated — callers decide how to handle them.
pub async fn register_if_absent(
    db: &DatabaseConnection,
    player_uuid: &str,
    username: Option<&str>,
) -> Result<(), sea_orm::DbErr> {
    match users::Entity::find()
        .filter(users::Column::Playeruuid.eq(player_uuid))
        .one(db)
        .await?
    {
        Some(existing) => {
            if let Some(name) = username {
                let mut am: users::ActiveModel = existing.into();
                am.username = Set(name.to_owned());
                users::Entity::update(am).exec(db).await?;
            }
            Ok(())
        }
        None => {
            let am = users::ActiveModel {
                playeruuid: Set(player_uuid.to_owned()),
                username: Set(username
                    .map(str::to_owned)
                    .unwrap_or_else(|| player_uuid.to_owned())),
                balance: Set(0.0),
                ..Default::default()
            };
            users::Entity::insert(am).exec(db).await?;
            Ok(())
        }
    }
}

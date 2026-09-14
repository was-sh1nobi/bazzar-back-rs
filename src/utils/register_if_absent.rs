use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};

/// Upsert a user with balance 0 (guide §9.1 `registerIfAbsentByUuid`).
/// When the user already exists and a `username` is provided, the stored
/// name is updated. Errors are propagated — callers decide how to handle them.
pub async fn register_if_absent(
    db: &DatabaseConnection,
    player_uuid: &str,
    username: Option<&str>,
) -> Result<(), sea_orm::DbErr> {
    let stmt = Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"INSERT INTO users (playeruuid, username, balance)
           VALUES ($1, $2, 0.0)
           ON CONFLICT (playeruuid) DO UPDATE
           SET username = COALESCE($3, users.username)"#,
        [
            player_uuid.into(),
            username.unwrap_or(player_uuid).into(),
            username
                .map(Into::into)
                .unwrap_or(sea_orm::Value::from(Option::<String>::None)),
        ],
    );
    db.execute(stmt).await?;
    Ok(())
}

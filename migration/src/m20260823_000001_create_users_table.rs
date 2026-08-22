use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260823_000001_create_users_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Users::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Users::PlayerUuid).text().not_null().primary_key())
                    .col(ColumnDef::new(Users::Username).text().not_null())
                    .col(
                        ColumnDef::new(Users::Balance)
                            .double()
                            .not_null()
                            .default(0.0),
                    )
                    .col(ColumnDef::new(Users::PasswordHash).text().null())
                    .col(
                        ColumnDef::new(Users::CreatedAt)
                            .big_integer()
                            .null()
                            .default(Expr::cust("extract(epoch from now())::bigint")),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_users_username")
                    .table(Users::Table)
                    .col(Users::Username)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Users::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Users {
    Table,
    #[sea_orm(iden = "playeruuid")]
    PlayerUuid,
    #[sea_orm(iden = "username")]
    Username,
    #[sea_orm(iden = "balance")]
    Balance,
    #[sea_orm(iden = "password_hash")]
    PasswordHash,
    #[sea_orm(iden = "created_at")]
    CreatedAt,
}

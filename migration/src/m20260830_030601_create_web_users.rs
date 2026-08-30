use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260830_030601_create_web_users"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(WebUsers::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(WebUsers::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(WebUsers::Username).text().not_null().unique_key())
                    .col(ColumnDef::new(WebUsers::PasswordHash).text().not_null())
                    .col(
                        ColumnDef::new(WebUsers::Role)
                            .text()
                            .not_null()
                            .default("member"),
                    )
                    .col(
                        ColumnDef::new(WebUsers::CreatedAt)
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
                    .name("idx_web_users_username")
                    .table(WebUsers::Table)
                    .col(WebUsers::Username)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(WebUsers::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum WebUsers {
    Table,
    Id,
    Username,
    PasswordHash,
    Role,
    CreatedAt,
}

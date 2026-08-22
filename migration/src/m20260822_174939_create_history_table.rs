use sea_orm_migration::{prelude::*, schema::*};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260822_174939_create_history_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
              .create_table(
                  Table::create()
                      .table(History::Table)
                      .if_not_exists()
                      .col(pk_auto(History::Id))
                      .col(text_null(History::Seller))
                      .col(text_null(History::Buyer))
                      .col(text_null(History::Material))
                      .col(text_null(History::Price))
                      .col(integer_null(History::Amount))
                      .col(text_null(History::Source))
                      .col(text_null(History::Origin))
                  .to_owned(),
              ).await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_history_buyer")
                    .table(History::Table)
                    .col(History::Buyer)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(History::Table).to_owned()).await
    }
}


#[derive(DeriveIden)]
enum History{
    Table,
    Id,
    Seller,
    Buyer,
    Material,
    Price,
    Amount,
    Source,
    Origin,
}
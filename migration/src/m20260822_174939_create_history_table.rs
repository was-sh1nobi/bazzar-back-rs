use sea_orm_migration::{prelude::*, schema::*};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260822_174939_create_history_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
         _manager
             .create_table(
                 Table::create()
                     .table(History::Table)
                     .if_not_exists()
                     .col(pk_auto(History::Id))
                     .col(text(History::Seller).not_null())
                     .col(text(History::Buyer).not_null())
                     .col(text(History::Material).not_null())
                     .col(text(History::Price).not_null())
                     .col(integer(History::Amount).not_null())
                     .col(text(History::Source).not_null())
                     .col(text(History::Origin).not_null())
                 .to_owned(),

             ).await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        _manager
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
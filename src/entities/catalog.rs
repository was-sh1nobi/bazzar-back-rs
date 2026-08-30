use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "catalog_status")]
pub enum CatalogStatus {
    #[sea_orm(string_value = "AVAILABLE")]
    Available,
    #[sea_orm(string_value = "UNAVAILABLE")]
    Unavailable,
    #[sea_orm(string_value = "REMOVED")]
    Removed,
    #[sea_orm(string_value = "COMING SOON")]
    ComingSoon,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "catalog")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub material: String,
    pub category: Option<String>,
    pub buy_price: f64,
    pub sell_price: f64,
    pub status: CatalogStatus,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

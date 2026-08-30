use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "item_status")]
pub enum ItemStatus {
    #[sea_orm(string_value = "AVAILABLE")]
    Available,
    #[sea_orm(string_value = "PENDING")]
    Pending,
    #[sea_orm(string_value = "SOLD")]
    Sold,
    #[sea_orm(string_value = "REMOVED")]
    Removed,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "items")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub seller: String,
    /// Direct UUID of the seller for fast queries (added 2026-09-01, backfilled from users).
    /// Use this for all new queries instead of the legacy `seller` name join.
    pub seller_playeruuid: Option<String>,
    pub material: Option<String>,
    pub amount: Option<i32>,
    pub price: Option<String>,
    pub source: Option<String>,
    pub origin: Option<String>,
    pub status: ItemStatus,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::Seller",
        to = "super::users::Column::Username",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Users,
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::SellerPlayeruuid",
        to = "super::users::Column::Playeruuid",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    SellerByUuid,
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Users.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

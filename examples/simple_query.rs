//! Simple SeaORM query example — copy this pattern anywhere
//! Run: cargo run --example simple_query

use bazzar_back::entities::{items, users};
use sea_orm::{ColumnTrait, Database, EntityTrait, QueryFilter, Set};

#[tokio::main]
async fn main() -> Result<(), sea_orm::DbErr> {
    // 1. connect (uses DATABASE_URL from .env, same as src/database/mod.rs:4)
    let db = Database::connect("postgres://amir:pass@localhost:5432/bazzardb").await?;

    // ── SELECT: find one user by playeruuid ──────────────────────────
    let user = users::Entity::find()
        .filter(users::Column::Playeruuid.eq("550e8400-e29b-41d4-a716-446655440000"))
        .one(&db)
        .await?;

    println!("found user: {:?}", user);

    // ── SELECT: find all AVAILABLE items by seller ───────────────────
    let steve_items = items::Entity::find()
        .filter(items::Column::Seller.eq("Steve"))
        .filter(items::Column::Status.eq(items::ItemStatus::Available))
        .all(&db)
        .await?;

    println!("Steve has {} available listings", steve_items.len());
    for it in &steve_items {
        println!(
            "  #{} {} x{} @ {}",
            it.id,
            it.material.as_deref().unwrap_or("?"),
            it.amount.unwrap_or(0),
            it.price.as_deref().unwrap_or("0")
        );
    }

    // ── INSERT: create a new item ────────────────────────────────────
    let new_item = items::ActiveModel {
        seller: Set("Steve".to_owned()),
        material: Set(Some("DIAMOND".to_owned())),
        amount: Set(Some(3)),
        price: Set(Some("150.0".to_owned())), // spec stores price as TEXT:5.1
        source: Set(Some("player".to_owned())),
        origin: Set(Some("survival".to_owned())),
        status: Set(items::ItemStatus::Available),
        ..Default::default() // id auto-generates
    };
    let res = items::Entity::insert(new_item).exec(&db).await?;
    println!("inserted item id = {}", res.last_insert_id);

    Ok(())
}

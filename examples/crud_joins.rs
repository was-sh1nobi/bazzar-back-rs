//! SeaORM Cookbook: UPDATE / DELETE / JOINs
//! Run: cargo run --example crud_joins
//!
//! Uses tables from BACKEND_MIGRATION_GUIDE.md §5.1:
//!   items(id, seller, material, amount, price, status, ...)
//!   history(id, seller, buyer, ...)
//!   users(playeruuid PK, username, balance, ...)

use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Database, EntityTrait, JoinType, QueryFilter,
    QuerySelect, RelationTrait,
};
use bazzar_back::entities::{history, items, users};

#[tokio::main]
async fn main() -> Result<(), sea_orm::DbErr> {
    let db = Database::connect("postgres://amir:pass@localhost:5432/bazzardb").await?;

    // ─────────────────────────────────────────────────────────────
    // UPDATE — single row (read -> modify -> save)
    // ─────────────────────────────────────────────────────────────
    // e.g. change price of item #1
    if let Some(model) = items::Entity::find_by_id(1).one(&db).await? {
        let mut am: items::ActiveModel = model.into();
        am.price = Set(Some("999.5".to_owned())); // TEXT per spec §5.1
        let updated: items::Model = am.update(&db).await?;
        println!("UPDATE single: id={} new_price={:?}", updated.id, updated.price);
    }

    // UPDATE — bulk via update_many (no SELECT, single SQL)
    // e.g. bump price of all DIAMOND listings (TEXT column, no enum cast needed)
    let res = items::Entity::update_many()
        .col_expr(items::Column::Price, sea_orm::sea_query::Expr::value("200.0"))
        .filter(items::Column::Material.eq("DIAMOND"))
        .filter(items::Column::Status.eq(items::ItemStatus::Available))
        .exec(&db)
        .await?;
    println!("UPDATE many: rows_affected={}", res.rows_affected);
    // For enum status, use cast: Expr::cust("'PENDING'::item_status")

    // UPDATE — users balance (economy §9 atomic pattern hint)
    // creditIfRegistered: find uuid by username then changeBalance
    let username = "Steve";
    if let Some(seller) = users::Entity::find()
        .filter(users::Column::Username.eq(username))
        .one(&db)
        .await?
    {
        let mut am: users::ActiveModel = seller.into();
        // naive read-modify-write; for production use:
        //   UPDATE users SET balance = balance + 100 WHERE playeruuid=$1 AND balance+100>=0
        // via raw SQL or col_expr with balance+value
        // Here demo sets absolute value:
        am.balance = Set(1500.0);
        let u = am.update(&db).await?;
        println!("UPDATE user balance: {} -> {}", username, u.balance);
    }

    // ─────────────────────────────────────────────────────────────
    // DELETE
    // ─────────────────────────────────────────────────────────────
    // single by PK
    let del = items::Entity::delete_by_id(999_999).exec(&db).await?;
    println!("DELETE by_id rows_affected={}", del.rows_affected);

    // bulk: delete all REMOVED listings (soft-delete cleanup)
    let del_many = items::Entity::delete_many()
        .filter(items::Column::Status.eq(items::ItemStatus::Removed))
        .exec(&db)
        .await?;
    println!("DELETE many (REMOVED) rows_affected={}", del_many.rows_affected);

    // history delete example
    let _ = history::Entity::delete_many()
        .filter(history::Column::Material.eq("COBBLESTONE"))
        .exec(&db)
        .await?;

    // ─────────────────────────────────────────────────────────────
    // JOINs — items.seller (name) -> users.username
    // Relation defined in src/entities/items.rs:30
    // ─────────────────────────────────────────────────────────────

    // 1) INNER JOIN — only items whose seller exists in users
    //    SQL: SELECT items.* FROM items INNER JOIN users ON items.seller = users.username
    let inner: Vec<items::Model> = items::Entity::find()
        .inner_join(users::Entity) // uses Relation::Users, keeps base entity
        .all(&db)
        .await?;
    // Tuple form with user data uses find_also_related:
    let inner2: Vec<(items::Model, Option<users::Model>)> = items::Entity::find()
        .find_also_related(users::Entity)
        .filter(users::Column::Username.is_not_null())
        .all(&db)
        .await?;
    println!("INNER JOIN rows={} (also_related rows={})", inner.len(), inner2.len());

    // Manual INNER JOIN with explicit JoinType (same as above)
    let manual_inner = items::Entity::find()
        .join(JoinType::InnerJoin, items::Relation::Users.def())
        .filter(users::Column::Balance.gt(0.0))
        .all(&db)
        .await?;
    println!("manual INNER JOIN wealthy sellers: {}", manual_inner.len());

    // 2) LEFT JOIN — all items, with optional user (keeps items even if seller not registered)
    let left: Vec<(items::Model, Option<users::Model>)> = items::Entity::find()
        .find_also_related(users::Entity)
        .all(&db)
        .await?;
    println!("LEFT JOIN (find_also_related) rows={}", left.len());
    for (it, usr) in left.iter().take(3) {
        println!("  item #{} seller={} -> user={:?}", it.id, it.seller, usr.as_ref().map(|u| &u.username));
    }

    // Explicit LEFT JOIN via QuerySelect
    let left2 = items::Entity::find()
        .join(JoinType::LeftJoin, items::Relation::Users.def())
        .all(&db)
        .await?;
    println!("LEFT JOIN explicit rows={}", left2.len());

    // 3) LEFT JOIN users -> items (reverse: all users with their listings)
    let users_left_items = users::Entity::find()
        .join(JoinType::LeftJoin, users::Relation::Items.def())
        .all(&db)
        .await?;
    println!("users LEFT JOIN items rows={}", users_left_items.len());

    // 4) RIGHT JOIN — not common, demo syntax (Postgres supports it)
    let right = items::Entity::find()
        .join(JoinType::RightJoin, items::Relation::Users.def())
        .all(&db)
        .await?;
    println!("RIGHT JOIN rows={}", right.len());

    // 5) JOIN on history -> users (buyer uuid -> playeruuid)
    // history has two relations to users, so use explicit join:
    let history_inner = history::Entity::find()
        .join(JoinType::InnerJoin, history::Relation::BuyerUser.def())
        .all(&db)
        .await?;
    println!("history INNER JOIN buyer rows={}", history_inner.len());

    // 6) CROSS JOIN — cartesian product (rare, no ON clause)
    // SeaORM's CrossJoin with Relation generates ON, so use raw SQL:
    // SELECT items.* FROM items CROSS JOIN users -> every item x every user
    // Note: raw enum needs cast: SELECT id, seller, material, amount, price::text, source, origin, status::text FROM items CROSS JOIN users
    println!("CROSS JOIN = SELECT * FROM items CROSS JOIN users (cartesian, use LIMIT!)");

    Ok(())
}

mod common;

use actix_web::{App, http::StatusCode, test};
use bazzar_back::auth::create_jwt;

const TEST_UUID: &str = "123e4567-e89b-12d3-a456-426614174000";
const OTHER_UUID: &str = "223e4567-e89b-12d3-a456-426614174000";

fn bearer_header(uuid: &str) -> (&'static str, String) {
    let token = create_jwt(uuid, "Steve").expect("create test jwt");
    ("Authorization", format!("Bearer {token}"))
}

#[actix_web::test]
async fn root_welcome_route() {
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(&app, test::TestRequest::get().uri("/").to_request()).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[actix_web::test]
async fn health_route() {
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response =
        test::call_service(&app, test::TestRequest::get().uri("/health").to_request()).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[actix_web::test]
async fn auth_register_route_requires_valid_json() {
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/register")
            .set_payload("invalid")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn auth_login_route_requires_valid_json() {
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/login")
            .set_payload("invalid")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn auth_status_route_requires_player() {
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::get().uri("/auth/status").to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn auth_change_route_requires_authorization() {
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::post().uri("/auth/change").to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn auth_change_route_with_auth_requires_valid_json() {
    let (k, v) = bearer_header(TEST_UUID);
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/change")
            .insert_header((k, v))
            .set_payload("invalid")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn balance_route_requires_authorization() {
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response =
        test::call_service(&app, test::TestRequest::get().uri("/balance").to_request()).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn balance_route_requires_player() {
    let (k, v) = bearer_header(TEST_UUID);
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/balance")
            .insert_header((k, v))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn balance_route_rejects_mismatched_player() {
    let (k, v) = bearer_header(TEST_UUID);
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/balance?player={OTHER_UUID}"))
            .insert_header((k, v))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn balance_route_matching_player_reaches_database_boundary() {
    let (k, v) = bearer_header(TEST_UUID);
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/balance?player={TEST_UUID}"))
            .insert_header((k, v))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[actix_web::test]
async fn balance_set_route_is_gone() {
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::post().uri("/balance/set").to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::GONE);
}

#[actix_web::test]
async fn balance_change_route_is_gone() {
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/balance/change")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::GONE);
}

#[actix_web::test]
async fn stats_route_requires_authorization() {
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response =
        test::call_service(&app, test::TestRequest::get().uri("/stats").to_request()).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn stats_route_requires_player() {
    let (k, v) = bearer_header(TEST_UUID);
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/stats")
            .insert_header((k, v))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn stats_route_rejects_mismatched_player() {
    let (k, v) = bearer_header(TEST_UUID);
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/stats?player={OTHER_UUID}"))
            .insert_header((k, v))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn stats_route_matching_player_reaches_database_boundary() {
    let (k, v) = bearer_header(TEST_UUID);
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&format!("/stats?player={TEST_UUID}"))
            .insert_header((k, v))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[actix_web::test]
async fn items_route_reaches_database_boundary() {
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response =
        test::call_service(&app, test::TestRequest::get().uri("/items").to_request()).await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[actix_web::test]
async fn items_poll_route_reaches_database_boundary() {
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/items/poll?timeout=1")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[actix_web::test]
async fn items_active_route_requires_authorization() {
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::get().uri("/items/active").to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn items_active_route_reaches_database_boundary() {
    let (k, v) = bearer_header(TEST_UUID);
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/items/active")
            .insert_header((k, v))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[actix_web::test]
async fn items_bought_route_requires_authorization() {
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::get().uri("/items/bought").to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn items_bought_route_reaches_database_boundary() {
    let (k, v) = bearer_header(TEST_UUID);
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/items/bought")
            .insert_header((k, v))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[actix_web::test]
async fn items_history_route_requires_authorization() {
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::get().uri("/items/history").to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn items_history_route_reaches_database_boundary() {
    let (k, v) = bearer_header(TEST_UUID);
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/items/history")
            .insert_header((k, v))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[actix_web::test]
async fn items_add_route_requires_authorization() {
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::post().uri("/items/add").to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn items_add_route_with_auth_requires_valid_json() {
    let (k, v) = bearer_header(TEST_UUID);
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/items/add")
            .insert_header((k, v))
            .set_payload("invalid")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn items_buy_route_requires_authorization() {
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::post().uri("/items/buy").to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn items_buy_route_with_auth_requires_valid_json() {
    let (k, v) = bearer_header(TEST_UUID);
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/items/buy")
            .insert_header((k, v))
            .set_payload("invalid")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn items_retrieve_route_requires_authorization() {
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/items/retrieve")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn items_retrieve_route_with_auth_requires_valid_json() {
    let (k, v) = bearer_header(TEST_UUID);
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/items/retrieve")
            .insert_header((k, v))
            .set_payload("invalid")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn list_route_requires_authorization() {
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response =
        test::call_service(&app, test::TestRequest::post().uri("/list").to_request()).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn list_route_with_auth_requires_valid_json() {
    let (k, v) = bearer_header(TEST_UUID);
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/list")
            .insert_header((k, v))
            .set_payload("invalid")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn legacy_list_route_requires_authorization() {
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(&app, test::TestRequest::post().uri("/").to_request()).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn legacy_list_route_with_auth_requires_valid_json() {
    let (k, v) = bearer_header(TEST_UUID);
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/")
            .insert_header((k, v))
            .set_payload("invalid")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn shop_catalog_route_returns_catalog() {
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::get().uri("/shop/catalog").to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[actix_web::test]
async fn shop_catalog_reload_route_requires_authorization() {
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/shop/catalog/reload")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn shop_buy_route_requires_authorization() {
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::post().uri("/shop/buy").to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn shop_buy_route_with_auth_requires_valid_json() {
    let (k, v) = bearer_header(TEST_UUID);
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/shop/buy")
            .insert_header((k, v))
            .set_payload("invalid")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn shop_sell_route_requires_authorization() {
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::post().uri("/shop/sell").to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn shop_sell_route_with_auth_requires_valid_json() {
    let (k, v) = bearer_header(TEST_UUID);
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/shop/sell")
            .insert_header((k, v))
            .set_payload("invalid")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

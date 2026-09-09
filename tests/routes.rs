mod common;

use actix_web::{App, http::StatusCode, test};

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
async fn balance_route_requires_player() {
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response =
        test::call_service(&app, test::TestRequest::get().uri("/balance").to_request()).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn stats_route_requires_authorization() {
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response =
        test::call_service(&app, test::TestRequest::get().uri("/stats").to_request()).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
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
        test::TestRequest::get().uri("/items/poll").to_request(),
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
async fn legacy_list_route_requires_authorization() {
    let app = test::init_service(App::new().configure(common::configure)).await;
    let response = test::call_service(&app, test::TestRequest::post().uri("/").to_request()).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

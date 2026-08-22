use actix_web::{web, HttpResponse, Responder, Scope};

pub fn auth_scope() -> Scope {
    web::scope("/auth")
        .route("/login" , web::post().to(login))
}

async fn login() -> impl Responder {
    HttpResponse::Ok().finish()
}


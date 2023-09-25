use axum::Json;
use serde_json::{json, Value};

pub async fn health() -> Json<Value> {
    Json(json!( {
        "message": "API up!",
    }))
}

pub mod auth;
pub mod event;
pub mod user;

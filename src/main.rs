use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Serialize, Clone)]
struct User {
    id: u32,
    name: String,
}

#[derive(Deserialize)]
struct CreateUserRequest {
    name: String,
}

type AppState = Arc<Mutex<HashMap<u32, User>>>;

async fn root() -> &'static str {
    "hello from axum\n"
}

async fn health() -> &'static str {
    "ok\n"
}

async fn list_users(State(state): State<AppState>) -> Json<Vec<User>> {
    let users = state.lock().await;
    let list: Vec<User> = users.values().cloned().collect();
    Json(list)
}

async fn get_user(
    State(state): State<AppState>,
    Path(id): Path<u32>,
) -> Result<Json<User>, axum::http::StatusCode> {
    let users = state.lock().await;
    match users.get(&id) {
        Some(user) => Ok(Json(user.clone())),
        None => Err(axum::http::StatusCode::NOT_FOUND),
    }
}

async fn create_user(
    State(state): State<AppState>,
    Json(payload): Json<CreateUserRequest>,
) -> (axum::http::StatusCode, Json<User>) {
    let mut users = state.lock().await;
    let next_id = (users.len() as u32) + 1;
    let user = User {
        id: next_id,
        name: payload.name,
    };
    users.insert(next_id, user.clone());
    (axum::http::StatusCode::CREATED, Json(user))
}

#[tokio::main]
async fn main() {
    let state: AppState = Arc::new(Mutex::new(HashMap::new()));

    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/users", get(list_users).post(create_user))
        .route("/users/{id}", get(get_user))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .unwrap();
    println!("listening on http://127.0.0.1:8080");
    axum::serve(listener, app).await.unwrap();
}
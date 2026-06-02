use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Serialize)]
struct User {
    id: i32,
    name: String,
    email: String,
    created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
struct CreateUserRequest {
    name: String,
    email: String,
}

async fn root() -> &'static str {
    "hello from axum + postgres\n"
}

async fn health() -> &'static str {
    "ok\n"
}

async fn list_users(State(pool): State<PgPool>) -> Result<Json<Vec<User>>, axum::http::StatusCode> {
    let users = sqlx::query_as!(
        User,
        "SELECT id, name, email, created_at FROM users ORDER BY id"
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        eprintln!("db error: {}", e);
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(users))
}

async fn get_user(
    State(pool): State<PgPool>,
    Path(id): Path<i32>,
) -> Result<Json<User>, axum::http::StatusCode> {
    let user = sqlx::query_as!(
        User,
        "SELECT id, name, email, created_at FROM users WHERE id = $1",
        id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        eprintln!("db error: {}", e);
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match user {
        Some(u) => Ok(Json(u)),
        None => Err(axum::http::StatusCode::NOT_FOUND),
    }
}

async fn create_user(
    State(pool): State<PgPool>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<(axum::http::StatusCode, Json<User>), axum::http::StatusCode> {
    let user = sqlx::query_as!(
        User,
        "INSERT INTO users (name, email) VALUES ($1, $2) RETURNING id, name, email, created_at",
        payload.name,
        payload.email
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(db_err) = &e {
            if db_err.is_unique_violation() {
                return axum::http::StatusCode::CONFLICT;
            }
        }
        eprintln!("db error: {}", e);
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok((axum::http::StatusCode::CREATED, Json(user)))
}

async fn delete_user(
    State(pool): State<PgPool>,
    Path(id): Path<i32>,
) -> Result<axum::http::StatusCode, axum::http::StatusCode> {
    let result = sqlx::query!("DELETE FROM users WHERE id = $1", id)
        .execute(&pool)
        .await
        .map_err(|e| {
            eprintln!("db error: {}", e);
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if result.rows_affected() == 0 {
        Err(axum::http::StatusCode::NOT_FOUND)
    } else {
        Ok(axum::http::StatusCode::NO_CONTENT)
    }
}


#[derive(Deserialize)]
struct UpdateUserRequest {
    name: String,
    email: String,
}

async fn update_user(
    State(pool): State<PgPool>,
    Path(id): Path<i32>,
    Json(payload): Json<UpdateUserRequest>,
) -> Result<Json<User>, axum::http::StatusCode> {
    let user = sqlx::query_as!(
        User,
        "UPDATE users SET name = $1, email = $2 WHERE id = $3 RETURNING id, name, email, created_at",
        payload.name,
        payload.email,
        id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(db_err) = &e {
            if db_err.is_unique_violation() {
                return axum::http::StatusCode::CONFLICT;
            }
        }
        eprintln!("db error: {}", e);
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match user {
        Some(u) => Ok(Json(u)),
        None => Err(axum::http::StatusCode::NOT_FOUND),
    }
}

#[tokio::main]
async fn main() {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = PgPool::connect(&db_url)
        .await
        .expect("failed to connect to database");

    println!("connected to database");
    let app = Router::new()
    .route("/", get(root))
    .route("/health", get(health))
    .route("/users", get(list_users).post(create_user))
    .route(
        "/users/{id}",
        get(get_user).put(update_user).delete(delete_user),
    )
    .with_state(pool);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .unwrap();
    println!("listening on http://127.0.0.1:8080");
    axum::serve(listener, app).await.unwrap();
}
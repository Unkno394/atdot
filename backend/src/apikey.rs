use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::{error::AppError, state::AppState};
use super::AuthUser;

#[derive(Deserialize)]
pub struct CreateKeyRequest {
    pub name: String,
}

#[derive(Serialize)]
pub struct ApiKeyResponse {
    pub id:         Uuid,
    pub key:        String,
    pub name:       String,
    pub created_at: String,
}

fn generate_key() -> String {
    format!("mdr_{}", Uuid::new_v4().to_string().replace('-', ""))
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> Result<Json<Vec<ApiKeyResponse>>, AppError> {
    let uid = Uuid::parse_str(&auth.user_id).unwrap();

    let rows = sqlx::query!(
        "SELECT id, key, name, created_at FROM api_keys WHERE user_id = $1 ORDER BY created_at DESC",
        uid
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows.iter().map(|r| ApiKeyResponse {
        id:         r.id,
        key:        r.key.clone(),
        name:       r.name.clone(),
        created_at: r.created_at.to_rfc3339(),
    }).collect()))
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(req): Json<CreateKeyRequest>,
) -> Result<Json<ApiKeyResponse>, AppError> {
    if req.name.trim().is_empty() {
        return Err(AppError::BadRequest("name is required".into()));
    }

    let uid = Uuid::parse_str(&auth.user_id).unwrap();
    let key = generate_key();

    let row = sqlx::query!(
        "INSERT INTO api_keys (user_id, key, name) VALUES ($1, $2, $3) RETURNING id, key, name, created_at",
        uid, key, req.name.trim()
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(ApiKeyResponse {
        id:         row.id,
        key:        row.key,
        name:       row.name,
        created_at: row.created_at.to_rfc3339(),
    }))
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let uid = Uuid::parse_str(&auth.user_id).unwrap();
    sqlx::query!(
        "DELETE FROM api_keys WHERE id = $1 AND user_id = $2",
        id, uid
    )
    .execute(&state.db)
    .await?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

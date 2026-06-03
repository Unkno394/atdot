use axum::{extract::State, http::HeaderMap, Json};
use chrono::Utc;
use serde::Deserialize;
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

use crate::{error::AppError, state::AppState};

#[derive(Deserialize)]
pub struct VerifyRequest {
    pub challenge_id: Uuid,
    pub session_id:   String,
}

/// POST /api/challenge/verify
///
/// SDK calls this after the user completes the challenge overlay.
/// Returns {"ok": true, "action": "allow"} on success.
pub async fn verify(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<VerifyRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = headers
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Auth("missing X-API-Key header".into()))?;

    sqlx::query!("SELECT id FROM api_keys WHERE key = $1", api_key)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::Auth("invalid API key".into()))?;

    let row = sqlx::query(
        "SELECT id, expires_at, solved_at
         FROM challenges
         WHERE id = $1 AND session_id = $2",
    )
    .bind(req.challenge_id)
    .bind(&req.session_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("challenge not found".into()))?;

    let expires_at: chrono::DateTime<Utc> = row.try_get("expires_at")
        .map_err(|_| AppError::Internal("expires_at missing".into()))?;
    let solved_at: Option<chrono::DateTime<Utc>> = row.try_get("solved_at").ok().flatten();

    if Utc::now() > expires_at {
        return Ok(Json(serde_json::json!({"ok": false, "reason": "expired"})));
    }
    if solved_at.is_some() {
        return Ok(Json(serde_json::json!({"ok": false, "reason": "already_solved"})));
    }

    sqlx::query("UPDATE challenges SET solved_at = NOW() WHERE id = $1")
        .bind(req.challenge_id)
        .execute(&state.db)
        .await?;

    tracing::info!(
        challenge_id = %req.challenge_id,
        session_id   = %req.session_id,
        "challenge solved"
    );

    Ok(Json(serde_json::json!({"ok": true, "action": "allow"})))
}

use axum::{extract::State, Json};
use serde::Deserialize;
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

use crate::{error::AppError, state::AppState};
use super::AuthUser;

#[derive(Deserialize)]
pub struct FeedbackRequest {
    pub session_id:      String,
    pub confirmed_fraud: bool,
}

/// POST /api/feedback — mark a session as confirmed fraud or legitimate.
/// Also applies a fraud strike to the user's embedding so the adaptive
/// threshold tightens for future events from the same principal.
pub async fn submit(
    State(state): State<Arc<AppState>>,
    _auth: AuthUser,
    Json(req): Json<FeedbackRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let updated = sqlx::query(
        "UPDATE session_scores
         SET confirmed_fraud = $1
         WHERE session_id = $2",
    )
    .bind(req.confirmed_fraud)
    .bind(&req.session_id)
    .execute(&state.db)
    .await?;

    // Look up the user_id (merchant principal) tied to this session
    // so we can apply the fraud signal to their embedding.
    let row = sqlx::query(
        r#"SELECT k.user_id
           FROM session_scores ss
           JOIN api_keys k ON ss.api_key_id = k.id
           WHERE ss.session_id = $1
           LIMIT 1"#,
    )
    .bind(&req.session_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if let Some(row) = row {
        if let Ok(user_id) = row.try_get::<Uuid, _>("user_id") {
            state.fraud.apply_fraud_signal(user_id, req.confirmed_fraud);
        }
    }

    Ok(Json(serde_json::json!({
        "ok":      true,
        "updated": updated.rows_affected(),
    })))
}

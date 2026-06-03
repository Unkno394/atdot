use axum::{extract::State, http::HeaderMap, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::{error::AppError, state::AppState};
use super::AuthUser;

#[derive(Deserialize)]
pub struct IngestRequest {
    pub session_id:  String,
    pub visitor_id:  Option<String>,
    pub event_type:  String,
    pub payload:     Option<serde_json::Value>,
}

#[derive(Serialize)]
pub struct StatsResponse {
    pub dau:            i64,
    pub events_today:   i64,
    pub total_sessions: i64,
    pub fraud_alerts:   i64,
}

#[derive(Serialize)]
pub struct RecentEvent {
    pub id:         Uuid,
    pub session_id: String,
    pub event_type: String,
    pub timestamp:  String,
    pub ip:         Option<String>,
}

pub async fn ingest(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<IngestRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // validate API key
    let api_key = headers
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Auth("missing X-API-Key header".into()))?;

    let key_row = sqlx::query!("SELECT id FROM api_keys WHERE key = $1", api_key)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::Auth("invalid API key".into()))?;

    // extract client info
    let ip = headers
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("").trim().to_string());

    let ua = headers
        .get("User-Agent")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    sqlx::query!(
        r#"INSERT INTO events (api_key_id, session_id, visitor_id, event_type, payload, ip, user_agent)
           VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        key_row.id,
        req.session_id,
        req.visitor_id,
        req.event_type,
        req.payload.unwrap_or(serde_json::json!({})),
        ip,
        ua
    )
    .execute(&state.db)
    .await?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn stats(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> Result<Json<StatsResponse>, AppError> {
    let uid = Uuid::parse_str(&auth.user_id).unwrap();

    let dau = sqlx::query_scalar!(
        r#"SELECT COUNT(DISTINCT e.visitor_id) as "count!"
           FROM events e JOIN api_keys k ON e.api_key_id = k.id
           WHERE k.user_id = $1 AND e.timestamp > NOW() - INTERVAL '24 hours'"#,
        uid
    )
    .fetch_one(&state.db)
    .await?;

    let events_today = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "count!"
           FROM events e JOIN api_keys k ON e.api_key_id = k.id
           WHERE k.user_id = $1 AND e.timestamp > NOW() - INTERVAL '24 hours'"#,
        uid
    )
    .fetch_one(&state.db)
    .await?;

    let total_sessions = sqlx::query_scalar!(
        r#"SELECT COUNT(DISTINCT e.session_id) as "count!"
           FROM events e JOIN api_keys k ON e.api_key_id = k.id
           WHERE k.user_id = $1"#,
        uid
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(StatsResponse {
        dau,
        events_today,
        total_sessions,
        fraud_alerts: 0, // подключается после fraud detection слоя
    }))
}

pub async fn recent(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> Result<Json<Vec<RecentEvent>>, AppError> {
    let uid = Uuid::parse_str(&auth.user_id).unwrap();

    let rows = sqlx::query!(
        r#"SELECT e.id, e.session_id, e.event_type, e.timestamp, e.ip
           FROM events e JOIN api_keys k ON e.api_key_id = k.id
           WHERE k.user_id = $1
           ORDER BY e.timestamp DESC LIMIT 50"#,
        uid
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows.iter().map(|r| RecentEvent {
        id:         r.id,
        session_id: r.session_id.clone(),
        event_type: r.event_type.clone(),
        timestamp:  r.timestamp.to_rfc3339(),
        ip:         r.ip.clone(),
    }).collect()))
}

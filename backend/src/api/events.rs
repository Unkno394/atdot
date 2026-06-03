use axum::{extract::State, http::HeaderMap, Json};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

use crate::fraud::scoring::ScoringAction;
use crate::{error::AppError, state::AppState};
use super::AuthUser;

#[derive(Deserialize)]
pub struct IngestRequest {
    pub session_id:  String,
    pub visitor_id:  Option<String>,
    pub event_type:  String,
    pub payload:     Option<serde_json::Value>,
    /// End-user identity set via atdot.identify(userId).
    /// Can be any string; non-UUID values are hashed to a deterministic UUID
    /// so L1 can build a per-user behavioural graph.
    pub user_id:     Option<String>,
    pub webrtc_ip:   Option<String>,
    pub ipv6:        Option<String>,
    pub timezone:    Option<String>,
    pub fingerprint: Option<String>,
}

#[derive(Deserialize)]
pub struct BatchRequest {
    pub events: Vec<IngestRequest>,
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

/// Parse the SDK's user_id string into a UUID for L1 graph keying.
/// Valid UUID strings are used as-is; anything else is hashed via UUID v5
/// (deterministic — same input always produces the same UUID).
fn resolve_user_id(s: &str) -> Uuid {
    Uuid::parse_str(s)
        .unwrap_or_else(|_| Uuid::new_v5(&Uuid::NAMESPACE_DNS, s.as_bytes()))
}

/// Core per-event processing: score, persist, optionally create challenge.
/// Returns the JSON body to send back for this one event.
async fn process_event(
    state:      &AppState,
    req:        &IngestRequest,
    key_id:     Uuid,
    ip:         Option<&str>,
    ua:         Option<&str>,
    owner_uid:  Option<Uuid>,
    prev_event: Option<&str>,
) -> serde_json::Value {
    // Prefer the SDK-supplied end-user ID; fall back to the merchant's owner UUID
    let effective_uid = req.user_id.as_deref()
        .map(resolve_user_id)
        .or(owner_uid);

    let fraud_score = state.fraud.score_event(
        &req.session_id,
        effective_uid,
        &req.event_type,
        &req.payload.clone().unwrap_or_default(),
        ip,
        req.visitor_id.as_deref(),
        ua,
        req.webrtc_ip.as_deref(),
        req.ipv6.as_deref(),
        req.timezone.as_deref(),
        req.fingerprint.as_deref(),
        prev_event,
    ).await;

    // Persist score
    let reasons_json = serde_json::to_value(&fraud_score.reasons).unwrap_or_default();
    let _ = sqlx::query(
        r#"INSERT INTO session_scores
           (session_id, api_key_id, score, l1_score, l2_score, l3_score, action,
            reasons, embedding_score, event_type)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#,
    )
    .bind(&req.session_id)
    .bind(key_id)
    .bind(fraud_score.score as f64)
    .bind(fraud_score.l1    as f64)
    .bind(fraud_score.l2    as f64)
    .bind(fraud_score.l3    as f64)
    .bind(fraud_score.action.to_string())
    .bind(&reasons_json)
    .bind(fraud_score.embedding_score as f64)
    .bind(&req.event_type)
    .execute(&state.db)
    .await;

    // Log honeypot / decoy triggers to dedicated table
    if req.event_type == "honeypot_trigger" || req.event_type == "decoy_interaction" {
        let _ = sqlx::query(
            "INSERT INTO honeypot_triggers (session_id, visitor_id, ip, user_agent)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(&req.session_id)
        .bind(req.visitor_id.as_deref())
        .bind(ip)
        .bind(ua)
        .execute(&state.db)
        .await;
    }

    // Create challenge record when required
    let challenge_payload = if fraud_score.action == ScoringAction::Challenge {
        let cid = Uuid::new_v4();
        let b   = cid.as_bytes();
        let target_x = 20.0 + (b[0] as f64 / 255.0) * 60.0;
        let target_y = 20.0 + (b[1] as f64 / 255.0) * 60.0;

        let ok = sqlx::query(
            "INSERT INTO challenges (id, session_id, expires_at, target_x, target_y)
             VALUES ($1, $2, NOW() + INTERVAL '5 minutes', $3, $4)",
        )
        .bind(cid)
        .bind(&req.session_id)
        .bind(target_x)
        .bind(target_y)
        .execute(&state.db)
        .await;

        if ok.is_ok() {
            Some(serde_json::json!({
                "challenge_id": cid.to_string(),
                "target_x":     target_x,
                "target_y":     target_y,
            }))
        } else { None }
    } else { None };

    // Broadcast to WebSocket clients
    let ws_msg = serde_json::json!({
        "type":       "new_event",
        "session_id": req.session_id,
        "event_type": req.event_type,
        "ip":         ip,
        "timestamp":  chrono::Utc::now().to_rfc3339(),
        "score":      fraud_score.score,
        "action":     fraud_score.action.to_string(),
        "reasons":    fraud_score.reasons,
    }).to_string();
    let _ = state.ws_tx.send(ws_msg);

    serde_json::json!({
        "ok":        true,
        "score":     fraud_score.score,
        "action":    fraud_score.action.to_string(),
        "challenge": challenge_payload,
    })
}

// ── route handlers ────────────────────────────────────────────────────────────

pub async fn ingest(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<IngestRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (key_id, ip, ua, owner_uid) = auth_and_extract(&state, &headers).await?;

    sqlx::query!(
        r#"INSERT INTO events (api_key_id, session_id, visitor_id, event_type, payload, ip, user_agent)
           VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
        key_id,
        req.session_id,
        req.visitor_id,
        req.event_type,
        req.payload.clone().unwrap_or(serde_json::json!({})),
        ip.as_deref(),
        ua.as_deref(),
    )
    .execute(&state.db)
    .await?;

    let prev_event = query_prev_event(&state, &req.session_id).await;

    let resp = process_event(
        &state, &req, key_id,
        ip.as_deref(), ua.as_deref(),
        owner_uid, prev_event.as_deref(),
    ).await;

    Ok(Json(resp))
}

/// POST /api/ingest/batch — send multiple events in a single request.
/// Events are processed in order; each gets its own score/action response.
pub async fn ingest_batch(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(batch): Json<BatchRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    if batch.events.is_empty() {
        return Ok(Json(serde_json::json!({ "ok": true, "responses": [] })));
    }

    let (key_id, ip, ua, owner_uid) = auth_and_extract(&state, &headers).await?;

    // Insert all events into the events table
    for req in &batch.events {
        let _ = sqlx::query!(
            r#"INSERT INTO events (api_key_id, session_id, visitor_id, event_type, payload, ip, user_agent)
               VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
            key_id,
            req.session_id,
            req.visitor_id,
            req.event_type,
            req.payload.clone().unwrap_or(serde_json::json!({})),
            ip.as_deref(),
            ua.as_deref(),
        )
        .execute(&state.db)
        .await;
    }

    // Process events in order, tracking prev_event within the batch
    // (avoids N separate DB queries — one initial query covers the first event)
    let first_session = &batch.events[0].session_id;
    let mut prev_event: Option<String> = query_prev_event(&state, first_session).await;

    let mut responses = Vec::with_capacity(batch.events.len());
    for req in &batch.events {
        let resp = process_event(
            &state, req, key_id,
            ip.as_deref(), ua.as_deref(),
            owner_uid, prev_event.as_deref(),
        ).await;
        prev_event = Some(req.event_type.clone());
        responses.push(resp);
    }

    Ok(Json(serde_json::json!({ "ok": true, "responses": responses })))
}

// ── helpers ───────────────────────────────────────────────────────────────────

async fn auth_and_extract(
    state:   &AppState,
    headers: &HeaderMap,
) -> Result<(Uuid, Option<String>, Option<String>, Option<Uuid>), AppError> {
    let api_key = headers
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Auth("missing X-API-Key header".into()))?;

    let key_row = sqlx::query!("SELECT id FROM api_keys WHERE key = $1", api_key)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::Auth("invalid API key".into()))?;

    let ip = headers
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("").trim().to_string());

    let ua = headers
        .get("User-Agent")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    let owner_uid = sqlx::query!("SELECT user_id FROM api_keys WHERE id = $1", key_row.id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .map(|r| r.user_id);

    Ok((key_row.id, ip, ua, owner_uid))
}

async fn query_prev_event(state: &AppState, session_id: &str) -> Option<String> {
    sqlx::query_scalar!(
        r#"SELECT event_type FROM events
           WHERE session_id = $1
           ORDER BY timestamp DESC
           LIMIT 1 OFFSET 1"#,
        session_id
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
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

    let fraud_alerts: i64 = sqlx::query(
        r#"SELECT COUNT(*) as c
           FROM session_scores ss
           JOIN api_keys k ON ss.api_key_id = k.id
           WHERE k.user_id = $1 AND ss.score > 0.65
             AND ss.created_at > NOW() - INTERVAL '24 hours'"#,
    )
    .bind(uid)
    .fetch_one(&state.db)
    .await
    .and_then(|r| r.try_get::<i64, _>("c"))
    .unwrap_or(0);

    Ok(Json(StatsResponse { dau, events_today, total_sessions, fraud_alerts }))
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

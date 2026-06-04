use axum::{extract::{ConnectInfo, Path, State}, http::HeaderMap, Json};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::net::SocketAddr;
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

/// Parse the SDK's user_id string into a UUID.
/// Valid UUID strings are used as-is; anything else is hashed via UUID v5.
fn resolve_user_id(s: &str) -> Uuid {
    Uuid::parse_str(s)
        .unwrap_or_else(|_| Uuid::new_v5(&Uuid::NAMESPACE_DNS, s.as_bytes()))
}

/// Build a namespace-scoped graph key: (tenant_id ⊗ subject_id) via UUID v5.
/// Ensures that the same end-user on two different tenants gets independent graphs.
fn namespace_graph_key(tenant_id: Uuid, subject_id: Uuid) -> Uuid {
    Uuid::new_v5(&tenant_id, subject_id.as_bytes())
}

/// Resolve the L1 graph key using the correct identity hierarchy:
///   1. Authenticated end-user (user_id) — strong, survives browser changes
///   2. Anonymous visitor (visitor_id)   — weaker, local to this device/cookie
///   3. None                             — no identity, L1 is skipped
///
/// tenant_id namespaces the graph so the same end-user on two sites stays separate.
fn resolve_graph_key(
    tenant_id:  Option<Uuid>,
    user_id:    Option<&str>,
    visitor_id: Option<&str>,
) -> Option<Uuid> {
    let tenant = tenant_id?;
    match (user_id, visitor_id) {
        (Some(uid), _) => {
            Some(namespace_graph_key(tenant, resolve_user_id(uid)))
        }
        (None, Some(vid)) => {
            let vid_uuid = Uuid::new_v5(&Uuid::NAMESPACE_DNS, vid.as_bytes());
            Some(namespace_graph_key(tenant, vid_uuid))
        }
        (None, None) => None,
    }
}

/// Core per-event processing: score, persist, optionally create challenge.
/// Returns the JSON body to send back for this one event.
async fn process_event(
    state:      &AppState,
    req:        &IngestRequest,
    key_id:     Uuid,
    ip:         Option<&str>,
    ua:         Option<&str>,
    tenant_id:  Option<Uuid>,   // merchant who owns this API key
    prev_event: Option<&str>,
) -> serde_json::Value {
    // Account is primary, device is secondary — never fall back to the merchant's own identity
    let graph_key = resolve_graph_key(
        tenant_id,
        req.user_id.as_deref(),
        req.visitor_id.as_deref(),
    );

    let fraud_score = state.fraud.score_event(
        &req.session_id,
        graph_key,
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
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(req): Json<IngestRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (key_id, ip, ua, tenant_id) = auth_and_extract(&state, &headers, Some(peer)).await?;

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
        tenant_id, prev_event.as_deref(),
    ).await;

    Ok(Json(resp))
}

/// POST /api/ingest/batch — send multiple events in a single request.
/// Events are processed in order; each gets its own score/action response.
pub async fn ingest_batch(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(batch): Json<BatchRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    if batch.events.is_empty() {
        return Ok(Json(serde_json::json!({ "ok": true, "responses": [] })));
    }

    let (key_id, ip, ua, tenant_id) = auth_and_extract(&state, &headers, Some(peer)).await?;

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
            tenant_id, prev_event.as_deref(),
        ).await;
        prev_event = Some(req.event_type.clone());
        responses.push(resp);
    }

    Ok(Json(serde_json::json!({ "ok": true, "responses": responses })))
}

// ── helpers ───────────────────────────────────────────────────────────────────

async fn auth_and_extract(
    state:    &AppState,
    headers:  &HeaderMap,
    peer:     Option<SocketAddr>,
) -> Result<(Uuid, Option<String>, Option<String>, Option<Uuid>), AppError> {
    let api_key = headers
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Auth("missing X-API-Key header".into()))?;

    let key_row = sqlx::query!("SELECT id FROM api_keys WHERE key = $1", api_key)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::Auth("invalid API key".into()))?;

    // IP priority: X-Forwarded-For (reverse proxy) → X-Real-IP → direct connection
    let ip = headers
        .get("X-Forwarded-For")
        .or_else(|| headers.get("X-Real-IP"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("").trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| peer.map(|a| a.ip().to_string()));

    let ua = headers
        .get("User-Agent")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    let tenant_id = sqlx::query!("SELECT user_id FROM api_keys WHERE id = $1", key_row.id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .map(|r| r.user_id);

    Ok((key_row.id, ip, ua, tenant_id))
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

/// GET /api/events/scores/:session_id
/// Returns all fraud scores recorded for this session (newest first).
/// Used by the dashboard to show per-event score breakdown on click.
pub async fn session_scores(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(session_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let uid = Uuid::parse_str(&auth.user_id).unwrap();

    let rows = sqlx::query(
        r#"SELECT ss.id, ss.event_type, ss.score, ss.l1_score, ss.l2_score,
                  ss.l3_score, ss.embedding_score, ss.action, ss.reasons, ss.created_at
           FROM session_scores ss
           JOIN api_keys k ON ss.api_key_id = k.id
           WHERE ss.session_id = $1 AND k.user_id = $2
           ORDER BY ss.created_at DESC
           LIMIT 30"#,
    )
    .bind(&session_id)
    .bind(uid)
    .fetch_all(&state.db)
    .await?;

    let scores: Vec<serde_json::Value> = rows.iter().map(|r| {
        serde_json::json!({
            "id":              r.try_get::<Uuid, _>("id").map(|u| u.to_string()).unwrap_or_default(),
            "event_type":      r.try_get::<Option<String>, _>("event_type").unwrap_or(None),
            "score":           r.try_get::<f64, _>("score").unwrap_or(0.0),
            "l1":              r.try_get::<Option<f64>, _>("l1_score").unwrap_or(None),
            "l2":              r.try_get::<Option<f64>, _>("l2_score").unwrap_or(None),
            "l3":              r.try_get::<Option<f64>, _>("l3_score").unwrap_or(None),
            "embedding":       r.try_get::<Option<f64>, _>("embedding_score").unwrap_or(None),
            "action":          r.try_get::<String, _>("action").unwrap_or_default(),
            "reasons":         r.try_get::<serde_json::Value, _>("reasons").unwrap_or(serde_json::json!([])),
            "timestamp":       r.try_get::<chrono::DateTime<chrono::Utc>, _>("created_at")
                                 .map(|t| t.to_rfc3339())
                                 .unwrap_or_default(),
        })
    }).collect();

    Ok(Json(serde_json::json!({ "session_id": session_id, "scores": scores })))
}

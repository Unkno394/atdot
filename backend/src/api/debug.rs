use axum::{extract::State, Json};
use serde::Serialize;
use sqlx::Row;
use std::sync::Arc;

use crate::{error::AppError, state::AppState};

#[derive(Serialize)]
pub struct ScoreEntry {
    session_id:      String,
    event_type:      Option<String>,
    score:           f64,
    l1:              Option<f64>,
    l2:              Option<f64>,
    l3:              Option<f64>,
    embedding_score: Option<f64>,
    action:          String,
    reasons:         serde_json::Value,
    created_at:      String,
}

#[derive(Serialize)]
pub struct ActionDist {
    allow:     i64,
    challenge: i64,
    block:     i64,
}

#[derive(Serialize)]
pub struct DebugStats {
    recent_scores:    Vec<ScoreEntry>,
    action_dist:      ActionDist,
    avg_score:        f64,
    high_risk_count:  i64,
    total_scored:     i64,
}

pub async fn debug_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<DebugStats>, AppError> {

    let rows = sqlx::query(
        r#"SELECT session_id, event_type, score, l1_score, l2_score, l3_score,
                  embedding_score, action, reasons, created_at
           FROM session_scores
           ORDER BY created_at DESC
           LIMIT 50"#,
    )
    .fetch_all(&state.db)
    .await?;

    let recent_scores = rows.iter().map(|r| ScoreEntry {
        session_id:      r.try_get("session_id").unwrap_or_default(),
        event_type:      r.try_get("event_type").unwrap_or(None),
        score:           r.try_get("score").unwrap_or(0.0),
        l1:              r.try_get("l1_score").unwrap_or(None),
        l2:              r.try_get("l2_score").unwrap_or(None),
        l3:              r.try_get("l3_score").unwrap_or(None),
        embedding_score: r.try_get("embedding_score").unwrap_or(None),
        action:          r.try_get("action").unwrap_or_default(),
        reasons:         r.try_get("reasons").unwrap_or(serde_json::json!([])),
        created_at:      r.try_get::<chrono::DateTime<chrono::Utc>, _>("created_at")
                          .map(|t| t.to_rfc3339())
                          .unwrap_or_default(),
    }).collect();

    let dist_row = sqlx::query(
        r#"SELECT
             COUNT(*) FILTER (WHERE action = 'allow')     AS allow,
             COUNT(*) FILTER (WHERE action = 'challenge') AS challenge,
             COUNT(*) FILTER (WHERE action = 'block')     AS block
           FROM session_scores"#,
    )
    .fetch_one(&state.db)
    .await?;

    let action_dist = ActionDist {
        allow:     dist_row.try_get("allow").unwrap_or(0),
        challenge: dist_row.try_get("challenge").unwrap_or(0),
        block:     dist_row.try_get("block").unwrap_or(0),
    };

    let agg = sqlx::query(
        r#"SELECT
             COUNT(*)                              AS total,
             COALESCE(AVG(score), 0)               AS avg_score,
             COUNT(*) FILTER (WHERE score > 0.65)  AS high_risk
           FROM session_scores"#,
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(DebugStats {
        recent_scores,
        action_dist,
        avg_score:       agg.try_get("avg_score").unwrap_or(0.0),
        high_risk_count: agg.try_get("high_risk").unwrap_or(0),
        total_scored:    agg.try_get("total").unwrap_or(0),
    }))
}

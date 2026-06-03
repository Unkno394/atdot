use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

const MIN_USERS_FOR_CANDIDATE: usize = 5;
const MIN_HUMANITY_AVG:        f64   = 0.65;
const MIN_HUMAN_FRACTION:      f64   = 0.80;
const MIN_TEMPORAL_ENTROPY:    f64   = 1.50;
const QUARANTINE_DAYS:         i64   = 3;

/// Score a session sequence against the global promoted pattern database.
///
/// Returns 0.0 when the sequence closely matches a known-human pattern,
/// up to 0.15 for unknown sequences. Uses LCS similarity so partial matches
/// also reduce the penalty (avoids false positives on slight variations).
pub async fn score(db: &PgPool, sequence: &[String]) -> f32 {
    if sequence.len() < 2 { return 0.0; }

    let seq_json = serde_json::to_value(sequence).unwrap_or_default();

    // Fast path: exact match with a promoted human pattern
    let exact: i64 = sqlx::query(
        "SELECT COUNT(*) as c FROM pattern_candidates WHERE status = 'promoted' AND sequence = $1",
    )
    .bind(&seq_json)
    .fetch_one(db)
    .await
    .and_then(|r| r.try_get::<i64, _>("c"))
    .unwrap_or(0);

    if exact > 0 { return 0.0; }

    // Similarity search: fetch promoted patterns and find best LCS match.
    // Capped at 200 rows so this stays O(1) in DB work regardless of total patterns.
    let rows = sqlx::query(
        "SELECT sequence FROM pattern_candidates WHERE status = 'promoted' LIMIT 200",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let best_similarity = rows.iter()
        .filter_map(|r| r.try_get::<serde_json::Value, _>("sequence").ok())
        .filter_map(|v| serde_json::from_value::<Vec<String>>(v).ok())
        .map(|pat| lcs_similarity(sequence, &pat))
        .fold(0.0f32, f32::max);

    if best_similarity >= 0.85 {
        // Near-identical to a known-human pattern — treat as human
        0.0
    } else if best_similarity >= 0.50 {
        // Partial match: scale penalty by how dissimilar it is
        (1.0 - best_similarity) * 0.10
    } else {
        // No known pattern — mild suspicion
        0.15
    }
}

/// Longest Common Subsequence similarity: LCS_length / max(len_a, len_b).
/// 1.0 = identical sequences, 0.0 = completely different.
fn lcs_similarity(a: &[String], b: &[String]) -> f32 {
    let (m, n) = (a.len(), b.len());
    if m == 0 || n == 0 { return 0.0; }

    // Rolling two-row DP — O(m*n) time, O(n) space
    let mut prev = vec![0u16; n + 1];
    let mut curr = vec![0u16; n + 1];

    for i in 1..=m {
        for j in 1..=n {
            curr[j] = if a[i - 1] == b[j - 1] {
                prev[j - 1] + 1
            } else {
                curr[j - 1].max(prev[j])
            };
        }
        std::mem::swap(&mut prev, &mut curr);
        curr.fill(0);
    }

    prev[n] as f32 / m.max(n) as f32
}

/// 4-level promotion gate. Called when a pattern accumulates 5+ contributing sessions.
pub async fn try_promote(
    db:                    &PgPool,
    sequence:              &[String],
    contributing_sessions: &[String],
    humanity_scores:       &[f32],
    session_timestamps:    &[DateTime<Utc>],
) -> PromotionDecision {
    if humanity_scores.len() < MIN_USERS_FOR_CANDIDATE {
        return PromotionDecision::Pending;
    }

    // GATE 1 — humanity
    let avg_humanity = humanity_scores.iter().sum::<f32>() as f64 / humanity_scores.len() as f64;
    let human_fraction = humanity_scores.iter().filter(|&&s| s > 0.7).count() as f64
        / humanity_scores.len() as f64;

    if avg_humanity < MIN_HUMANITY_AVG || human_fraction < MIN_HUMAN_FRACTION {
        return PromotionDecision::Reject("low humanity score".into());
    }

    // GATE 2 — temporal entropy (synchronized sessions = scripted attack)
    let entropy = temporal_entropy(session_timestamps);
    if entropy < MIN_TEMPORAL_ENTROPY {
        return PromotionDecision::Reject("synchronized sessions (possible scripted attack)".into());
    }

    // GATE 3 — behavioral diversity
    if behavioral_diversity(humanity_scores) < 0.1 {
        return PromotionDecision::Reject("no behavioral diversity (too similar)".into());
    }

    // GATE 4 — quarantine (3-day observation period before full promotion)
    let seq_json      = serde_json::to_value(sequence).unwrap_or_default();
    let sessions_json = serde_json::to_value(contributing_sessions).unwrap_or_default();
    let humanity_json = serde_json::to_value(humanity_scores).unwrap_or_default();
    let ts_json = serde_json::to_value(
        session_timestamps.iter().map(|t| t.to_rfc3339()).collect::<Vec<_>>()
    ).unwrap_or_default();

    let existing = sqlx::query(
        "SELECT id, status, quarantine_until FROM pattern_candidates WHERE sequence = $1",
    )
    .bind(&seq_json)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    match existing {
        None => {
            let quarantine_until = Utc::now() + chrono::Duration::days(QUARANTINE_DAYS);
            let _ = sqlx::query(
                r#"INSERT INTO pattern_candidates
                   (sequence, status, contributing, humanity_scores, timestamps,
                    quarantine_until, humanity_avg, temporal_entropy)
                   VALUES ($1, 'quarantine', $2, $3, $4, $5, $6, $7)"#,
            )
            .bind(&seq_json).bind(&sessions_json).bind(&humanity_json).bind(&ts_json)
            .bind(quarantine_until).bind(avg_humanity).bind(entropy)
            .execute(db).await;
            PromotionDecision::Quarantine
        }
        Some(row) => {
            let status:            String            = row.try_get("status").unwrap_or_default();
            let quarantine_until:  Option<DateTime<Utc>> = row.try_get("quarantine_until").unwrap_or(None);
            let id:                Uuid              = row.try_get("id").unwrap_or_else(|_| Uuid::new_v4());

            if status == "promoted" { return PromotionDecision::Promote; }

            if status == "quarantine" {
                let until = quarantine_until.unwrap_or_else(Utc::now);
                if Utc::now() < until { return PromotionDecision::Quarantine; }
                let _ = sqlx::query(
                    "UPDATE pattern_candidates SET status = 'promoted' WHERE id = $1",
                )
                .bind(id).execute(db).await;
                return PromotionDecision::Promote;
            }
            PromotionDecision::Pending
        }
    }
}

#[derive(Debug, Clone)]
pub enum PromotionDecision {
    Promote,
    Quarantine,
    Pending,
    Reject(String),
}

fn temporal_entropy(timestamps: &[DateTime<Utc>]) -> f64 {
    if timestamps.len() < 2 { return 0.0; }
    let mut day_counts: std::collections::HashMap<i64, u32> = std::collections::HashMap::new();
    for ts in timestamps {
        *day_counts.entry(ts.timestamp() / 86400).or_insert(0) += 1;
    }
    let n = timestamps.len() as f64;
    -day_counts.values()
        .map(|&c| { let p = c as f64 / n; p * p.ln() })
        .sum::<f64>()
}

fn behavioral_diversity(scores: &[f32]) -> f32 {
    if scores.len() < 2 { return 0.0; }
    let mean = scores.iter().sum::<f32>() / scores.len() as f32;
    let var  = scores.iter().map(|s| (s - mean).powi(2)).sum::<f32>() / (scores.len() - 1) as f32;
    var.sqrt()
}

use chrono::Utc;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use uuid::Uuid;

pub use crate::fraud::l1::FLUSH_INTERVAL;

use crate::fraud::{
    features::EventFeatures,
    geoip::GeoIp,
    l1::L1Store,
    l2, l3,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScoringAction { Allow, Challenge, Block }

impl std::fmt::Display for ScoringAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScoringAction::Allow     => write!(f, "allow"),
            ScoringAction::Challenge => write!(f, "challenge"),
            ScoringAction::Block     => write!(f, "block"),
        }
    }
}

#[derive(Debug)]
pub struct FraudScore {
    pub score:           f32,
    pub l1:              f32,
    pub l2:              f32,
    pub l3:              f32,
    pub embedding_score: f32,
    pub action:          ScoringAction,
    pub reasons:         Vec<String>,
}

pub struct FraudEngineInner {
    pub l1:  L1Store,
    pub geo: GeoIp,
    pub db:  PgPool,
}

pub type FraudEngine = Arc<FraudEngineInner>;

impl FraudEngineInner {
    pub fn new(db: PgPool, graphs_path: &str) -> anyhow::Result<Arc<Self>> {
        let l1 = L1Store::open(graphs_path)?;
        let vpn_tree = l1.open_tree("vpn_ips")?;
        Ok(Arc::new(Self { geo: GeoIp::new(vpn_tree), l1, db }))
    }

    pub async fn score_event(
        &self,
        session_id:  &str,
        user_id:     Option<Uuid>,
        event_type:  &str,
        payload:     &serde_json::Value,
        ip:          Option<&str>,
        visitor_id:  Option<&str>,
        user_agent:  Option<&str>,
        webrtc_ip:   Option<&str>,
        ipv6:        Option<&str>,
        timezone:    Option<&str>,
        fingerprint: Option<&str>,
        prev_event:  Option<&str>,
    ) -> FraudScore {
        let mut reasons = Vec::new();

        // Honeypot / decoy click: immediate maximum score — automated bot confirmed
        if event_type == "honeypot_trigger" || event_type == "decoy_interaction" {
            reasons.push("honeypot triggered — automated bot detected".into());
            return FraudScore {
                score: 1.0, l1: 1.0, l2: 1.0, l3: 1.0,
                embedding_score: 1.0,
                action: ScoringAction::Block,
                reasons,
            };
        }

        let ef = EventFeatures::from_payload(event_type, payload);

        // L2 runs async before acquiring any graph lock
        let l2_score = l2::score(
            &self.db, &self.geo,
            ip, visitor_id, user_agent,
            webrtc_ip, ipv6, timezone, fingerprint,
        ).await;
        if l2_score > 0.5 {
            reasons.push(format!("L2 network anomaly: {:.2}", l2_score));
        }

        let (l1_score, path_score, familiarity, l3_score, embedding_s,
             continuity_s, completed_seq, humanity_score) = match user_id {
            None => (0.1, 0.0, 0.0, 0.0, 0.2, 0.0, None, 0.5),
            Some(uid) => {
                let r = self.l1.score(uid, prev_event, &ef);
                reasons.extend(r.reasons);

                let l3_s = if r.session_seq.len() >= 3 {
                    let s = l3::score(&self.db, &r.session_seq).await;
                    if s > 0.1 {
                        reasons.push(format!("L3 unknown global pattern: {:.2}", s));
                    }
                    s
                } else { 0.0 };

                if r.continuity_score > 0.4 {
                    reasons.push(format!(
                        "continuity break (possible handoff): {:.2}", r.continuity_score
                    ));
                }

                (r.l1_score, r.path_score, r.familiarity, l3_s,
                 r.embedding_score, r.continuity_score,
                 r.completed_seq, r.humanity_score)
            }
        };

        let ho_score    = higher_order_score(&ef, &mut reasons);
        let combined_ho = (ho_score * 0.7 + path_score * 0.3).clamp(0.0, 1.0);

        if embedding_s > 0.6 {
            reasons.push(format!("embedding anomaly: {:.2}", embedding_s));
        }

        let s_rate = (l1_score    * 0.32
                    + l2_score    * 0.23
                    + embedding_s * 0.18
                    + combined_ho * 0.15
                    + continuity_s * 0.07
                    + l3_score    * 0.05).clamp(0.0, 1.0);

        let score = conditional_anomaly(s_rate, event_global_risk(event_type), familiarity, 0.4);

        // Adaptive threshold: tighter for predictable users, looser for variable ones
        let block_thresh = match user_id {
            Some(uid) => self.l1.get_adaptive_threshold(uid),
            None      => 0.75,
        };
        let action = action_from_score(score, block_thresh);

        tracing::info!(
            event      = event_type,
            score      = format!("{:.3}", score),
            action     = %action,
            threshold  = format!("{:.2}", block_thresh),
            l1         = format!("{:.3}", l1_score),
            l2         = format!("{:.3}", l2_score),
            l3         = format!("{:.3}", l3_score),
            embed      = format!("{:.3}", embedding_s),
            continuity = format!("{:.3}", continuity_s),
            ho         = format!("{:.3}", ho_score),
            reasons    = ?reasons,
            "fraud_score"
        );

        // Trigger L3 pattern accumulation when a session ends
        if let Some(seq) = completed_seq {
            if seq.len() >= 3 {
                let db   = self.db.clone();
                let sid  = session_id.to_string();
                tokio::spawn(async move {
                    accumulate_pattern(&db, &seq, &sid, humanity_score).await;
                });
            }
        }

        FraudScore {
            score,
            l1: l1_score,
            l2: l2_score,
            l3: l3_score,
            embedding_score: embedding_s,
            action,
            reasons,
        }
    }

    pub fn flush_cold_graphs(&self) { self.l1.flush_cold_graphs(); }

    pub fn apply_fraud_signal(&self, user_id: uuid::Uuid, confirmed_fraud: bool) {
        self.l1.apply_fraud_signal(user_id, confirmed_fraud);
    }
}

/// Accumulate a completed session into pattern_candidates, then call try_promote
/// if we have 5+ contributing sessions.
async fn accumulate_pattern(
    db:         &PgPool,
    sequence:   &[String],
    session_id: &str,
    humanity:   f32,
) {
    let seq_json = serde_json::to_value(sequence).unwrap_or_default();
    let now = Utc::now();

    let existing = sqlx::query(
        "SELECT id, contributing, humanity_scores, timestamps
         FROM pattern_candidates
         WHERE sequence = $1 AND status != 'promoted'",
    )
    .bind(&seq_json)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    match existing {
        None => {
            let _ = sqlx::query(
                r#"INSERT INTO pattern_candidates
                   (sequence, status, contributing, humanity_scores, timestamps)
                   VALUES ($1, 'pending', $2, $3, $4)
                   ON CONFLICT DO NOTHING"#,
            )
            .bind(&seq_json)
            .bind(serde_json::json!([session_id]))
            .bind(serde_json::json!([humanity]))
            .bind(serde_json::json!([now.to_rfc3339()]))
            .execute(db)
            .await;
        }
        Some(row) => {
            let id: Uuid = row.try_get("id").unwrap_or_else(|_| Uuid::new_v4());

            let mut contribs: Vec<String> = row.try_get::<serde_json::Value, _>("contributing")
                .ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
            let mut scores: Vec<f32> = row.try_get::<serde_json::Value, _>("humanity_scores")
                .ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
            let mut tss: Vec<String> = row.try_get::<serde_json::Value, _>("timestamps")
                .ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();

            if contribs.contains(&session_id.to_string()) { return; }

            contribs.push(session_id.to_string());
            scores.push(humanity);
            tss.push(now.to_rfc3339());

            let _ = sqlx::query(
                "UPDATE pattern_candidates
                 SET contributing=$1, humanity_scores=$2, timestamps=$3
                 WHERE id=$4",
            )
            .bind(serde_json::to_value(&contribs).unwrap_or_default())
            .bind(serde_json::to_value(&scores).unwrap_or_default())
            .bind(serde_json::to_value(&tss).unwrap_or_default())
            .bind(id)
            .execute(db)
            .await;

            if contribs.len() >= 5 {
                let timestamps: Vec<chrono::DateTime<Utc>> = tss.iter()
                    .filter_map(|t| t.parse().ok())
                    .collect();
                let decision = l3::try_promote(db, sequence, &contribs, &scores, &timestamps).await;
                tracing::info!(
                    seq_len  = sequence.len(),
                    sessions = contribs.len(),
                    decision = ?decision,
                    "L3 promotion"
                );
            }
        }
    }
}

fn conditional_anomaly(s_rate: f32, global_risk: f32, familiarity: f32, alpha: f32) -> f32 {
    (s_rate * (1.0 + alpha * global_risk * (1.0 - familiarity))).clamp(0.0, 1.0)
}

/// Use the user's adaptive threshold instead of a hardcoded constant.
fn action_from_score(score: f32, block_thresh: f32) -> ScoringAction {
    let challenge_thresh = block_thresh * 0.76;
    if score >= block_thresh      { ScoringAction::Block }
    else if score >= challenge_thresh { ScoringAction::Challenge }
    else                          { ScoringAction::Allow }
}

fn event_global_risk(event_type: &str) -> f32 {
    match event_type {
        "purchase" | "checkout"   => 0.90,
        "login"                   => 0.80,
        "password_change"         => 1.00,
        "withdrawal" | "transfer" => 1.00,
        "registration"            => 0.70,
        "click"                   => 0.20,
        "page_view"               => 0.10,
        "page_hide" | "scroll"    => 0.05,
        _                         => 0.30,
    }
}

fn higher_order_score(ef: &EventFeatures, reasons: &mut Vec<String>) -> f32 {
    if ef.event_type != "click" { return 0.0; }

    let linearity = ef.mouse_linearity;
    let traj_len  = ef.trajectory_len as u64;
    let pause_ms  = ef.pause_ms;
    let mut score = 0.0f32;

    if linearity > 0.95 && traj_len > 5 {
        score += 0.30;
        reasons.push(format!("perfect mouse trajectory (linearity={:.2})", linearity));
    }
    if pause_ms < 80.0 && traj_len > 0 {
        score += 0.25;
        reasons.push(format!("near-instant click ({:.0}ms)", pause_ms));
    }
    if pause_ms < 150.0 && linearity > 0.90 {
        score += 0.20;
        reasons.push("no novelty response (low pause + high linearity)".into());
    }

    score.clamp(0.0, 1.0)
}

use serde_json::Value;
use std::collections::HashSet;

use crate::fraud::embedding::{feat, SessionVector, DIM};

/// Raw features extracted from a single event's payload
#[derive(Debug, Clone, Default)]
pub struct EventFeatures {
    pub pause_ms:         f64,
    pub mouse_linearity:  f32,
    pub trajectory_len:   u32,
    pub click_x:          Option<f32>,
    pub click_y:          Option<f32>,
    pub scroll_depth:     Option<f32>,
    pub event_type:       String,
    pub fitts_id:          Option<f32>,
        pub hover_duration_ms: Option<f64>,
        pub micro_corrections: Option<u32>,
        pub max_velocity:      Option<f32>,
        pub final_velocity:    Option<f32>,
        pub is_new_element:    Option<bool>,
}

impl EventFeatures {
    pub fn from_payload(event_type: &str, payload: &Value) -> Self {
        Self {
            pause_ms:          payload["pause_ms"].as_f64().unwrap_or(0.0),
            mouse_linearity:   payload["mouse_linearity"].as_f64().unwrap_or(0.0) as f32,
            trajectory_len:    payload["trajectory_len"].as_u64().unwrap_or(0) as u32,
            click_x:           payload["x"].as_f64().map(|v| v as f32),
            click_y:           payload["y"].as_f64().map(|v| v as f32),
            scroll_depth:      payload["scroll_depth"].as_f64().map(|v| v as f32),
            event_type:        event_type.to_string(),
            fitts_id:          payload["fitts_id"].as_f64().map(|v| v as f32),
            hover_duration_ms: payload["hover_duration_ms"].as_f64(),
            micro_corrections: payload["micro_corrections"].as_u64().map(|v| v as u32),
            max_velocity:      payload["max_velocity"].as_f64().map(|v| v as f32),
            final_velocity:    payload["final_velocity"].as_f64().map(|v| v as f32),
            is_new_element:    payload["is_new_element"].as_bool(),
        }
    }
}

/// Aggregated session-level feature vector used for embedding
pub struct SessionAccumulator {
    pub events:           Vec<EventFeatures>,
    pub first_ts_ms:      i64,
    pub last_ts_ms:       i64,
}

impl SessionAccumulator {
    pub fn new(first_ts_ms: i64) -> Self {
        Self { events: Vec::new(), first_ts_ms, last_ts_ms: first_ts_ms }
    }

    pub fn add(&mut self, ef: EventFeatures, ts_ms: i64) {
        self.last_ts_ms = ts_ms;
        self.events.push(ef);
    }

    pub fn to_vector(&self) -> SessionVector {
        let n = self.events.len();
        if n == 0 { return SessionVector::default(); }

        let mut v = [0.0f32; DIM];

        let pauses: Vec<f64> = self.events.iter().map(|e| e.pause_ms).collect();
        let avg_pause = pauses.iter().sum::<f64>() / n as f64;
        let pause_var = variance_f64(&pauses);

        let linearities: Vec<f32> = self.events.iter()
            .filter(|e| e.trajectory_len > 0)
            .map(|e| e.mouse_linearity)
            .collect();
        let avg_lin = if linearities.is_empty() { 0.5 }
                      else { linearities.iter().sum::<f32>() / linearities.len() as f32 };
        let lin_var = variance_f32(&linearities);

        let duration_s = ((self.last_ts_ms - self.first_ts_ms).max(0) as f64 / 1000.0) as f32;
        let click_count = self.events.iter().filter(|e| e.event_type == "click").count();
        let clicks_per_min = if duration_s > 0.0 { click_count as f32 / (duration_s / 60.0) } else { 0.0 };

        let scroll_depth = self.events.iter()
            .filter_map(|e| e.scroll_depth)
            .fold(0.0f32, f32::max);

        let unique_types: HashSet<&str> = self.events.iter()
            .map(|e| e.event_type.as_str())
            .collect();

        let traj_lengths: Vec<f32> = self.events.iter()
            .filter(|e| e.trajectory_len > 0)
            .map(|e| e.trajectory_len as f32)
            .collect();
        
        let traj_var = variance_f32(&traj_lengths);
        
        v[feat::AVG_PAUSE_MS]        = (avg_pause / 1000.0) as f32;
        v[feat::PAUSE_VARIANCE]      = (pause_var.sqrt() / 1000.0) as f32;
        v[feat::CLICK_FREQUENCY]     = clicks_per_min;
        v[feat::AVG_MOUSE_LINEARITY] = avg_lin;
        v[feat::LINEARITY_VARIANCE]  = lin_var;
        v[feat::SCROLL_DEPTH]        = scroll_depth;
        v[feat::SESSION_DURATION_S]  = duration_s;
        v[feat::EVENT_COUNT]         = n as f32;
        v[feat::UNIQUE_EVENT_TYPES]  = unique_types.len() as f32;
        v[feat::SCAN_ENTROPY]        = scan_entropy(&self.events);
        v[feat::TRAJECTORY_VARIANCE] = traj_var;

        // ── cognitive / motor features (require enriched click fields) ────────
        let clicks: Vec<&EventFeatures> = self.events.iter()
            .filter(|e| e.event_type == "click")
            .collect();

        // Fitts compliance: ratio of actual pause to Fitts-predicted time T = 100 + 150·fitts_id
        let fitts_compliance = {
            let ratios: Vec<f32> = clicks.iter()
                .filter_map(|e| {
                    let fid = e.fitts_id?;
                    let predicted_ms = 100.0 + 150.0 * fid;
                    Some((e.pause_ms as f32 / predicted_ms).min(3.0) / 3.0)
                })
                .collect();
            if ratios.is_empty() { 0.5 } else { ratios.iter().sum::<f32>() / ratios.len() as f32 }
        };

        // Hover latency: mean hover duration normalized over 3000 ms
        let hover_latency = {
            let hovers: Vec<f64> = clicks.iter().filter_map(|e| e.hover_duration_ms).collect();
            if hovers.is_empty() { 0.0f32 } else {
                let mean_ms = hovers.iter().sum::<f64>() / hovers.len() as f64;
                (mean_ms / 3000.0).min(1.0) as f32
            }
        };

        // Micro-correction rate: mean direction-changes / trajectory_len
        let micro_correction_rate = {
            let rates: Vec<f32> = clicks.iter()
                .filter_map(|e| {
                    let mc = e.micro_corrections? as f32;
                    let tl = e.trajectory_len as f32;
                    if tl > 0.0 { Some(mc / tl) } else { None }
                })
                .collect();
            if rates.is_empty() { 0.0f32 }
            else { (rates.iter().sum::<f32>() / rates.len() as f32).min(1.0) }
        };

        // Velocity profile: 1 − (final_vel / max_vel) — S-curve deceleration signal
        let velocity_profile = {
            let profiles: Vec<f32> = clicks.iter()
                .filter_map(|e| {
                    let max_v = e.max_velocity?;
                    let fin_v = e.final_velocity?;
                    if max_v > 0.001 { Some(1.0 - (fin_v / max_v).min(1.0)) } else { None }
                })
                .collect();
            if profiles.is_empty() { 0.5f32 }
            else { profiles.iter().sum::<f32>() / profiles.len() as f32 }
        };

        // Novelty response: humans pause longer on first-seen elements
        let novelty_response = {
            let new_pauses: Vec<f64> = clicks.iter()
                .filter(|e| e.is_new_element == Some(true))
                .map(|e| e.pause_ms)
                .collect();
            let old_pauses: Vec<f64> = clicks.iter()
                .filter(|e| e.is_new_element == Some(false))
                .map(|e| e.pause_ms)
                .collect();
            if new_pauses.is_empty() || old_pauses.is_empty() {
                0.5f32
            } else {
                let avg_new = new_pauses.iter().sum::<f64>() / new_pauses.len() as f64;
                let avg_old = old_pauses.iter().sum::<f64>() / old_pauses.len() as f64;
                ((avg_new / avg_old.max(1.0)) as f32).min(2.0) / 2.0
            }
        };

        v[feat::FITTS_COMPLIANCE]       = fitts_compliance;
        v[feat::HOVER_LATENCY]          = hover_latency;
        v[feat::MICRO_CORRECTION_RATE]  = micro_correction_rate;
        v[feat::VELOCITY_PROFILE]       = velocity_profile;
        v[feat::NOVELTY_RESPONSE]       = novelty_response;

        SessionVector { features: v }
    }
}

/// Shannon entropy of event type sequence — low = bot-like (repetitive), high = human-like
fn scan_entropy(events: &[EventFeatures]) -> f32 {
    if events.is_empty() { return 0.0; }
    let mut counts: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
    for e in events { *counts.entry(e.event_type.as_str()).or_insert(0) += 1; }
    let n = events.len() as f32;
    -counts.values()
        .map(|&c| { let p = c as f32 / n; p * p.ln() })
        .sum::<f32>()
}

/// Humanity score: 0.0 = definitely bot, 1.0 = definitely human
/// Based on mouse behavior and timing variance
pub fn humanity_score(events: &[EventFeatures]) -> f32 {
    if events.is_empty() { return 0.5; }

    let pauses: Vec<f64> = events.iter().map(|e| e.pause_ms).collect();
    let avg_pause = pauses.iter().sum::<f64>() / pauses.len() as f64;
    let pause_variance = variance_f64(&pauses);

    let click_events: Vec<&EventFeatures> = events.iter()
        .filter(|e| e.event_type == "click" && e.trajectory_len > 0)
        .collect();
    let avg_linearity = if click_events.is_empty() { 0.5f32 }
    else {
        click_events.iter().map(|e| e.mouse_linearity).sum::<f32>() / click_events.len() as f32
    };

    let mut score = 0.5f32;

    // Perfect linearity = bot signal
    if avg_linearity > 0.95      { score -= 0.3; }
    else if avg_linearity < 0.75 { score += 0.15; }

    // Machine-like speed (< 150ms average) = suspicious
    if avg_pause < 150.0      { score -= 0.25; }
    else if avg_pause > 400.0 { score += 0.10; }

    // Variance in timing = human signal
    if pause_variance > 50000.0  { score += 0.20; }  // std > ~224ms
    else if pause_variance < 500.0 { score -= 0.15; } // std < ~22ms, robotic

    // Short sessions with too many events = bot
    let events_per_sec = events.len() as f64 / (avg_pause * events.len() as f64 / 1000.0).max(1.0);
    if events_per_sec > 5.0 { score -= 0.2; }

    score.clamp(0.0, 1.0)
}

/// Novelty response: detect if a user behaved like a human encountering something new
/// Humans slow down and show less certainty on unfamiliar flows
pub fn novelty_response(events: &[EventFeatures], is_new_flow: bool) -> f32 {
    if !is_new_flow || events.is_empty() { return 0.5; }

    let avg_pause = events.iter().map(|e| e.pause_ms).sum::<f64>() / events.len() as f64;
    let avg_linearity = events.iter()
        .filter(|e| e.trajectory_len > 0)
        .map(|e| e.mouse_linearity)
        .sum::<f32>()
        .max(0.001) / events.len() as f32;

    // Bot knows coordinates even on new flows → immediate, linear
    // Human reads first → slower, less linear
    let speed_human  = if avg_pause > 600.0 { 1.0f32 } else if avg_pause < 200.0 { 0.0 } else { (avg_pause as f32 - 200.0) / 400.0 };
    let path_human   = if avg_linearity < 0.8 { 1.0f32 } else { 1.0 - (avg_linearity - 0.8) / 0.2 };

    (speed_human * 0.5 + path_human * 0.5).clamp(0.0, 1.0)
}

fn variance_f64(values: &[f64]) -> f64 {
    if values.len() < 2 { return 0.0; }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (values.len() - 1) as f64
}

fn variance_f32(values: &[f32]) -> f32 {
    if values.len() < 2 { return 0.0; }
    let mean = values.iter().sum::<f32>() / values.len() as f32;
    values.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / (values.len() - 1) as f32
}

use uuid::Uuid;

pub const DIM: usize = 16;

/// Exponential decay per session: effective window ≈ 1/(1-DECAY) ≈ 33 sessions.
/// Old behaviour gradually loses weight so the model adapts to concept drift.
const DECAY: f32 = 0.97;

/// Feature vector extracted from a session
#[derive(Debug, Clone, Default)]
pub struct SessionVector {
    pub features: [f32; DIM],
}

/// Feature indices
pub mod feat {
    pub const AVG_PAUSE_MS:          usize = 0;
    pub const PAUSE_VARIANCE:        usize = 1;
    pub const CLICK_FREQUENCY:       usize = 2;
    pub const AVG_MOUSE_LINEARITY:   usize = 3;
    pub const LINEARITY_VARIANCE:    usize = 4;
    pub const SCROLL_DEPTH:          usize = 5;
    pub const SESSION_DURATION_S:    usize = 6;
    pub const EVENT_COUNT:           usize = 7;
    pub const UNIQUE_EVENT_TYPES:    usize = 8;
    pub const NOVELTY_RESPONSE:      usize = 9;
    pub const SCAN_ENTROPY:          usize = 10;
    pub const TRAJECTORY_VARIANCE:   usize = 11;
    pub const FITTS_COMPLIANCE:      usize = 12;
    pub const HOVER_LATENCY:         usize = 13;
    pub const MICRO_CORRECTION_RATE: usize = 14;
    pub const VELOCITY_PROFILE:      usize = 15;
}

/// User embedding stored between sessions.
///
/// Uses Welford online statistics with exponential decay so old sessions
/// gradually lose influence (concept drift adaptation).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BehaviorEmbedding {
    pub user_id:       Uuid,
    pub mu:            [f32; DIM],
    pub sigma2:        [f32; DIM],
    pub m2:            [f32; DIM],
    pub session_count: u32,
    /// Effective sample size after decay. Older embeddings without this field
    /// deserialize to 0.0 and are migrated on the first update.
    #[serde(default)]
    pub eff_n:         f32,
    /// Each confirmed-fraud label tightens the adaptive block threshold by 0.04.
    #[serde(default)]
    pub fraud_strikes: u8,
}

impl BehaviorEmbedding {
    pub fn new(user_id: Uuid) -> Self {
        Self {
            user_id,
            mu:            [0.0; DIM],
            sigma2:        [0.0; DIM],
            m2:            [0.0; DIM],
            session_count: 0,
            eff_n:         0.0,
            fraud_strikes: 0,
        }
    }

    /// Update with a completed session's feature vector.
    /// Applies exponential decay so the model forgets old behaviour over time.
    pub fn update(&mut self, v: &SessionVector) {
        // Migrate embeddings saved before eff_n was introduced
        if self.eff_n == 0.0 && self.session_count > 0 {
            self.eff_n = self.session_count as f32;
        }

        self.session_count += 1;
        self.eff_n = self.eff_n * DECAY + 1.0;
        let n = self.eff_n;

        for i in 0..DIM {
            // Decay M2 so older variance contribution shrinks
            self.m2[i] *= DECAY;
            let delta  = v.features[i] - self.mu[i];
            self.mu[i] += delta / n;
            let delta2 = v.features[i] - self.mu[i];
            self.m2[i] += delta * delta2;
            if n > 1.5 {
                self.sigma2[i] = self.m2[i] / (n - 1.0);
            }
        }
    }

    /// Anomaly score: 0.0=normal, 1.0=very anomalous.
    pub fn anomaly_score(&self, v: &SessionVector) -> f32 {
        if self.session_count < 5 { return 0.2; }

        let mut total  = 0.0f32;
        let mut active = 0u32;

        for i in 0..DIM {
            let std = self.sigma2[i].sqrt();
            if std < 0.001 { continue; }
            let z = (v.features[i] - self.mu[i]).abs() / std;
            total  += (z / 3.0_f32).min(1.0);
            active += 1;
        }

        if active == 0 { return 0.2; }
        (total / active as f32).clamp(0.0, 1.0)
    }

    /// Returns block threshold in [0.35, 0.75].
    /// Predictable users get a tighter threshold; confirmed-fraud strikes lower it further.
    pub fn adaptive_threshold(&self) -> f32 {
        let avg_variance = self.sigma2.iter().sum::<f32>() / DIM as f32;
        let base = (0.65 - avg_variance.sqrt() * 0.15).clamp(0.40, 0.75);
        (base - self.fraud_strikes as f32 * 0.04).clamp(0.35, base)
    }

    /// Record a confirmed-fraud signal. Caps at 10 strikes to prevent runaway tightening.
    pub fn record_fraud_strike(&mut self) {
        if self.fraud_strikes < 10 {
            self.fraud_strikes += 1;
        }
    }
}

/// Tracks "variance of variance" — the meta-stability of behavioral noise.
///
/// Human noise is tied to life state: attention, fatigue, mood.
/// It's neither perfectly stable (scripted bot) nor completely random (noise-injected bot).
/// Each person has a characteristic amplitude of instability.
///
/// Within a session: collects l1 deviation scores, computes deltas between adjacent
/// deviations, then returns variance(deltas) = how erratically the deviation itself jumps.
///
/// Between sessions: BehaviorEmbedding tracks Welford stats of per-session meta-variance,
/// so we can detect when a session's meta-variance is anomalous for this user.
#[derive(Debug, Clone, Default)]
pub struct MetaVarianceTracker {
    last_deviation: Option<f32>,
    deltas:         Vec<f32>,
}

impl MetaVarianceTracker {
    /// Push a new deviation score (e.g. l1_score for the current event).
    pub fn push(&mut self, deviation: f32) {
        if let Some(last) = self.last_deviation {
            self.deltas.push((deviation - last).abs());
        }
        self.last_deviation = Some(deviation);
    }

    /// Variance of the deviation-deltas.
    /// Returns None until we have at least 4 deltas (5 events).
    pub fn meta_variance(&self) -> Option<f32> {
        if self.deltas.len() < 4 { return None; }
        Some(variance(&self.deltas))
    }

    pub fn reset(&mut self) {
        self.last_deviation = None;
        self.deltas.clear();
    }
}

fn variance(v: &[f32]) -> f32 {
    if v.len() < 2 { return 0.0; }
    let mean = v.iter().sum::<f32>() / v.len() as f32;
    v.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / (v.len() - 1) as f32
}

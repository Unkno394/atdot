/// Continuity signature — detects session handoff from human to bot.
///
/// Humans show natural variance in micro-corrections and hover latency.
/// When a bot takes over, variance collapses toward zero — drift_direction goes negative.
#[derive(Debug, Clone, Default)]
pub struct ContinuitySignature {
    correction_history: Vec<f32>,
    hover_history:      Vec<f32>,
    pub drift_velocity:  f32,
    pub drift_direction: f32,  // +1.0 = toward human, -1.0 = toward machine
}

impl ContinuitySignature {
    const WINDOW: usize = 12;

    pub fn update(&mut self, micro_correction_rate: f32, hover_latency: f32) {
        self.correction_history.push(micro_correction_rate);
        self.hover_history.push(hover_latency);
        if self.correction_history.len() > Self::WINDOW {
            self.correction_history.remove(0);
            self.hover_history.remove(0);
        }
        self.recompute();
    }

    fn recompute(&mut self) {
        let n = self.correction_history.len();
        if n < 6 {
            self.drift_velocity  = 0.0;
            self.drift_direction = 0.0;
            return;
        }

        let mid = n / 2;
        let var_early = variance_f32(&self.correction_history[..mid])
            + variance_f32(&self.hover_history[..mid]);
        let var_late  = variance_f32(&self.correction_history[mid..])
            + variance_f32(&self.hover_history[mid..]);

        self.drift_velocity  = (var_late - var_early).abs();
        // Negative direction = variance dropped = machine-like
        self.drift_direction = if var_late < var_early { -1.0 } else { 1.0 };
    }

    /// Score: 0.0 = no anomaly, 1.0 = sudden collapse to machine behaviour.
    pub fn anomaly_score(&self) -> f32 {
        if self.drift_velocity < 0.005 || self.drift_direction >= 0.0 {
            return 0.0;
        }
        (self.drift_velocity * 5.0).min(1.0)
    }
}

fn variance_f32(v: &[f32]) -> f32 {
    if v.len() < 2 { return 0.0; }
    let mean = v.iter().sum::<f32>() / v.len() as f32;
    v.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / (v.len() - 1) as f32
}

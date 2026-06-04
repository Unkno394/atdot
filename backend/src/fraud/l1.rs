use dashmap::DashMap;
use parking_lot::RwLock;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::fraud::{
    continuity::ContinuitySignature,
    embedding::BehaviorEmbedding,
    features::{EventFeatures, SessionAccumulator, humanity_score},
    graph::BehaviorGraph,
    meta_variance::MetaVarianceTracker,
};

pub const FLUSH_INTERVAL: Duration = Duration::from_secs(30);
const HOT_TTL:            Duration = Duration::from_secs(5 * 60);
const RECENT_WINDOW:      usize    = 5;

struct HotEntry {
    graph:            Arc<RwLock<BehaviorGraph>>,
    embedding:        Arc<RwLock<BehaviorEmbedding>>,
    session_events:   Vec<(EventFeatures, i64)>,  // Stage 1: raw — все события
    session_start_ms: i64,
    last_accessed:    Instant,
    dirty:            Arc<AtomicBool>,
    recent_events:    VecDeque<String>,
    session_sequence: Vec<String>,
    continuity:       ContinuitySignature,
    meta_var:         MetaVarianceTracker,
}

struct EntrySnapshot {
    graph:            Arc<RwLock<BehaviorGraph>>,
    dirty:            Arc<AtomicBool>,
    recent_snap:      Vec<String>,
    session_seq:      Vec<String>,
    embedding_score:  f32,
    continuity_score: f32,
    completed_seq:    Option<Vec<String>>,  // Some(_) только на page_hide
    humanity_score:   f32,
    meta_var_anomaly: f32,
}

pub struct L1Result {
    pub l1_score:         f32,
    pub path_score:       f32,
    pub familiarity:      f32,
    pub session_seq:      Vec<String>,
    pub reasons:          Vec<String>,
    pub embedding_score:  f32,
    pub continuity_score: f32,
    pub completed_seq:    Option<Vec<String>>,
    pub humanity_score:   f32,
    pub meta_var_anomaly: f32,
}

pub struct L1Store {
    graphs: DashMap<Uuid, HotEntry>,
    sled:   Arc<sled::Db>,
}

impl L1Store {
    pub fn open(path: &str) -> anyhow::Result<Self> {
        let sled = Arc::new(sled::open(path)?);
        Ok(Self { graphs: DashMap::new(), sled })
    }

    fn emb_key(user_id: Uuid) -> Vec<u8> {
        let mut key = b"emb_".to_vec();
        key.extend_from_slice(user_id.as_bytes());
        key
    }

    fn load_embedding(&self, user_id: Uuid) -> Option<BehaviorEmbedding> {
        let bytes = self.sled.get(Self::emb_key(user_id)).ok()??;
        bincode::deserialize(&bytes).ok()
    }

    fn save_embedding(&self, emb: &BehaviorEmbedding) -> anyhow::Result<()> {
        let value = bincode::serialize(emb)?;
        self.sled.insert(Self::emb_key(emb.user_id), value)?;
        Ok(())
    }

    pub fn open_tree(&self, name: &str) -> anyhow::Result<sled::Tree> {
        Ok(self.sled.open_tree(name)?)
    }

    fn load_graph(&self, user_id: Uuid) -> Option<BehaviorGraph> {
        let bytes = self.sled.get(user_id.as_bytes()).ok()??;
        bincode::deserialize(&bytes).ok()
    }

    fn save_graph(&self, graph: &BehaviorGraph) -> anyhow::Result<()> {
        let value = bincode::serialize(graph)?;
        self.sled.insert(graph.user_id.as_bytes(), value)?;
        Ok(())
    }

    fn approx_count(&self) -> u64 { self.sled.len() as u64 }

    /// Returns the adaptive block threshold for a user.
    /// Falls back to 0.75 if the user has fewer than 5 sessions.
    pub fn get_adaptive_threshold(&self, user_id: Uuid) -> f32 {
        self.graphs
            .get(&user_id)
            .map(|e| e.embedding.read().adaptive_threshold())
            .unwrap_or(0.75)
    }

    fn access_entry(&self, user_id: Uuid, ef: &EventFeatures) -> EntrySnapshot {
        let now_ms = chrono::Utc::now().timestamp_millis();

        let mut entry = self.graphs.entry(user_id).or_insert_with(|| {
            let graph = self.load_graph(user_id)
                .unwrap_or_else(|| BehaviorGraph::new(user_id));
            let embedding = self.load_embedding(user_id)
                .unwrap_or_else(|| BehaviorEmbedding::new(user_id));
            HotEntry {
                graph:            Arc::new(RwLock::new(graph)),
                embedding:        Arc::new(RwLock::new(embedding)),
                session_events:   Vec::new(),
                session_start_ms: now_ms,
                last_accessed:    Instant::now(),
                dirty:            Arc::new(AtomicBool::new(false)),
                recent_events:    VecDeque::with_capacity(RECENT_WINDOW + 1),
                session_sequence: Vec::new(),
                continuity:       ContinuitySignature::default(),
                meta_var:         MetaVarianceTracker::default(),
            }
        });

        entry.last_accessed = Instant::now();
        entry.recent_events.push_back(ef.event_type.clone());
        if entry.recent_events.len() > RECENT_WINDOW { entry.recent_events.pop_front(); }
        entry.session_sequence.push(ef.event_type.clone());
        entry.session_events.push((ef.clone(), now_ms));

        // Continuity signature update — only from click events
        if ef.event_type == "click" {
            let mc_rate = ef.micro_corrections
                .map(|mc| if ef.trajectory_len > 0 { mc as f32 / ef.trajectory_len as f32 } else { 0.0 })
                .unwrap_or(0.0);
            let hover = ef.hover_duration_ms
                .map(|h| (h / 3000.0).min(1.0) as f32)
                .unwrap_or(0.0);
            entry.continuity.update(mc_rate, hover);
        }
        let continuity_score = entry.continuity.anomaly_score();

        // Embedding anomaly score from Stage 1 (raw) events — used for real-time scoring
        let embedding_score = if entry.session_events.len() >= 3 {
            let mut acc = SessionAccumulator::new(entry.session_start_ms);
            for (e, ts) in entry.session_events.iter() {
                acc.add(e.clone(), *ts);
            }
            let sv = acc.to_vector();
            entry.embedding.read().anomaly_score(&sv)
        } else {
            0.2
        };

        // Meta-variance: compare current session's deviation-stability against user's history
        let meta_var_anomaly = entry.meta_var.meta_variance()
            .map(|mv| entry.embedding.read().meta_variance_anomaly(mv))
            .unwrap_or(0.0);

        // Session finalization on page_hide
        let (completed_seq, humanity_score_val) = if ef.event_type == "page_hide"
            && entry.session_events.len() >= 5
        {
            // Capture full sequence BEFORE clearing
            let seq = entry.session_sequence.clone();

            // Humanity score from raw events
            let evs: Vec<EventFeatures> = entry.session_events.iter().map(|(e, _)| e.clone()).collect();
            let h = humanity_score(&evs);

            // Persist meta-variance of this session into the embedding
            if let Some(mv) = entry.meta_var.meta_variance() {
                entry.embedding.write().update_meta_variance(mv);
            }
            entry.meta_var.reset();

            // Stage 2: filter events before training the embedding
            // Remove: same-type duplicates <80ms, accidental scroll/page_view <1s
            let filtered = filter_stage2(&entry.session_events);
            if filtered.len() >= 3 {
                let mut acc = SessionAccumulator::new(entry.session_start_ms);
                for (e, ts) in &filtered {
                    acc.add(e.clone(), *ts);
                }
                let sv = acc.to_vector();
                entry.embedding.write().update(&sv);
            }

            entry.session_events.clear();
            entry.session_sequence.clear();
            entry.session_start_ms = now_ms;
            entry.dirty.store(true, Ordering::Relaxed);
            (Some(seq), h)
        } else {
            (None, 0.0)
        };

        EntrySnapshot {
            graph:            entry.graph.clone(),
            dirty:            entry.dirty.clone(),
            recent_snap:      entry.recent_events.iter().cloned().collect(),
            session_seq:      entry.session_sequence.clone(),
            embedding_score,
            continuity_score,
            completed_seq,
            humanity_score:   humanity_score_val,
            meta_var_anomaly,
        }
    }

    pub fn score(
        &self,
        user_id:    Uuid,
        prev_event: Option<&str>,
        ef:         &EventFeatures,
    ) -> L1Result {
        let mut reasons = Vec::new();
        let snap = self.access_entry(user_id, ef);

        let l1_score = if let Some(prev) = prev_event {
            let s = snap.graph.read().score_transition(prev, ef);
            snap.graph.write().observe(prev, ef);
            snap.dirty.store(true, Ordering::Relaxed);
            if s > 0.6 {
                reasons.push(format!(
                    "L1 unusual transition ({} → {}): {:.2}", prev, ef.event_type, s
                ));
            }
            s
        } else {
            0.0
        };

        // Feed l1_score into the meta-variance tracker for next event's snapshot
        if let Some(mut entry) = self.graphs.get_mut(&user_id) {
            entry.meta_var.push(l1_score);
        }

        let recent_refs: Vec<&str> = snap.recent_snap.iter().map(|s| s.as_str()).collect();
        let path_score  = snap.graph.read().path_optimality_score(&recent_refs);
        let familiarity = snap.graph.read().familiarity(&ef.event_type);

        if snap.embedding_score > 0.6 {
            reasons.push(format!("embedding drift: {:.2}", snap.embedding_score));
        }
        if path_score > 0.75 {
            reasons.push(format!("optimal path traversal (bot-like): {:.2}", path_score));
        }
        if snap.continuity_score > 0.4 {
            reasons.push(format!("continuity break (possible session handoff): {:.2}", snap.continuity_score));
        }
        if snap.meta_var_anomaly > 0.5 {
            reasons.push(format!("meta-variance anomaly (noise pattern mismatch): {:.2}", snap.meta_var_anomaly));
        }

        L1Result {
            l1_score,
            path_score,
            familiarity,
            embedding_score:  snap.embedding_score,
            continuity_score: snap.continuity_score,
            session_seq:      snap.session_seq,
            completed_seq:    snap.completed_seq,
            humanity_score:   snap.humanity_score,
            meta_var_anomaly: snap.meta_var_anomaly,
            reasons,
        }
    }

    /// Apply a confirmed-fraud (or false-positive) signal to a user's embedding.
    /// Tightens the adaptive threshold for confirmed fraud.
    /// Works for hot users immediately; cold users are updated via sled.
    pub fn apply_fraud_signal(&self, user_id: Uuid, confirmed_fraud: bool) {
        if !confirmed_fraud { return; }

        if let Some(entry) = self.graphs.get(&user_id) {
            entry.embedding.write().record_fraud_strike();
            entry.dirty.store(true, Ordering::Relaxed);
            return;
        }

        // User not in hot cache — load, update, save directly in sled
        if let Some(mut emb) = self.load_embedding(user_id) {
            emb.record_fraud_strike();
            let _ = self.save_embedding(&emb);
        }
    }

    pub fn flush_cold_graphs(&self) {
        let now = Instant::now();
        let mut to_evict: Vec<Uuid> = Vec::new();

        for entry in self.graphs.iter() {
            if now.duration_since(entry.last_accessed) < HOT_TTL { continue; }
            if entry.dirty.load(Ordering::Relaxed) {
                let mut graph = entry.graph.write();
                graph.prune();
                if let Err(e) = self.save_graph(&*graph) {
                    tracing::warn!("graph flush failed for {}: {}", entry.key(), e);
                    continue;
                }
                let emb = entry.embedding.read();
                if let Err(e) = self.save_embedding(&*emb) {
                    tracing::warn!("embedding flush failed for {}: {}", entry.key(), e);
                }
                entry.dirty.store(false, Ordering::Relaxed);
            }
            to_evict.push(*entry.key());
        }

        for uid in &to_evict { self.graphs.remove(uid); }

        if !to_evict.is_empty() {
            tracing::debug!(
                "flushed {} cold graphs; {} still hot, ~{} cold on disk",
                to_evict.len(), self.graphs.len(), self.approx_count(),
            );
        }
    }
}

/// Stage 2 filter: remove noise before training the embedding.
/// Keeps intentional events; drops duplicate bursts and accidental micro-transitions.
fn filter_stage2(events: &[(EventFeatures, i64)]) -> Vec<(EventFeatures, i64)> {
    let mut out = Vec::with_capacity(events.len());
    for i in 0..events.len() {
        let (ef, ts) = &events[i];
        if i > 0 {
            let (prev_ef, prev_ts) = &events[i - 1];
            // Drop same-type duplicates within 80 ms
            if ef.event_type == prev_ef.event_type && (ts - prev_ts) < 80 {
                continue;
            }
            // Drop accidental scroll/page_view within 1 s
            if matches!(ef.event_type.as_str(), "scroll" | "page_view")
                && (ts - prev_ts) < 1000
            {
                continue;
            }
        }
        out.push((ef.clone(), *ts));
    }
    out
}

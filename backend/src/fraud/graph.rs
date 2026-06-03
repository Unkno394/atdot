use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;
use uuid::Uuid;

const PRUNE_AFTER_N:   u64  = 500;
const PRUNE_MIN_COUNT: u64  = 2;
const PRUNE_MIN_PROB:  f32  = 0.01;

/// Online Welford — running mean and variance without storing all values
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Welford {
    pub count: u64,
    pub mean:  f64,
    pub m2:    f64,
}

impl Welford {
    pub fn update(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        self.m2 += delta * (value - self.mean);
    }

    pub fn variance(&self) -> f64 {
        if self.count < 2 { 0.0 } else { self.m2 / (self.count - 1) as f64 }
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }
}

/// Directed edge: transition A → B
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct EdgeData {
    pub count:       u64,
    pub pause_stats: Welford,
}

/// Hardcoded prior — используется пока L1 пустой или L3 не накопился.
/// Значения основаны на типичном e-commerce поведении.
/// Когда L3 накопит данные — заменяется реальными числами через update_from_l3().
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PriorGraph {
    /// transitions[from][to] = вероятность перехода
    pub transitions: HashMap<String, HashMap<String, f32>>,
    /// pause_means[from][to] = средняя пауза в ms
    pub pause_means: HashMap<String, HashMap<String, f64>>,
    /// pause_stds[from][to] = стандартное отклонение паузы в ms
    pub pause_stds:  HashMap<String, HashMap<String, f64>>,
}

impl PriorGraph {
    pub fn ecommerce_default() -> Self {
        // макрос чтобы не писать .into() везде
        macro_rules! map {
            ($($k:expr => $v:expr),* $(,)?) => {{
                let mut m = HashMap::new();
                $(m.insert($k.to_string(), $v);)*
                m
            }};
        }

        let transitions = map! {
            "page_view" => map! {
                "click"     => 0.40f32,
                "scroll"    => 0.25,
                "page_view" => 0.20,
                "page_hide" => 0.10,
                "search"    => 0.05,
            },
            "click" => map! {
                "page_view"  => 0.40f32,
                "click"      => 0.30,
                "scroll"     => 0.15,
                "purchase"   => 0.08,
                "page_hide"  => 0.07,
            },
            "scroll" => map! {
                "click"     => 0.45f32,
                "scroll"    => 0.25,
                "page_view" => 0.20,
                "page_hide" => 0.10,
            },
            "search" => map! {
                "page_view" => 0.55f32,
                "click"     => 0.30,
                "page_hide" => 0.15,
            },
            "purchase" => map! {
                "page_view" => 0.65f32,
                "page_hide" => 0.35,
            },
            "page_hide" => map! {
                "page_view" => 1.0f32,
            },
        };

        // средние паузы (ms) — сколько думает человек между событиями
        let pause_means = map! {
            "page_view" => map! {
                "click"     => 2800.0f64,  // читает страницу ~2.8 сек
                "scroll"    => 1500.0,
                "page_view" => 3500.0,
                "page_hide" => 5000.0,
                "search"    => 4000.0,
            },
            "click" => map! {
                "page_view"  => 1200.0f64,
                "click"      =>  900.0,
                "scroll"     =>  600.0,
                "purchase"   => 9000.0,   // долго думает перед покупкой
                "page_hide"  => 2000.0,
            },
            "scroll" => map! {
                "click"     => 1800.0f64,
                "scroll"    =>  400.0,
                "page_view" => 2500.0,
                "page_hide" => 3000.0,
            },
            "search" => map! {
                "page_view" => 1500.0f64,
                "click"     =>  800.0,
                "page_hide" => 4000.0,
            },
            "purchase" => map! {
                "page_view" => 3000.0f64,
                "page_hide" => 2000.0,
            },
        };

        // стандартные отклонения — человек непоследователен
        let pause_stds = map! {
            "page_view" => map! {
                "click"     => 2200.0f64,
                "scroll"    =>  900.0,
                "page_view" => 2800.0,
                "page_hide" => 4000.0,
                "search"    => 3000.0,
            },
            "click" => map! {
                "page_view"  =>  900.0f64,
                "click"      =>  700.0,
                "scroll"     =>  400.0,
                "purchase"   => 7000.0,
                "page_hide"  => 1500.0,
            },
            "scroll" => map! {
                "click"     => 1400.0f64,
                "scroll"    =>  300.0,
                "page_view" => 2000.0,
                "page_hide" => 2500.0,
            },
            "search" => map! {
                "page_view" => 1200.0f64,
                "click"     =>  600.0,
                "page_hide" => 3000.0,
            },
            "purchase" => map! {
                "page_view" => 2500.0f64,
                "page_hide" => 1500.0,
            },
        };

        Self { transitions, pause_means, pause_stds }
    }

    /// Score перехода на основе prior: 0.0 = норма, 1.0 = аномалия
    pub fn score(&self, from: &str, to: &str, pause_ms: f64) -> f32 {
        let prob = self.transitions
            .get(from)
            .and_then(|m| m.get(to))
            .copied()
            .unwrap_or(0.04); // неизвестный переход — редкий но возможный

        let rarity = 1.0 - prob;

        let pause_anomaly = self.pause_means
            .get(from).and_then(|m| m.get(to))
            .zip(self.pause_stds.get(from).and_then(|m| m.get(to)))
            .map(|(mean, std)| {
                if *std < 1.0 { return 0.0f32; }
                let z = ((pause_ms - mean) / std).abs();
                (z / 3.0).min(1.0) as f32
            })
            .unwrap_or(0.3); // нет данных о паузе — умеренная неуверенность

        (rarity * 0.6 + pause_anomaly * 0.4).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BehaviorGraph {
    pub user_id:           Uuid,
    pub edges:             HashMap<String, HashMap<String, EdgeData>>,
    pub node_out_counts:   HashMap<String, u64>,
    pub total_transitions: u64,
    pub last_event:        Option<String>,
    pub last_event_ts_ms:  Option<i64>,
    /// Prior используется пока личных данных мало.
    /// None = без prior (тесты, особые случаи).
    pub prior:             Option<PriorGraph>,
}

impl BehaviorGraph {
    /// Новый граф без prior — личные данные с нуля.
    pub fn new(user_id: Uuid) -> Self {
        Self {
            user_id,
            edges: HashMap::new(),
            node_out_counts: HashMap::new(),
            total_transitions: 0,
            last_event: None,
            last_event_ts_ms: None,
            prior: None,
        }
    }

    /// Новый граф с хардкодным prior.
    /// Используй это для всех новых пользователей пока L3 не накопился.
    pub fn new_with_default_prior(user_id: Uuid) -> Self {
        Self {
            prior: Some(PriorGraph::ecommerce_default()),
            ..Self::new(user_id)
        }
    }

    pub fn observe(&mut self, from: &str, to: &str, pause_ms: f64) {
        self.edges
            .entry(from.to_string())
            .or_default()
            .entry(to.to_string())
            .or_default()
            .count += 1;

        self.edges.get_mut(from).unwrap()
            .get_mut(to).unwrap()
            .pause_stats.update(pause_ms);

        *self.node_out_counts.entry(from.to_string()).or_insert(0) += 1;
        self.total_transitions += 1;

        if self.total_transitions % PRUNE_AFTER_N == 0 {
            self.prune();
        }
    }

    pub fn prune(&mut self) {
        let out_counts = self.node_out_counts.clone();
        for (from, out_edges) in self.edges.iter_mut() {
            let out_total = *out_counts.get(from).unwrap_or(&1) as f32;
            out_edges.retain(|_, e| {
                e.count >= PRUNE_MIN_COUNT && (e.count as f32 / out_total) >= PRUNE_MIN_PROB
            });
        }
        self.edges.retain(|_, out_edges| !out_edges.is_empty());
    }

    /// P(to | from) — raw transition probability
    pub fn transition_prob(&self, from: &str, to: &str) -> f32 {
        let out = *self.node_out_counts.get(from).unwrap_or(&0);
        if out == 0 { return 0.0; }
        let count = self.edges.get(from)
            .and_then(|m| m.get(to))
            .map(|e| e.count)
            .unwrap_or(0);
        count as f32 / out as f32
    }

    pub fn score_transition(&self, from: &str, to: &str, pause_ms: f64) -> f32 {
        let out = *self.node_out_counts.get(from).unwrap_or(&0);
    
        // насколько доверяем личному графу: 0.0 при out=0, 1.0 при out≥20
        let personal_confidence = (out as f32 / 20.0).min(1.0);
    
        // personal score — только если есть хоть какие-то данные
        let personal_score = if out > 0 {
            let count = self.edges.get(from)
                .and_then(|m| m.get(to))
                .map(|e| e.count)
                .unwrap_or(0);
    
            let k = self.edges.get(from)
                .map(|m| m.len() as f32)
                .unwrap_or(1.0)
                .max(1.0);
            let smoothed_prob = (count as f32 + 1.0) / (out as f32 + k);
            let rarity_score  = (1.0 - smoothed_prob) * personal_confidence;
    
            let pause_anomaly = self.edges
                .get(from)
                .and_then(|m| m.get(to))
                .map(|e| {
                    let std = e.pause_stats.std_dev();
                    if std < 1.0 { return 0.0f32; }
                    let z = ((pause_ms - e.pause_stats.mean) / std).abs();
                    (z / 3.0).min(1.0) as f32
                })
                .unwrap_or(0.0);
    
            (rarity_score * 0.6 + pause_anomaly * 0.4).clamp(0.0, 1.0)
        } else {
            0.5 // нет личных данных — нейтральный score
        };
    
        // prior score
        let prior_score = self.prior
            .as_ref()
            .map(|p| p.score(from, to, pause_ms))
            .unwrap_or(0.5); // нет prior — нейтральный
    
        // смешиваем: чем больше личных данных, тем меньше влияние prior
        let prior_confidence = 1.0 - personal_confidence;
        (personal_confidence * personal_score + prior_confidence * prior_score)
            .clamp(0.0, 1.0)
    }

    pub fn familiarity(&self, event_type: &str) -> f32 {
        if self.total_transitions == 0 { return 0.0; }
        let count = *self.node_out_counts.get(event_type).unwrap_or(&0);
        (count as f32 / self.total_transitions as f32).min(1.0)
    }

    /// Unconstrained shortest path A→B via Dijkstra.
    /// Weight = -log P(to|from). Returns None if unreachable.
    pub fn shortest_path(&self, start: &str, goal: &str) -> Option<f32> {
        if !self.node_out_counts.contains_key(start) { return None; }

        let mut dist: HashMap<&str, f32> = HashMap::new();
        let mut heap: BinaryHeap<PathEntry> = BinaryHeap::new();

        dist.insert(start, 0.0);
        heap.push(PathEntry { cost: 0.0, node: start });

        while let Some(PathEntry { cost, node }) = heap.pop() {
            if node == goal { return Some(cost); }
            if cost > *dist.get(node).unwrap_or(&f32::MAX) { continue; }

            let out_total = *self.node_out_counts.get(node).unwrap_or(&1) as f32;
            if let Some(out_edges) = self.edges.get(node) {
                for (next, edge) in out_edges {
                    let prob = (edge.count as f32 / out_total).max(1e-6);
                    let next_cost = cost + -prob.ln();
                    let prev = *dist.get(next.as_str()).unwrap_or(&f32::MAX);
                    if next_cost < prev {
                        dist.insert(next.as_str(), next_cost);
                        heap.push(PathEntry { cost: next_cost, node: next.as_str() });
                    }
                }
            }
        }
        None
    }

    /// Depth-constrained shortest path: find cheapest path from start to goal
    /// using *exactly* `steps` hops. Returns None if no such path exists.
    ///
    /// This is the correct basis for path_optimality_score: we compare an
    /// actual k-hop path against the optimal k-hop path — apples to apples.
    pub fn shortest_path_of_length(&self, start: &str, goal: &str, steps: usize) -> Option<f32> {
        if steps == 0 {
            return if start == goal { Some(0.0) } else { None };
        }
        if !self.node_out_counts.contains_key(start) { return None; }

        // State: (node_name, remaining_steps). Use String to avoid lifetime constraints.
        let mut dist: HashMap<(String, usize), f32> = HashMap::new();
        let mut heap: BinaryHeap<DepthEntry> = BinaryHeap::new();

        let init_key = (start.to_string(), steps);
        dist.insert(init_key, 0.0);
        heap.push(DepthEntry { cost: 0.0, node: start.to_string(), remaining: steps });

        while let Some(DepthEntry { cost, node, remaining }) = heap.pop() {
            if remaining == 0 {
                if node == goal { return Some(cost); }
                continue;
            }

            let state = (node.clone(), remaining);
            if cost > *dist.get(&state).unwrap_or(&f32::MAX) { continue; }

            let out_total = *self.node_out_counts.get(node.as_str()).unwrap_or(&1) as f32;
            if let Some(out_edges) = self.edges.get(node.as_str()) {
                for (next, edge) in out_edges {
                    let prob = (edge.count as f32 / out_total).max(1e-6);
                    let next_cost = cost + -prob.ln();
                    let next_state = (next.clone(), remaining - 1);
                    let prev = *dist.get(&next_state).unwrap_or(&f32::MAX);
                    if next_cost < prev {
                        dist.insert(next_state, next_cost);
                        heap.push(DepthEntry {
                            cost: next_cost,
                            node: next.clone(),
                            remaining: remaining - 1,
                        });
                    }
                }
            }
        }
        None
    }

    /// Score how closely a recent event window follows the graph's optimal paths.
    ///
    /// Compares the actual 2-hop cost A→B→C against the optimal 2-hop cost from
    /// A to C (same hop count, apples-to-apples). Bots traverse graphs optimally
    /// (near-zero deviation); humans wander and take sub-optimal routes.
    ///
    /// Returns 0.0 (human-like detour) to 1.0 (bot-like optimality).
    /// Requires ≥3 events and a mature graph (≥20 transitions).
    pub fn path_optimality_score(&self, events: &[&str]) -> f32 {
        if events.len() < 3 || self.total_transitions < 20 {
            return 0.0;
        }

        let mut total_optimality = 0.0f32;
        let mut steps = 0usize;

        for window in events.windows(3) {
            let (a, b, c) = (window[0], window[1], window[2]);

            // Optimal 2-hop cost A→?→C (same depth as actual path)
            if let Some(optimal_2hop) = self.shortest_path_of_length(a, c, 2) {
                if optimal_2hop <= 0.0 { continue; }

                // Actual 2-hop cost A→B→C
                let prob_ab = self.transition_prob(a, b).max(1e-6);
                let prob_bc = self.transition_prob(b, c).max(1e-6);
                let actual_cost = -prob_ab.ln() + -prob_bc.ln();

                // deviation=0 → perfectly optimal (suspicious); deviation>0 → detour (human)
                let deviation = (actual_cost - optimal_2hop) / optimal_2hop;
                let optimality = (1.0 - deviation.min(1.0)).max(0.0);
                total_optimality += optimality;
                steps += 1;
            }
        }

        if steps == 0 { 0.0 } else { (total_optimality / steps as f32).clamp(0.0, 1.0) }
    }

    pub fn approx_size_bytes(&self) -> usize {
        let edge_count: usize = self.edges.values().map(|m| m.len()).sum();
        edge_count * 60 + self.node_out_counts.len() * 30
    }
}

/// Min-heap entry for unconstrained Dijkstra
struct PathEntry<'a> {
    cost: f32,
    node: &'a str,
}

impl PartialEq  for PathEntry<'_> { fn eq(&self, o: &Self) -> bool { self.cost.eq(&o.cost) } }
impl Eq         for PathEntry<'_> {}
impl PartialOrd for PathEntry<'_> {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> { Some(self.cmp(o)) }
}
impl Ord for PathEntry<'_> {
    fn cmp(&self, o: &Self) -> Ordering {
        o.cost.partial_cmp(&self.cost).unwrap_or(Ordering::Equal)
    }
}

/// Min-heap entry for depth-constrained Dijkstra
struct DepthEntry {
    cost:      f32,
    node:      String,
    remaining: usize,
}

impl PartialEq  for DepthEntry { fn eq(&self, o: &Self) -> bool { self.cost.eq(&o.cost) } }
impl Eq         for DepthEntry {}
impl PartialOrd for DepthEntry {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> { Some(self.cmp(o)) }
}
impl Ord for DepthEntry {
    fn cmp(&self, o: &Self) -> Ordering {
        o.cost.partial_cmp(&self.cost).unwrap_or(Ordering::Equal)
    }
}

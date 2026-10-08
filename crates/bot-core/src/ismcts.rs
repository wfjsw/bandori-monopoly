//! Single-observer ISMCTS over determinizations (`docs/BOT.md` §3.4).
//!
//! One iteration = one fresh determinization of the root information set,
//! a UCB tree descent over the searching seat's abstracted decisions, then a
//! heuristic rollout to the horizon and a static evaluation. The tree is
//! shared across iterations through information-set keys (the public view a
//! seat sees, clocks stripped), which is what makes it an *information set*
//! tree rather than a game tree.
//!
//! Anytime: a wall-clock budget and an iteration cap. On timeout the best
//! root action so far is returned; with zero completed iterations the
//! heuristic action is the fallback (`docs/BOT.md` §1).
//!
//! Three upgrades over the B4 core (BOT-RESEARCH #2/#3/#5), each behind a flag
//! so the old behaviour stays available for A/B:
//!
//! * **Root-parallel search** ([`Ismcts::search_root_parallel`]): N
//!   independent searches, one per thread, each with its own determinization
//!   stream (seed derived from the request seed + thread index); root visit
//!   counts and value sums are merged to pick the action (Sephton et al.
//!   WCCI 2014 -- root parallel is the efficient choice for ISMCTS).
//! * **Implicit-minimax eval backup + progressive bias** (Lanctot et al.
//!   CIG 2014; Chaslot et al. 2008): the rollout's simulation outcome and the
//!   heuristic evaluation are kept as two separate statistics per edge; the
//!   heuristic one is backed up max-style along the searching seat's decision
//!   path (every tree node is that seat's = a max node) and mixed into the
//!   selection score by [`SearchConfig::eval_weight`]. The heuristic's
//!   per-action preference ([`Simulator::action_priors`], fed by `aiAnswer` /
//!   `ai_picked` / `wants_buy` / `estCost`) adds a decaying
//!   `bias_weight * prior / sqrt(1 + visits)` bonus.
//! * **Tree reuse**: [`Ismcts::retain_after`] keeps the subtree under the
//!   action actually played (nodes are already keyed by the public view's
//!   information-set hash), so consecutive decisions of one seat start warm.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crate::action::Action;
use crate::determinize::DeterminizerRng;
use crate::sim::{Advance, Horizon, Simulator};

/// Search budget for one decision.
#[derive(Debug, Clone, Copy)]
pub struct SearchConfig {
    /// Wall-clock per decision.
    pub budget: Duration,
    /// Iteration cap (independent of the wall clock).
    pub max_iterations: u64,
    /// Rollout horizon in rounds (1–2 is the affordable default).
    pub horizon_rounds: u32,
    /// UCB exploration constant.
    pub ucb_c: f64,
    /// Cap on tree depth inside one iteration (root-player decisions).
    pub max_depth: usize,
    /// Seed for the search RNG (per decision; never the match RNG).
    pub seed: u64,
    /// `α` -- how much the heuristic-evaluation statistic is mixed into the
    /// selection score next to the Monte-Carlo mean (Lanctot et al. CIG 2014).
    /// `0.0` = pure Monte-Carlo, the pre-#3 behaviour.
    pub eval_weight: f64,
    /// Back the heuristic evaluation up implicit-minimax style (max along the
    /// searching seat's decision path) instead of as a plain mean.
    pub implicit_minimax: bool,
    /// `c_h` -- progressive-bias weight (Chaslot et al. 2008). `0.0` = off,
    /// the pre-#3 behaviour (including random untried expansion).
    pub bias_weight: f64,
    /// Record a fork / descent / rollout / eval / key timing breakdown in
    /// [`SearchOutcome::timings`]. Off by default.
    pub profile: bool,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            budget: Duration::from_millis(200),
            max_iterations: 100_000,
            horizon_rounds: 2,
            ucb_c: 1.41,
            max_depth: 8,
            seed: 0,
            eval_weight: 0.3,
            implicit_minimax: true,
            bias_weight: 0.4,
            profile: false,
        }
    }
}

impl SearchConfig {
    /// The pre-#3 selection behaviour: no heuristic mix, no progressive bias.
    pub fn legacy() -> Self {
        Self {
            eval_weight: 0.0,
            implicit_minimax: false,
            bias_weight: 0.0,
            ..Self::default()
        }
    }
}

/// Per-action statistics at one information set.
#[derive(Debug, Clone, Default)]
struct Edge {
    /// Simulation outcomes (terminal win/score, or the cutoff value).
    visits: u64,
    value_sum: f64,
    /// Heuristic evaluations -- a *separate* statistic (Lanctot et al.).
    eval_visits: u64,
    eval_sum: f64,
    /// Implicit-minimax backup of the heuristic value: max over the subtree
    /// (every node on the path is the searching seat's = a max node).
    eval_max: f64,
}

impl Edge {
    fn mean(&self) -> f64 {
        if self.visits == 0 {
            0.0
        } else {
            self.value_sum / self.visits as f64
        }
    }

    fn eval_mean(&self) -> f64 {
        if self.eval_visits == 0 {
            0.0
        } else {
            self.eval_sum / self.eval_visits as f64
        }
    }

    /// The heuristic term that mixes into selection: minimax-style max when
    /// enabled, plain mean otherwise.
    fn eval_term(&self, implicit_minimax: bool) -> f64 {
        if implicit_minimax {
            if self.eval_visits == 0 {
                0.0
            } else {
                self.eval_max
            }
        } else {
            self.eval_mean()
        }
    }
}

/// One information-set node: the abstracted actions branched on there.
#[derive(Debug, Default, Clone)]
struct Node {
    edges: HashMap<Action, Edge>,
    /// Last-seen heuristic priors (progressive bias), parallel to the actions.
    priors: HashMap<Action, f64>,
}

impl Node {
    fn has_untried(&self, actions: &[Action]) -> bool {
        actions.iter().any(|a| !self.edges.contains_key(a))
    }
}

/// The ISMCTS tree. Kept across consecutive decisions of one seat via
/// [`Self::retain_after`]; keyed by the public view's information-set hash.
#[derive(Debug, Default, Clone)]
pub struct Ismcts {
    tree: HashMap<u64, Node>,
    /// Node keys visited under each root action of the last search -- the
    /// material [`Self::retain_after`] keeps.
    subtree: HashMap<Action, HashSet<u64>>,
}

/// Root action statistics after the merge (root-parallel: sums across the
/// independent searches).
#[derive(Debug, Clone)]
pub struct ActionStats {
    pub action: Action,
    pub visits: u64,
    pub value_sum: f64,
    pub eval_max: f64,
    pub eval_sum: f64,
    pub eval_visits: u64,
}

impl ActionStats {
    pub fn mean(&self) -> f64 {
        if self.visits == 0 {
            0.0
        } else {
            self.value_sum / self.visits as f64
        }
    }

    pub fn eval_mean(&self) -> f64 {
        if self.eval_visits == 0 {
            0.0
        } else {
            self.eval_sum / self.eval_visits as f64
        }
    }
}

/// Per-phase wall time inside the search loop (profiling the B4/B5 iteration
/// cost: fork materialisation vs tree vs rollout).
#[derive(Debug, Clone, Copy, Default)]
pub struct Timings {
    pub fork: Duration,
    pub descent: Duration,
    pub rollout: Duration,
    pub evaluate: Duration,
    pub key: Duration,
}

/// What one decision returned.
#[derive(Debug, Clone)]
pub struct SearchOutcome {
    pub action: Action,
    pub iterations: u64,
    pub elapsed: Duration,
    /// Root action statistics `(action, mean, visits)`, best first. Use
    /// [`Self::root_stats_full`] for the merged sums.
    pub root_stats: Vec<(Action, f64, u64)>,
    /// True when the heuristic had to answer (no legal abstracted actions,
    /// or the search never completed an iteration).
    pub heuristic: bool,
    /// Full root statistics (visits + value sums + heuristic evals), sorted
    /// like [`Self::root_stats`].
    pub root_stats_full: Vec<ActionStats>,
    /// Nodes carried over from a previous tree (tree reuse), when measured.
    pub reused_nodes: usize,
    /// Timing breakdown, when [`SearchConfig::profile`] is on.
    pub timings: Option<Timings>,
    /// The root information-set key of this decision (for
    /// [`Ismcts::retain_after`] / service-side caching).
    pub root_key: u64,
}

/// Deterministic per-thread search seed: thread 0 keeps `seed` exactly (so a
/// 1-thread root-parallel search is bit-identical to a plain one), the others
/// are split-mixed away from it.
pub fn seed_for_thread(seed: u64, thread: usize) -> u64 {
    if thread == 0 {
        seed
    } else {
        let mix = 0x9E37_79B9_7F4A_7C15u64.wrapping_mul(thread as u64 + 1);
        splitmix64(seed ^ mix)
    }
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Merge per-search root statistics into one list, sorted best-first (visits
/// desc, mean desc, stable action order). Value sums are added, `eval_max`
/// takes the max.
pub fn merge_stats(parts: &[Vec<ActionStats>]) -> Vec<ActionStats> {
    let mut by_action: HashMap<Action, ActionStats> = HashMap::new();
    for part in parts {
        for s in part {
            let e = by_action.entry(s.action.clone()).or_insert_with(|| ActionStats {
                action: s.action.clone(),
                visits: 0,
                value_sum: 0.0,
                eval_max: f64::NEG_INFINITY,
                eval_sum: 0.0,
                eval_visits: 0,
            });
            e.visits += s.visits;
            e.value_sum += s.value_sum;
            e.eval_max = e.eval_max.max(s.eval_max);
            e.eval_sum += s.eval_sum;
            e.eval_visits += s.eval_visits;
        }
    }
    let mut out: Vec<ActionStats> = by_action.into_values().collect();
    for s in &mut out {
        if s.eval_max == f64::NEG_INFINITY {
            s.eval_max = 0.0;
        }
    }
    sort_stats(&mut out);
    out
}

fn sort_stats(stats: &mut [ActionStats]) {
    stats.sort_by(|x, y| {
        y.visits
            .cmp(&x.visits)
            .then_with(|| {
                y.mean()
                    .partial_cmp(&x.mean())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            // Stable total order (`Action: Ord`) so the merged pick is
            // deterministic even when visits and means tie.
            .then_with(|| x.action.cmp(&y.action))
    });
}

impl Ismcts {
    pub fn new() -> Self {
        Self::default()
    }

    /// How many information-set nodes the tree holds (tests / metrics).
    pub fn node_count(&self) -> usize {
        self.tree.len()
    }

    /// How many abstracted actions are branched on at `key` (tests / metrics).
    pub fn edge_count(&self, key: u64) -> usize {
        self.tree.get(&key).map(|n| n.edges.len()).unwrap_or(0)
    }

    /// Drop the tree (a new decision usually wants a fresh one unless the
    /// caller is reusing via [`Self::retain_after`]).
    pub fn clear(&mut self) {
        self.tree.clear();
        self.subtree.clear();
    }

    /// Tree reuse (`docs/BOT-RESEARCH` #5): keep the subtree under the action
    /// actually played at `root_key`, drop the siblings and everything not
    /// reached under it. The root node keeps only the played edge. Call after
    /// a decision when the tree will be kept across consecutive decisions of
    /// the same seat (nodes are keyed by the public view's information-set
    /// hash, so kept subtrees re-attach whenever the same info set recurs).
    pub fn retain_after(&mut self, root_key: u64, action: &Action) {
        let keep: HashSet<u64> = self
            .subtree
            .remove(action)
            .map(|mut s| {
                s.insert(root_key);
                s
            })
            .unwrap_or_else(|| {
                let mut s = HashSet::new();
                s.insert(root_key);
                s
            });
        self.subtree.clear();
        self.tree.retain(|k, _| keep.contains(k));
        if let Some(node) = self.tree.get_mut(&root_key) {
            node.edges.retain(|a, _| a == action);
            node.priors.retain(|a, _| a == action);
        }
    }

    /// Union `other` into `self`, summing edge statistics (root-parallel tree
    /// merge). `eval_max` takes the max. The subtree bookkeeping is unioned
    /// too, so [`Self::retain_after`] works on a merged tree.
    pub fn merge_from(&mut self, other: Ismcts) {
        for (key, node) in other.tree {
            let mine = self.tree.entry(key).or_default();
            for (a, e) in node.edges {
                let m = mine.edges.entry(a).or_default();
                m.visits += e.visits;
                m.value_sum += e.value_sum;
                m.eval_visits += e.eval_visits;
                m.eval_sum += e.eval_sum;
                m.eval_max = m.eval_max.max(e.eval_max);
            }
            for (a, p) in node.priors {
                mine.priors.insert(a, p);
            }
        }
        for (action, keys) in other.subtree {
            self.subtree.entry(action).or_default().extend(keys);
        }
    }

    /// Search the current decision of `seat` in `sim`'s root view.
    ///
    /// Falls back to the root action list's first entry (the heuristic's
    /// first choice in the abstraction) when nothing is searchable.
    pub fn search<S: Simulator>(&mut self, sim: &mut S, seat: usize, cfg: SearchConfig) -> SearchOutcome {
        let started = Instant::now();
        let mut rng = DeterminizerRng::new(cfg.seed);
        let mut times = Timings::default();
        let mark = |on: bool, t0: Instant, acc: &mut Duration| {
            if on {
                *acc += t0.elapsed();
            }
        };

        // Root action list, from a probe determinization (the root view's
        // legal set does not depend on the hidden sample). The wall-clock
        // budget covers the search loop only -- the probe is setup.
        let t0 = Instant::now();
        let mut probe = match sim.determinize(&mut rng) {
            Ok(f) => f,
            Err(_) => {
                return SearchOutcome {
                    action: Action::Bid { amount: -1 },
                    iterations: 0,
                    elapsed: started.elapsed(),
                    root_stats: Vec::new(),
                    heuristic: true,
                    root_stats_full: Vec::new(),
                    reused_nodes: 0,
                    timings: None,
                    root_key: 0,
                };
            }
        };
        mark(cfg.profile, t0, &mut times.fork);
        let root_actions = sim.legal_actions(&probe, seat);
        if root_actions.is_empty() {
            // Heuristic-delegated decision: `advance` will answer it. The
            // caller should not even call `search` here; return a harmless
            // placeholder and flag it.
            let _ = &mut probe;
            return SearchOutcome {
                action: Action::Bid { amount: -1 },
                iterations: 0,
                elapsed: started.elapsed(),
                root_stats: Vec::new(),
                heuristic: true,
                root_stats_full: Vec::new(),
                reused_nodes: 0,
                timings: None,
                root_key: 0,
            };
        }
        let t0 = Instant::now();
        let root_key = sim.decision_key(&probe, seat);
        mark(cfg.profile, t0, &mut times.key);
        let root_priors = sim.action_priors(&probe, seat, &root_actions);
        {
            // Seed the root node's priors so the first selections see the
            // heuristic's preference even before a descent revisits the root.
            let node = self.tree.entry(root_key).or_default();
            for (a, p) in root_actions.iter().zip(&root_priors) {
                node.priors.entry(a.clone()).or_insert(*p);
            }
        }
        drop(probe);
        let reused_nodes = self.tree.len();
        let deadline = Instant::now() + cfg.budget;
        let horizon = Horizon {
            rounds: cfg.horizon_rounds,
        };

        let mut iterations = 0u64;
        // Always complete at least one iteration (the budget covers extra
        // ones), so a tight budget still returns a searched answer.
        loop {
            if iterations >= cfg.max_iterations {
                break;
            }
            if iterations > 0 && Instant::now() >= deadline {
                break;
            }
            let t0 = Instant::now();
            let mut fork = match sim.determinize(&mut rng) {
                Ok(f) => f,
                Err(_) => break,
            };
            mark(cfg.profile, t0, &mut times.fork);
            // Tree descent: the searching seat is never auto-played.
            let mut path: Vec<(u64, Action)> = Vec::new();
            let mut expanded = false;
            let mut depth = 0;
            let t0 = Instant::now();
            loop {
                if depth >= cfg.max_depth {
                    break;
                }
                if iterations > 0 && Instant::now() >= deadline {
                    break;
                }
                match sim.advance(&mut fork, seat, horizon, true) {
                    Advance::Ended | Advance::Horizon => break,
                    Advance::Decision => {
                        let actions = sim.legal_actions(&fork, seat);
                        if actions.is_empty() {
                            break;
                        }
                        let t1 = Instant::now();
                        let key = sim.decision_key(&fork, seat);
                        mark(cfg.profile, t1, &mut times.key);
                        let priors = if cfg.bias_weight > 0.0 {
                            sim.action_priors(&fork, seat, &actions)
                        } else {
                            vec![0.5; actions.len()]
                        };
                        // Remember the priors for later selections at this
                        // information set (progressive bias).
                        {
                            let node = self.tree.entry(key).or_default();
                            for (a, p) in actions.iter().zip(&priors) {
                                node.priors.insert(a.clone(), *p);
                            }
                        }
                        let node = self.tree.get(&key).unwrap();
                        let action = if !expanded && node.has_untried(&actions) {
                            expanded = true;
                            // Expand an untried action. With the bias on, the
                            // heuristic's favourite goes first (Chaslot's
                            // progressive unpruning / move ordering); the old
                            // behaviour picks uniformly at random.
                            let untried: Vec<(usize, &Action)> = actions
                                .iter()
                                .enumerate()
                                .filter(|(_, a)| !node.edges.contains_key(*a))
                                .collect();
                            if untried.is_empty() {
                                break;
                            }
                            let k = if cfg.bias_weight > 0.0 {
                                // Highest prior, rng tie-break among the top.
                                let best = untried
                                    .iter()
                                    .map(|(i, _)| priors[*i])
                                    .fold(f64::NEG_INFINITY, f64::max);
                                let top: Vec<&Action> = untried
                                    .iter()
                                    .filter(|(i, _)| priors[*i] >= best - 1e-9)
                                    .map(|(_, a)| *a)
                                    .collect();
                                let j = rng.below(top.len());
                                top[j].clone()
                            } else {
                                let j = rng.below(untried.len());
                                untried[j].1.clone()
                            };
                            k
                        } else if !node.edges.is_empty() {
                            // UCB over tried actions, plus the progressive
                            // bias and the heuristic-eval mix.
                            let total: u64 =
                                node.edges.values().map(|e| e.visits).sum::<u64>().max(1);
                            let mut best: Option<(Action, f64)> = None;
                            for (i, a) in actions.iter().enumerate() {
                                let Some(e) = node.edges.get(a) else {
                                    continue;
                                };
                                let prior = priors.get(i).copied().unwrap_or(0.5);
                                let score = selection_score(e, total, prior, &cfg);
                                if best.as_ref().is_none_or(|(_, b)| score > *b) {
                                    best = Some((a.clone(), score));
                                }
                            }
                            match best {
                                Some((a, _)) => a,
                                None => break,
                            }
                        } else {
                            break;
                        };
                        if sim.apply(&mut fork, seat, &action).is_err() {
                            break;
                        }
                        path.push((key, action));
                        depth += 1;
                    }
                }
            }
            mark(cfg.profile, t0, &mut times.descent);
            // Rollout: every seat is a bot again.
            let t0 = Instant::now();
            sim.advance(&mut fork, seat, horizon, false);
            mark(cfg.profile, t0, &mut times.rollout);
            let t0 = Instant::now();
            // Two separate statistics (Lanctot et al. CIG 2014): the
            // simulation outcome (terminal win/score, else the cutoff value)
            // and the heuristic evaluation at the cutoff.
            let v = sim.simulation_value(&fork, seat);
            let h = sim.evaluate(&fork, seat);
            mark(cfg.profile, t0, &mut times.evaluate);
            for (key, action) in &path {
                if let Some(node) = self.tree.get_mut(key) {
                    let e = node.edges.entry(action.clone()).or_default();
                    e.visits += 1;
                    e.value_sum += v;
                    e.eval_visits += 1;
                    e.eval_sum += h;
                    // Implicit minimax: every node on the path is the
                    // searching seat's decision (a max node), so the backup
                    // is the max over the leaf values seen under the edge.
                    if h > e.eval_max {
                        e.eval_max = h;
                    }
                }
            }
            // Subtree bookkeeping for [`Self::retain_after`].
            if let Some((_, root_action)) = path.first() {
                let set = self.subtree.entry(root_action.clone()).or_default();
                for (key, _) in &path {
                    set.insert(*key);
                }
            }
            iterations += 1;
        }

        // Pick the most-visited root action (mean as tiebreak, then the
        // heuristic prior, then a stable action order); fall back to the
        // first root action.
        let node = self.tree.get(&root_key);
        let mut full: Vec<ActionStats> = root_actions
            .iter()
            .map(|a| {
                let e = node.and_then(|n| n.edges.get(a));
                let mut s = ActionStats {
                    action: a.clone(),
                    visits: 0,
                    value_sum: 0.0,
                    eval_max: 0.0,
                    eval_sum: 0.0,
                    eval_visits: 0,
                };
                if let Some(e) = e {
                    s.visits = e.visits;
                    s.value_sum = e.value_sum;
                    s.eval_max = e.eval_max;
                    s.eval_sum = e.eval_sum;
                    s.eval_visits = e.eval_visits;
                }
                s
            })
            .collect();
        // Prior-aware tiebreak: when visits and means tie, the heuristic's
        // preferred action wins (the priors were seeded on the root node).
        // Off in legacy mode (`bias_weight == 0`).
        let prior_of = |a: &Action| -> f64 {
            if cfg.bias_weight <= 0.0 {
                return 0.5;
            }
            node.and_then(|n| n.priors.get(a).copied())
                .unwrap_or(0.5)
        };
        full.sort_by(|x, y| {
            y.visits
                .cmp(&x.visits)
                .then_with(|| {
                    y.mean()
                        .partial_cmp(&x.mean())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| {
                    prior_of(&y.action)
                        .partial_cmp(&prior_of(&x.action))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| x.action.cmp(&y.action))
        });
        let root_stats: Vec<(Action, f64, u64)> = full
            .iter()
            .map(|s| (s.action.clone(), s.mean(), s.visits))
            .collect();
        let action = full
            .first()
            .map(|s| s.action.clone())
            .unwrap_or_else(|| root_actions[0].clone());
        SearchOutcome {
            action,
            iterations,
            elapsed: started.elapsed(),
            root_stats,
            heuristic: iterations == 0,
            root_stats_full: full,
            reused_nodes,
            timings: if cfg.profile { Some(times) } else { None },
            root_key,
        }
    }

    /// Root-parallel ISMCTS (`docs/BOT-RESEARCH` #2): `threads` independent
    /// searches of the same decision, each with its own determinization stream
    /// (`seed_for_thread(cfg.seed, i)`) and its own tree; the root statistics
    /// (and the trees) are merged when the budget expires. Near-linear
    /// speedup in iterations at zero extra latency -- every search stops at
    /// the same per-request deadline.
    ///
    /// `factory` is called `threads` times on the calling thread (one
    /// [`Simulator`] per search); only `S: Send` has to cross into the
    /// workers. `threads <= 1` is exactly [`Self::search`] on `factory()`
    /// (so the 1-thread result equals the single-thread one).
    pub fn search_root_parallel<S, F>(
        &mut self,
        mut factory: F,
        seat: usize,
        cfg: SearchConfig,
        threads: usize,
    ) -> SearchOutcome
    where
        S: Simulator + Send,
        F: FnMut() -> S,
    {
        if threads <= 1 {
            let mut sim = factory();
            return self.search(&mut sim, seat, cfg);
        }
        let started = Instant::now();
        let sims: Vec<S> = (0..threads).map(|_| factory()).collect();
        let mut outcomes: Vec<SearchOutcome> = Vec::with_capacity(threads);
        std::thread::scope(|scope| {
            let mut handles = Vec::with_capacity(threads);
            for (t, mut sim) in sims.into_iter().enumerate() {
                handles.push(scope.spawn(move || {
                    // Each thread runs its own tree over its own determinization
                    // stream; the trees are merged afterwards.
                    let mut tree = Ismcts::new();
                    let cfg_t = SearchConfig {
                        seed: seed_for_thread(cfg.seed, t),
                        ..cfg
                    };
                    let out = tree.search(&mut sim, seat, cfg_t);
                    (tree, out)
                }));
            }
            for h in handles {
                if let Ok((tree, out)) = h.join() {
                    self.merge_from(tree);
                    outcomes.push(out);
                }
            }
        });
        // Merge the root statistics (sums of visits / value sums) and pick.
        let parts: Vec<Vec<ActionStats>> = outcomes.iter().map(|o| o.root_stats_full.clone()).collect();
        let mut full = merge_stats(&parts);
        // `merge_stats` drops actions no thread saw; keep the root action
        // order stable by re-sorting only (it already sorts).
        let root_stats: Vec<(Action, f64, u64)> = full
            .iter()
            .map(|s| (s.action.clone(), s.mean(), s.visits))
            .collect();
        let iterations: u64 = outcomes.iter().map(|o| o.iterations).sum();
        let action = full
            .first()
            .map(|s| s.action.clone())
            .unwrap_or(Action::Bid { amount: -1 });
        let heuristic = outcomes.iter().all(|o| o.heuristic);
        let root_key = outcomes.first().map(|o| o.root_key).unwrap_or(0);
        let reused_nodes = outcomes.first().map(|o| o.reused_nodes).unwrap_or(0);
        let timings = if cfg.profile {
            let mut acc = Timings::default();
            for o in &outcomes {
                if let Some(t) = o.timings {
                    acc.fork += t.fork;
                    acc.descent += t.descent;
                    acc.rollout += t.rollout;
                    acc.evaluate += t.evaluate;
                    acc.key += t.key;
                }
            }
            Some(acc)
        } else {
            None
        };
        if full.is_empty() {
            return SearchOutcome {
                action: Action::Bid { amount: -1 },
                iterations,
                elapsed: started.elapsed(),
                root_stats: Vec::new(),
                heuristic: true,
                root_stats_full: Vec::new(),
                reused_nodes,
                timings,
                root_key,
            };
        }
        // Keep `full` sorted (merge_stats already did); silence the unused
        // mut if the sort is the only writer.
        let _ = &mut full;
        SearchOutcome {
            action,
            iterations,
            elapsed: started.elapsed(),
            root_stats,
            heuristic,
            root_stats_full: full,
            reused_nodes,
            timings,
            root_key,
        }
    }
}

/// Selection score: `(1-α)·MC + α·heuristic-eval` + UCB explore + decaying
/// progressive bias `c_h · prior / sqrt(1 + visits)`.
fn selection_score(e: &Edge, total: u64, prior: f64, cfg: &SearchConfig) -> f64 {
    if e.visits == 0 {
        return f64::INFINITY;
    }
    let mc = e.mean();
    let heur = e.eval_term(cfg.implicit_minimax);
    let mix = if cfg.eval_weight > 0.0 && e.eval_visits > 0 {
        (1.0 - cfg.eval_weight) * mc + cfg.eval_weight * heur
    } else {
        mc
    };
    let explore = cfg.ucb_c * ((total as f64).ln() / e.visits as f64).sqrt();
    let bias = if cfg.bias_weight > 0.0 {
        cfg.bias_weight * prior / ((e.visits as f64 + 1.0).sqrt())
    } else {
        0.0
    };
    mix + explore + bias
}

/// Stable order key for action tiebreaks (`Action: Ord` is the same order;
/// kept only as documentation of the intended kind order).
#[allow(dead_code)]
fn ordered(a: &Action) -> u8 {
    match a {
        Action::Play { .. } => 0,
        Action::Buy { .. } => 1,
        Action::Build { .. } => 2,
        Action::Offer { .. } => 3,
        Action::Bid { .. } => 4,
        Action::Counteract { .. } => 5,
        Action::Pick { .. } => 6,
        Action::Mortgage { .. } => 7,
    }
}
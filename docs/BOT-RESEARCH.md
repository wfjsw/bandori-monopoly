# MCTS acceleration — research notes (2026-10-07)

Scope: what the MCTS / ISMCTS / game-AI literature offers for **our** bot
(`docs/BOT.md`: 2–4 player Monopoly-like game, hidden hands/decks/event deck,
~285 wasm (or native) card rules, a [反击] counteraction system, ISMCTS over
per-iteration determinizations, heuristic rollout to a 2–3 round horizon,
net-worth evaluation, 0.2–1 s per decision, 4 server search threads + a planned
browser Web Worker).

**Cost picture (BOT.md §5).** Real card rules: depth-1 rollout ≈ 50 ms,
depth-2 ≈ 131 ms, depth-3 ≈ 188 ms (≈5–20 rollouts/s/core). ~56 000
`can_counteract` eligibility probes per game, card-rule runs, `World` clones
(~6 µs). B5 measures **1.0 iterations at 200 ms / 3.7 at 1 000 ms** — the
target is 40–100+. Two distinct bottlenecks: (a) the **engine shell** — the
[反击] window bookkeeping + per-probe `Run` + `world_copy` (GUARDS.md §1
measures 81 % of a wasmtime game in the shell, guest execution 1 %), and
(b) the **fork materialisation** — B4's save-JSON → `Match::restore` round
trip ≈ 10 ms of the 14 ms/iteration on StubRules.

Every item below maps to (a) or (b), or to spending each simulation better.

---

## 1. Fewer / cheaper simulations

| technique | what the literature says | mapping to our engine | gain | effort / risk |
|---|---|---|---|---|
| **Rollout cutoff + evaluation** | early termination with a heuristic eval is the standard cost/quality knob; randomised cutoffs avoid a fixed-horizon bias (see survey [Browne et al. 2012](https://turing.iimas.unam.mx/~luis/cursos/IA2021-1/lecturas/SurveyMCTS.pdf)) | we already stop at a 2–3 round horizon and score with `eval::relative_worth` (BOT.md §3.4). Drop the default horizon to **1 round + eval**, or evaluate at a random 1–2 round cutoff | rollout 131 ms → ~50 ms (2.5×) | low. Risk: net-worth eval ignores rent / set-completion, so a short horizon biases toward cash. Measure with same-budget win rate |
| **Implicit minimax backups** | [Lanctot, Winands, Pepels & Sturtevant, CIG 2014](https://mlanctot.info/files/papers/cig14-immcts.pdf): keep win rates and heuristic values as *two separate* statistics and back the heuristic up minimax-style; do **not** replace playouts with the eval. Gains in Kalah / Breakthrough / Lines of Action | our tree already stores visit counts + values; add a second `(eval_value, eval_visits)` pair per node, backed up as max for the moving seat / min for the opponent, and mix into the UCB (or use it only for tie-breaks at the root) | stronger play at the *same* rollout cost; partly substitutes for deeper rollouts | medium. Risk: heuristic calibration — a noisy eval in the backup can mislead early; anneal its weight with node visits (see progressive bias) |
| **Progressive bias / unpruning** | [Chaslot, Winands, van den Herik, Uiterwijk & Bouzy 2008](https://project.dke.maastrichtuniversity.nl/games/files/articles/pMCTS.pdf): progressive bias folds expensive heuristic knowledge into UCT as a decaying bonus; progressive unpruning grows the branching factor gradually. Both helped Mango, and together more so on larger boards | `engine/ai.rs` already *is* the expensive heuristic (it runs every rollout). Use its per-action preference (`ai_picked` / `wants_buy` / `wants_build`) as a **prior bonus `c_h / sqrt(n+1)`** in the tree policy, not just in rollouts. Progressive unpruning: expand only the top-k abstracted actions first (buy / build / a couple of card plays), open the rest as visits grow | better use of each iteration; directly attacks the 2–4 action root and the wider card-play roots | low–medium. Risk: double-counting the heuristic (rollout policy *and* bias) — calibrate `c_h` on the B4 harness |
| **n-tuple / small learned value** | n-tuple networks + TD learn strong evaluators from cheap self-play ([Szubert & Jaśkowski, CIG 2014](https://mszubert.github.io/papers/Szubert_2014_CIG.pdf); systematic n-tuples [arXiv:1406.1509](https://arxiv.org/abs/1406.1509)); inference is a few table lookups | we have replay records (`.bdrec`, `docs/REPLAY.md`) and the deckbook's self-play pipeline. Train a small value head on features we already compute (cash, deeds, set-completion, rent exposure, hand size, event-deck top) — linear model, GBDT, or a tiny MLP — and use it at the rollout cutoff | can replace the 2-round rollout with a 0–1 round one; inference µs vs 50–200 ms | medium. Risk: distribution shift (records are standard-bot games). Bootstrap from search (distil ISMCTS root values) once we have 10⁴+ decisions |
| **Tree policy as provider** | (no paper needed — it is the B2 "next increment", BOT.md §3.2) | today descent still uses halt/replay for prompts the tree branches on; making the tree policy the `AnswerProvider` removes a whole class of body re-runs | ~1.5× on rollouts with many prompts; required for deep trees | low–medium; blocked by the known B2 equivalence gap (write journal) |

**Takeaway.** The literature's cheapest wins here are *not* smarter search —
they are (i) not paying for a horizon you cannot afford (cutoff + eval) and
(ii) letting the heuristic you already run steer the tree as well as the
rollout (progressive bias). Both are small diffs on top of `bot-core`.

---

## 2. Better use of each simulation

| technique | what the literature says | mapping to our engine | gain | effort / risk |
|---|---|---|---|---|
| **RAVE / AMAF** | [Gelly & Silver 2011](https://ics.uci.edu/~dechter/courses/ics-295/winter-2018/papers/mcts-gelly-silver.pdf): "all-moves-as-first" statistics share rollouts across sibling moves via a β-weighted blend; huge in Go, smaller in games without a strong action-ordering correlation | AMAF needs "the same action appearing later in the rollout means this move was fine". Our actions are heterogeneous (card plays, buy/build, bid levels) and the rollout policy is greedy, so sibling correlation is weak. A **restricted AMAF over the buy/build/end family only** is the plausible slice | modest (10–30 %) if any | medium complexity for the gain. Prefer MAST / bias first |
| **MAST / move priors** | action-sampling statistics (Tesauro & Galley; Finnsson & Björnsson's learned control knowledge; summarised in [Chaslot et al., AIJ 2015, "Information capture and reuse strategies in MCTS"](https://www.sciencedirect.com/science/article/pii/S0004370214001052)) — a table of action → average outcome, shared across the tree | maintain a per-(decision-kind, action-key) running average from *all* rollouts, use it as the UCB prior and the tie-break. Cheap; naturally amortises across the 1–4 root actions | 10–30 % fewer iterations to the same choice | low. Risk: stale priors across wildly different board states — keep it coarse (by action *kind*, not card id) |
| **Transpositions** | TT-MCTS merges identical subtrees; "[Transpositions and move groups in Monte Carlo tree search](https://scispace.com/pdf/transpositions-and-move-groups-in-monte-carlo-tree-search-1g9tux3xje.pdf)", and the risks of merging when the state is not fully observed | our tree is *already* keyed by the seat's public view (clocks stripped) — an information-set tree. Extend `decision_key` to a real hash table across the whole search (and across consecutive decisions) instead of per-node maps; do **not** merge across seats | fewer duplicate subtrees after different dice/draw prefixes; bigger win in a 2–3 round tree than in a deep one | low. Risk: key collisions / partial keys (hidden counters) — keep the consistency tests (determinize_consistency) green |
| **Progressive widening** | standard for large / continuous action spaces (e.g. [Couëtoux et al. 2011](https://www.sciencedirect.com/science/article/pii/S2405896325020105) lineage); only expand `k = c · n^α` children | our auction bid set is already a 5-level abstraction (BOT.md §3.4) — good. For card-play roots with 10+ legal plays, widen with visits instead of expanding all at once | less wasted iteration budget on hopeless actions at shallow roots | low; pairs with progressive unpruning above |
| **Determinization reuse / re-determinization** | ISMCTS deliberately re-determinizes per iteration so statistics pool over the info set ([Cowling, Powley & Whitehouse 2012](https://eprints.whiterose.ac.uk/id/eprint/75048/1/CowlingPowleyWhitehouse2012.pdf)); [Goodman 2019, re-determinizing IS-MCTS](https://arxiv.org/abs/1902.06075) blocks hidden-info leakage in Hanabi; [Bitan & Kraus 2017, SDMCTS](https://arxiv.org/abs/1709.09451) fixes unobservable opponent moves from a model | keep the fresh determinization per iteration (it is the sound part of our design). Two cheap variants worth trying: (a) **one determinization, several tree iterations** before re-sampling (saves the ~10 ms materialisation amortised); (b) sample opponents' decks from the deck book prior (already planned, BOT.md §3.7) instead of uniform | (a) up to 2× if materialisation dominates; (b) better decisions per iteration | (a) medium — risks overweighting one sample of hidden state; (b) low, already specified |
| **Open-loop trees** | [Lecarpentier, Infantes, Lesire & Rachelson, IJCAI 2018](https://www.ijcai.org/proceedings/2018/0327.pdf): tree nodes are *action sequences*, not states; no state storage, re-simulate; chance is absorbed into the generative model; they also show subtree reuse without re-planning is sound-ish (optimal-action probability bound decays only logarithmically). Related: [Weinstein & Littman open-loop MCTS](https://www.ijcai.org/proceedings/2018/0327.pdf) lineage, and "[Open Loop Search for General Video Game Playing](http://www.cmap.polytechnique.fr/~nikolaus.hansen/proceedings/2015/GECCO/proceedings/p337.pdf)" (GECCO 2015) | our tree is already close: it keys on the *public view*, not on a stored `World`, and dice/draws are resolved inside the generative rollout. Going full open-loop (nodes = the seat's action sequence, state re-simulated from a determinization on demand) removes the per-node snapshot cost and makes dice **chance-as-transition** instead of an explicit chance node | removes the state-storage half of iteration cost; simplifies the [反击] windows (no need to snapshot them into the tree) | medium. Risk: open-loop error — a plan that would adapt to an observed draw/counteraction is not representable. With a 2–3 round horizon and myopic roots this is acceptable; re-evaluate if trees get deeper |

**PIMC and its pitfalls** (the determinization family our ISMCTS sits on):

* **Strategy fusion** and **non-locality** are the two classical failure
  modes, from [Frank & Basin 1998](https://www.sciencedirect.com/science/article/pii/S0004370214001052)
  (AIJ 100:87–123; cited as [5] in Cowling et al.). Strategy fusion: PIMC
  assumes it may pick a different move in each hidden state of the same
  information set. Non-locality: a node's value depends on distant parts of
  the tree, because the opponent's private information steers play.
* **Why PIMC still works:** [Long, Sturtevant, Buro & Furtak, AAAI 2010](https://ojs.aaai.org/index.php/AAAI/article/view/7562)
  ("Understanding the success of perfect information Monte Carlo sampling…")
  isolates synthetic-tree properties that predict when PIMC is near-optimal,
  and shows they are measurable in real games. The failure shape needs
  *anti-correlated* values across hidden states **and** a guaranteed
  alternative — e.g. a [反击] that beats one opponent hand and loses to
  another, next to a safe out. That exists here (counteraction windows), so
  pure PIMC/averaging would occasionally be fooled; SO-ISMCTS's single info-set
  tree removes the first mechanism.
* **Our current design is already the recommended one**: single-observer
  ISMCTS (Cowling et al.'s SO-ISMCTS) with per-iteration determinization
  handles the common strategy-fusion case and pools statistics over the
  information set. MO-ISMCTS (a tree per player) matters when opponents'
  moves are partially observable — our [反击] offers are public once made,
  so SO + the "partial observability" tweak is enough.
* **EPIMC** ([Arjonilla, Saffidine & Cazenave 2024](https://arxiv.org/abs/2408.02380))
  postpones the perfect-information resolution to reduce strategy fusion —
  a cheap variant to know about if we ever fall back to PIMC-style root
  averaging.

**Chance nodes (dice, draws).** 2d6 is small enough to enumerate at the root
(expectimax over the 11 outcomes) only for the *first* decision of a turn;
past that, the draw-pile and event-deck branching makes explicit chance nodes
infeasible. Sparse sampling ([Kearns, Mansour & Ng, IJCAI 1999 / JMLR 2002](https://www.cis.upenn.edu/~mkearns/papers/sparsesampling-journal.pdf))
and Monte-Carlo \*-minimax ([arXiv:1304.6057](https://arxiv.org/abs/1304.6057))
are the principled versions of what our rollout already does: sample the
chance outcomes inside the playout. Open-loop trees (above) make this explicit
by never creating chance nodes at all.

---

## 3. Action abstraction / pruning

| technique | mapping to our engine | gain | effort / risk |
|---|---|---|---|
| **Search only the decisions that matter** | already the design: roll / end / discard / trivial prompts are delegated to `aiAnswer` (`heuristic: true` in B5); only cards, buy/build, auctions, counteract offers, agent/tile picks are searched. B5: 21 of 24 decisions were delegated | keep it. Any new prompt type should default to delegated until it shows up often | — |
| **Macro-actions** | e.g. "buy if `wants_buy`, else end" as one root action; or a counteraction offer + follow-up spend as one move. [Macro-actions in MCTS](https://arxiv.org/pdf/1606.04615) show the usual win: fewer, more meaningful branches | our roots are already 2–4 actions. Macros help deeper in the tree (card chains), not at the root | 10–30 % when we search deeper | medium. Risk: losing the option to deviate mid-macro; only use for near-deterministic sequences |
| **Heuristic move ordering + top-k** | order the expansion of card plays by the heuristic's `ai_picked` score; expand top 3 first (progressive unpruning) | less budget on obviously bad card plays | low |
| **Bid abstraction** | we use 5 bid levels (min / min+100 / ¾·quote / quote / money−1000). The auction literature (e.g. [Sandholm's CA work](http://www.cs.cmu.edu/~sandholm/cs15-892F15/Bidding%20and%20allocation%20in%20CAs.pdf)) would call this a value-based bid abstraction — fine at this scale | keep 5 levels; make the levels adaptive to the quote (relative) rather than absolute | low |

---

## 4. Parallelism

| technique | what the literature says | mapping to our engine | gain | effort / risk |
|---|---|---|---|---|
| **Root parallelization** | [Sephton, Cowling, Powley & Whitehouse, WCCI 2014](https://edpowley.com/academic/papers/wcci2014-ismcts-parallelization.pdf) compare Root / Tree / Tree+virtual-loss / Leaf parallelization **for ISMCTS specifically**: root is the most efficient choice, tree parallel stays efficient for ISMCTS (its per-iteration determinization reduces lock contention vs plain UCT), **virtual losses did not help their game**, leaf is worst | our determinizer already gives each iteration its own world. Run **N independent ISMCTS searches** (one per worker thread, each with its own determinization stream and own tree) and merge visit counts / values at the root when the budget expires. No locks, no shared tree, near-linear speedup | server: **~4× iterations** (4 workers, BOT.md §3.5). Browser: 1 worker → no gain unless we add a small pool; phones are 3–5× slower anyway so 1 thread is the right default | **low** — `bot-service` already runs one search thread per request; this is a flag. Risk: tiny — 4 short searches ≈ 1 long one for UCT; only the root action split needs the merge |
| **Tree parallelization + virtual loss** | same paper; also "[An Analysis of Virtual Loss in Parallel MCTS](https://liacs.leidenuniv.nl/~plaata1/papers/paper_ICAART17.pdf)" (ICAART 2017) | only worth it if we want one shared tree across threads (e.g. deep trees with strong transpositions). Not the first step | 2–3× with more engineering | high: locking / consistency with an information-set tree. Do after root parallel |
| **Batch MCTS / vectorised sims** | [Batch MCTS (Cazenave)](https://www.lamsade.dauphine.fr/~cazenave/papers/BatchMCTS.pdf); AlphaZero-style batched leaf evaluation | only pays off with a *learned* evaluator that wants batches. Our cost is engine shell, not NN inference — skip until §7 lands | — | — |

**Fit.** Server: root-parallel over the 4 `BOT_THREADS` is the single cheapest
throughput multiplier in this whole document. Browser: one Web Worker is
correct; the phone budget stays the constraint, so the per-iteration wins
(§5, §8) matter more there than parallelism.

---

## 5. Anytime / budgeting / tree reuse

| technique | mapping to our engine | gain | effort / risk |
|---|---|---|---|
| **Pondering during opponents' turns** | standard in game engines (Fuego: [Mueller et al., TCIAIG 2010](https://webdocs.cs.ualberta.ca/~mmueller/ps/fuego-TCIAIG.pdf)); the time is free — the answer is only needed when the prompt reaches us | the server already polls idle seats every 200 ms (`server/src/botsvc.rs`). Point those polls at a **speculative search** from the current view whenever a counteraction / offer window is plausible, and keep the tree keyed by the public view. The B5 budget then becomes *extra* on top of free CPU | effective budget up to 2–5× on busy boards, at zero latency cost | low–medium. Risk: wasted work when the window never opens; risk of stale trees after a hidden-revealing event — invalidate the tree on any public state change (the view hash is the key) |
| **Tree reuse between consecutive decisions** | [Lecarpentier et al. 2018](https://www.ijcai.org/proceedings/2018/0327.pdf) (OLTA) show a reused subtree with a re-plan criterion keeps most of the strength and saves most of the rebuild cost; the bound on picking a suboptimal action inside a kept subtree decays only logarithmically | our decisions are often consecutive on one seat (buy → build → end, or a chain of [反击] offers). Keep the subtree under the action we just played, drop the rest; re-plan from scratch whenever the view hash's public fields change beyond the played action | 1.5–2× effective iterations in decision bursts | low. Risk: determinization mismatch (the kept subtree's statistics pool over *previous* determinizations) — that is exactly ISMCTS's intended sharing, so it is safe |
| **Time management** | spend less on obvious decisions, more on close ones ([V-MCTS](https://openreview.net/forum?id=B_LdLljS842) spends thinking time per difficulty; AlphaZero-style N-sims-per-move is the simple version) | B5 already uses `min(1000 ms, prompt time left − 200 ms)`. Add a **difficulty proxy**: stop early when the root's top-2 visit counts are far apart (a cheap sequential-halving stop rule) | 10–30 % budget freed for hard decisions | low |
| **Speculative tree traversal** | [SpecMCTS](https://ieeexplore.ieee.org/abstract/document/9576093) (2021) pipelines selection ahead of backup to hide simulator latency | relevant only if rollouts become the pipeline bottleneck *and* we have spare cores. Root parallel covers this better for us | — | — |

---

## 6. Learning-based accelerators (feasibility for us)

| approach | what it is | feasibility here | inference cost |
|---|---|---|---|
| **AlphaZero-style policy+value net** | self-play, MCTS as policy improvement | **not feasible**: 285 cards with wasm effects, hidden decks, 4 seats, counteraction windows — the simulator is ~100 ms/game (StubRules) to ~15 s/game (real rules). Self-play at AlphaZero scale is out. A *small* net over handcrafted features is feasible (below) | n/a |
| **Student of Games / ReBeL** | unified perfect/imperfect-info learning ([arXiv:2112.03178](https://arxiv.org/abs/2112.03178); [arXiv:2007.13544](https://arxiv.org/abs/2007.13544)) via public-belief-state search | conceptually the right family (public state = our `MatchState` view), but they need huge compute and a *ground-truth* info-state abstraction we do not have. Read them for the public-belief framing, do not attempt the training | n/a |
| **Distilled value from search / records** | run the current ISMCTS offline, record (view features → root value / best action); train a value/policy head; use it as the rollout cutoff eval and as progressive bias | **the practical path.** We already have: replay records, the deckbook self-play harness (`crates/game-rules/examples/deckbook.rs`, multi-threaded, resumable), and a feature set in `eval::relative_worth` | linear model / GBDT: < 1 µs. Tiny MLP (e.g. 128×64): ~1–5 µs in native, maybe 20–50 µs in wasm — negligible next to a 50 ms rollout |
| **n-tuple networks** | small lookup tables over board feature windows, trained by TD ([Szubert & Jaśkowski 2014](https://mszubert.github.io/papers/Szubert_2014_CIG.pdf)) | good fit for *board* features (tile ownership, house counts, cash bands); weak for hidden-hand terms (those stay in the determinizer). Train on self-play from the deckbook pipeline | a few hundred table lookups; free |

**Recommendation on learning:** do not block anything on it. Once the
simulator is 5–20× cheaper (§7 items 1 and 2), self-play becomes affordable and
a small value head is worth training *as the rollout cutoff eval*. Until then,
a handcrafted eval + progressive bias is the better use of the same effort.

---

## 7. Simulator-side tricks (our profile, our levers)

Our engine profile is dominated by things the MCTS literature calls "the
simulator", not by the search. These are the highest-leverage items in the
whole document.

| trick | evidence in our numbers | expected gain | status |
|---|---|---|---|
| **Static counteraction eligibility cache** | 48 900 `can_counteract` probes/game; each builds a `Run` + `cx.world_copy()` *before* the guard (GUARDS.md §1). A card can only answer a trigger kind its manifest declares | probe cost → a bitmask test for most cards; the shell's 81 % share should collapse toward the StubRules floor | **planned** — GUARDS.md G0–G2 landed (declares bitmask / `counteracts_to`), G3/G4 move real cards' clauses into `pre` filters. This *is* the "cached counteraction index" |
| **Native rules (`rules-native`)** | B1 measured 1.4× end-to-end; its real value is enabling B2 and cheap forks | already shipped (B1) | done; turn the feature on by default once the drift check closes |
| **Inline answers (B2)** | 1.3–1.8× measured; removes prompt re-runs | shipped (B2) | done; the remaining gap is the equivalence rebase (write journal) |
| **Tree policy as the answer provider** | BOT.md §3.2 "next increment" | removes halt/replay during descent | planned |
| **Native in-memory fork instead of save-JSON** | B4: 14 ms/iteration on StubRules, ~10 ms of it save→restore. `Match::fork` (B2) already clones a `World` in ~6 µs | **~10×** on iteration overhead where materialisation dominates (StubRules today; real rules after the counteract cache lands) | not started (needs a `Match::fork_from` / direct-`World` determinizer path) |
| **Incremental / COW world (B3)** | clone is 5–6 µs, ~7 % of a game (BOT.md §5) | low value until the above land; re-profile then | deferred (correctly) |
| **Avoid re-running deterministic prefixes** | B2's two-pass drive is exactly this (two guest runs per body, not *k*+1) | shipped | done |
| **Warm module / avoid fire-up** | wasmi fire-up is 57 % of a run; wasmtime 17 % | `rules-native` removes it entirely for search | done |

---

## 8. Prioritised recommendations (top 5)

Ordered by value / effort for *our* situation. "Impact" is what each does to
the B5 metric we care about: **iterations per decision** (today 1.0 at 200 ms,
3.7 at 1 000 ms; target 40–100+) and play strength.

| # | idea | expected impact | effort | already planned? | what to measure |
|---|---|---|---|---|---|
| **1** | **Static counteract eligibility cache + GUARDS `pre` migration (G3/G4)** — replace per-card `can_counteract` probes with a trigger-kind → candidate-card index; no `Run` / `world_copy` for cards that cannot answer | real-ruleset rollout 131 ms → 20–40 ms (**3–6×**); iterations 1–4 → 8–25 at 200 ms. Biggest single win, and it compounds with everything else | medium (design done in `docs/GUARDS.md`) | **yes** — GUARDS.md G0–G2 landed, G3/G4 pending (after purchasing) | `bot-cost` counters: probes/game, ms/round, ms per depth-2 rollout (BOT.md §5 bench) |
| **2** | **Root-parallel ISMCTS** — 4 independent searches, one per `BOT_THREADS` worker, each with its own determinization stream; merge root visit counts at budget end (Sephton et al.: root is the efficient choice for ISMCTS; virtual loss did not help) | server **~4×** iterations at zero latency cost; browser unchanged (1 worker is right for phones) | **low** (one flag in `bot-service`) | no | iterations/decision at 200/1000 ms under 4 concurrent rooms; win rate vs standard bot must not drop (it should rise) |
| **3** | **Cheap rollouts: horizon 1–2 + implicit-minimax eval + progressive bias from `ai.rs`** — store heuristic values separately and back them up minimax-style (Lanctot et al. CIG 2014); fold `ai_picked` / `wants_buy` into UCB as a decaying prior (Chaslot et al. 2008) | another **2–3×** iterations and better strength per iteration; directly substitutes for the depth-3 rollout we cannot afford | medium (two small changes in `bot-core`) | no | same-budget win rate vs current 2-round rollout (B4 harness, then real rules); ablate bias weight `c_h` |
| **4** | **Native fork + tree-policy-as-provider** — kill the 10 ms save/restore determinizer path (use `Match::fork` / a direct `World` materialiser) and close the B2 equivalence gap so descent is one forward pass | **~10×** on iteration overhead where materialisation dominates (StubRules today; the ceiling after #1); also required for any tree deeper than the root | medium (the B2 write-journal increment + a `Match::fork_from` API) | partly — B2 "next increment" documented in BOT.md §3.2 | ms/iteration breakdown (fork vs rollout vs tree) in the B4 harness; `stub_rules_inline_matches_halt_replay_exactly` stays green |
| **5** | **Pondering + tree reuse (open-loop flavour)** — speculative search during opponents' turns (the server already polls idle seats every 200 ms), keep the subtree under the played action across consecutive decisions; nodes keyed by the public view, dice as transition (Lecarpentier et al. 2018) | effective budget **1.5–3×** on busy boards for free; makes the browser budget less painful | low–medium | no | fraction of decisions answered from a pre-pondered tree; win rate; invalidation rate on public-state change |

### What we deliberately do *not* recommend first

* **RAVE/AMAF** — weak action-ordering correlation in our move set; try MAST
  instead if a share-statistics win is wanted.
* **Tree parallelization / virtual loss** — root parallel gets the same 4×
  without lock engineering; the WCCI 2014 ISMCTS results even question virtual
  loss.
* **AlphaZero / Student of Games / ReBeL training** — wrong cost class for a
  285-card wasm ruleset. Revisit only after #1/#2 make self-play cheap, and
  then only as a small distilled value head.
* **Explicit expectimax over dice** — enumerate 2d6 only at the root of a
  turn's first decision if at all; the draws and event deck dominate and stay
  sampled.

### Validation plan (one bench, four flags)

Keep `crates/game-rules/examples/bot_cost.rs` and
`crates/bot-core/examples/ismcts_vs_bots.rs` as the two gates:

1. **ms per depth-2 rollout** (real rules, inline provider) — gate for #1, #4.
2. **ms per iteration** (StubRules and real) with a fork/rollout/tree split —
   gate for #4.
3. **iterations/decision at 200 ms / 1 000 ms**, 1 thread vs 4 — gate for #2.
4. **win rate / score vs standard bot** at a *fixed* iteration budget and at a
   *fixed* wall budget — gate for #3 and for the whole stack. (N ≥ 200 games;
   the deckbook trial's ±1.3 k score noise at n = 24 is too high to decide on.)

---

## References

* P. I. Cowling, E. J. Powley, D. Whitehouse. *Information Set Monte Carlo
  Tree Search*. IEEE TCIAIG 4(2), 2012.
  <https://eprints.whiterose.ac.uk/id/eprint/75048/1/CowlingPowleyWhitehouse2012.pdf>
* N. Sephton, P. I. Cowling, E. Powley, D. Whitehouse. *Parallelization of
  Information Set Monte Carlo Tree Search*. WCCI 2014.
  <https://edpowley.com/academic/papers/wcci2014-ismcts-parallelization.pdf>
* J. Long, N. R. Sturtevant, M. Buro, T. Furtak. *Understanding the Success of
  Perfect Information Monte Carlo Sampling in Game Tree Search*. AAAI 2010.
  <https://ojs.aaai.org/index.php/AAAI/article/view/7562>
* I. Frank, D. Basin. *Search in games with incomplete information: A case
  study using Bridge card play*. Artificial Intelligence 100(1–2), 1998.
  (strategy fusion / non-locality)
* M. Lanctot, M. H. M. Winands, T. Pepels, N. R. Sturtevant. *Monte Carlo Tree
  Search with Heuristic Evaluations using Implicit Minimax Backups*. CIG 2014.
  <https://mlanctot.info/files/papers/cig14-immcts.pdf>
* G. M. J-B. Chaslot, M. H. M. Winands, H. J. van den Herik, J. W. H. M.
  Uiterwijk, B. Bouzy. *Progressive Strategies for Monte-Carlo Tree Search*
  (progressive bias / progressive unpruning). 2008.
  <https://project.dke.maastrichtuniversity.nl/games/files/articles/pMCTS.pdf>
* S. Gelly, D. Silver. *Monte-Carlo tree search and rapid action value
  estimation in computer Go* (RAVE/AMAF). Artificial Intelligence 175(11), 2011.
  <https://ics.uci.edu/~dechter/courses/ics-295/winter-2018/papers/mcts-gelly-silver.pdf>
* E. Lecarpentier, G. Infantes, C. Lesire, E. Rachelson. *Open Loop Execution
  of Tree-Search Algorithms* (OLTA / subtree reuse). IJCAI 2018.
  <https://www.ijcai.org/proceedings/2018/0327.pdf>
* M. Kearns, Y. Mansour, A. Y. Ng. *A Sparse Sampling Algorithm for
  Near-Optimal Planning in Large Markov Decision Processes*. IJCAI 1999 /
  JMLR 2002. <https://www.cis.upenn.edu/~mkearns/papers/sparsesampling-journal.pdf>
* M. Szubert, W. Jaśkowski. *Temporal Difference Learning of N-Tuple Networks
  for the Game of Hex*. CIG 2014.
  <https://mszubert.github.io/papers/Szubert_2014_CIG.pdf>
  and *Systematic N-Tuple Networks for Position Evaluation*
  <https://arxiv.org/abs/1406.1509>
* J. Goodman. *Re-determinizing Information Set Monte Carlo Tree Search in
  Hanabi*. 2019. <https://arxiv.org/abs/1902.06075>
* M. Bitan, S. Kraus. *Combining Prediction of Human Decisions with ISMCTS in
  Imperfect Information Games* (SDMCTS). 2017.
  <https://arxiv.org/abs/1709.09451>
* G. Arjonilla, A. Saffidine, T. Cazenave. *Perfect Information Monte Carlo
  with Postponing Reasoning* (EPIMC). 2024. <https://arxiv.org/abs/2408.02380>
* C. B. Browne et al. *A Survey of Monte Carlo Tree Search Methods*. IEEE
  TCIAIG 4(1), 2012.
  <https://turing.iimas.unam.mx/~luis/cursos/IA2021-1/lecturas/SurveyMCTS.pdf>
* M. Lanctot, K. Waugh, M. Zinkevich, M. Bowling. *Monte Carlo Sampling for
  Regret Minimization in Extensive Games* (MCCFR). NIPS 2009. (background)
* M. Schmid et al. *Student of Games*. Science Advances, 2023.
  <https://arxiv.org/abs/2112.03178> — N. Brown, T. Sandholm et al. *ReBeL*.
  <https://arxiv.org/abs/2007.13544>
* R. Coulom. *Efficient Selectivity and Backup Operators in Monte-Carlo Tree
  Search*. CG 2006. L. Kocsis, C. Szepesvári. *Bandit based Monte-Carlo
  Planning* (UCT). ECML 2006. (foundational)
* ICAART 2017, *An Analysis of Virtual Loss in Parallel MCTS*.
  <https://liacs.leidenuniv.nl/~plaata1/papers/paper_ICAART17.pdf>
* M. Müller et al. *Fuego — An Open-source Framework for Board Games and
  Beyond* (pondering / time management). IEEE TCIAIG 2010.
  <https://webdocs.cs.ualberta.ca/~mmueller/ps/fuego-TCIAIG.pdf>

Local context: `docs/BOT.md` (architecture + measurements), `docs/GUARDS.md`
(counteract prefilter G0–G4), `docs/ENGINE.md` (sandbox run cost),
`docs/REPLAY.md` (records available as training data).
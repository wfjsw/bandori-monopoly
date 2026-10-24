//! Minimal harness for the engine-universal integration tests in this
//! directory (chain / counteraction windows, activation log, movement events,
//! settle-stack, ruleset loader, …).
//!
//! Black-box: a real [`Match`] driven the way a client drives it (`act`
//! commands, prompt answers), with the board arranged through the `world_mut`
//! test seam. Every player is a human in a Solo match, so nothing happens on
//! its own -- no bot turns, no prompt time-outs -- and a counteraction window is
//! offered to whoever holds an eligible card.
//!
//! Seats are fixed: player `k` is member `k + 1` and sits in seat `k`, so every
//! `who` below is both the player number and the engine's player id.
//!
//! Test bodies here name only `TEST:*` fixtures (`rules/fixtures/test-cards`).
//! The match still boots a real opening deal, so the loader also stages
//! `dist/cards`; `clean()` wipes the deal before each test. A fixtures-only
//! boot is the long-term ideal (see docs/rulebook/TEST-FINDINGS.md).
//!
//! The game-specific suite (shipped cards / skills / this game's rulebook
//! clauses) lives in `rules/tests/tests/` on the full harness there.
//!
//! Build the modules first: `node tools/build-ruleset.mjs`.
#![allow(dead_code)]

use std::path::Path;
use std::sync::{Arc, OnceLock};

use game_core::data::{GameData, Roster};
use game_core::engine::CardRules;
use game_core::engine::Match;
use game_core::msg::{Arg, Msg};
use game_core::net::{NetMessage, RoomMember};
use game_core::scoring::ScoreWeights;
use game_core::state::{
    prop, stage, ActiveEvent, FieldCard, MatchEvent, MatchPlayer, MatchPrompt, MatchState,
    StateVar, TileMark,
};
use game_core::MatchMode;
use game_rules::{Ruleset, WasmRules};

pub const SEED: u64 = 20261006;

/// Opaque engine-side scratch keys (not game vocabulary).
const K_LAYER_A: &str = "k1";
const K_LAYER_B: &str = "k2";
const K_LAYER_C: &str = "k3";
const K_LAYER_D: &str = "k4";
const K_LAYER_E: &str = "k5";
const K_TILE_SLOT: &str = "k6";
const K_CHARGE: &str = "charge"; // generic counter name, not a status

/// The shared tables. The roster comes from the three legacy JSON files
/// (`Roster::from_fixture`) rather than the ruleset: `data()` is a `OnceLock`
/// that `rules()` itself calls, so it cannot ask the ruleset without inverting
/// that dependency. Least invasive and matches the game-core tests; swap for a
/// `dump_roster` fixture when `data/{cards,personas,groups}.json` retire.
pub fn data() -> Arc<GameData> {
    static DATA: OnceLock<Arc<GameData>> = OnceLock::new();
    DATA.get_or_init(|| {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let roster = Roster::from_fixture(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../rules/tests/fixtures/roster.json"),
            )
            .expect("roster fixture"),
        )
        .expect("roster fixture parses");
        Arc::new(
            GameData::load(
                |f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()),
                roster,
            )
            .unwrap(),
        )
    })
    .clone()
}

/// The shipped cards and skills plus the `TEST:*` fixtures, loaded once.
pub fn rules() -> Arc<WasmRules> {
    static RULES: OnceLock<Arc<WasmRules>> = OnceLock::new();
    RULES
        .get_or_init(|| {
            // Kind flags ride the `ruleset_kind_flags` export on a rebuilt
            // module; older dist wasm may lack it, so seed from the ruleset
            // declaration the tests already link (`bandori-std` is a dev-dep).
            game_rules::set_kind_flags(bandori_std::kind::kind_hook_only());
            let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist");
            let mut b = Ruleset::builder();
            for sub in ["cards", "fixtures"] {
                let dir = root.join(sub);
                let index: serde_json::Value = serde_json::from_str(
                    &std::fs::read_to_string(dir.join("index.json"))
                        .expect("run node tools/build-ruleset.mjs"),
                )
                .unwrap();
                for m in index["modules"].as_array().unwrap() {
                    b.add(&std::fs::read(dir.join(m["file"].as_str().unwrap())).unwrap())
                        .unwrap();
                }
            }
            Arc::new(WasmRules::new(b.build().unwrap(), data()))
        })
        .clone()
}

/// Board index of the tile with this exact name (`data/board.json`). The
/// rulebook's 「#N格」 is index `N - 1`.
pub fn tile(name: &str) -> usize {
    data()
        .tiles
        .iter()
        .position(|t| t.name == name)
        .unwrap_or_else(|| panic!("no tile named {name:?}"))
}

/// Every persona name in the roster, in roster order.
pub fn personas() -> Vec<String> {
    data().personas.iter().map(|c| c.name.clone()).collect()
}

pub struct Table {
    pub m: Match,
    pub n: usize,
}

impl Table {
    /// `chars.len()` players; player `k` plays `chars[k]` (skills bound, the
    /// opening run as in a real game). Runs to player 0's 运营 stage, answering
    /// the opening prompts (mulligan etc.) with their defaults.
    pub fn new(chars: &[&str]) -> Self {
        Self::with_seed(chars, SEED)
    }

    /// [`Self::new`] with extra cards on top of each player's opening draw
    /// pile **before** `quick_start`, so a `deckBeforeGame` hook sees them.
    pub fn with_opening_draw(chars: &[&str], top_first: &[&str]) -> Self {
        let names = personas();
        let cs: Vec<&str> = names.iter().take(chars.len()).map(String::as_str).collect();
        let mut t = Self::new_with_draw(&cs, top_first);
        t.strip_skills();
        t.clean();
        t.begin_turn(0);
        t
    }

    fn new_with_draw(chars: &[&str], top_first: &[&str]) -> Self {
        use game_core::engine::{CardRules, Match};
        use game_core::net::RoomMember;
        use game_core::scoring::ScoreWeights;
        let n = chars.len();
        let members: Vec<RoomMember> = (1..=n as i32)
            .map(|id| RoomMember {
                id,
                player: format!("P{}", id - 1),
                bot: false,
                ..Default::default()
            })
            .collect();
        let mut m = Match::new(
            data(),
            rules(),
            &members,
            SEED,
            MatchMode::Solo,
            ScoreWeights::default(),
        );
        {
            let w = m.world_mut();
            w.st.players.sort_by_key(|p| p.member);
        }
        for (k, c) in chars.iter().enumerate() {
            if !c.is_empty() {
                m.set_persona(k as i32 + 1, c);
            }
        }
        {
            // Deterministic piles, then seed the draw pile **below** the
            // opening deal so `deckBeforeGame` sees the card and it is still
            // in the pile when the hook runs (the deal takes from the top).
            let w = m.world_mut();
            for h in w.hidden.iter_mut() {
                h.hand.clear();
                h.draw.clear();
                h.discard.clear();
            }
            for card in top_first.iter().rev() {
                w.hidden[0].draw.insert(0, card.to_string());
            }
        }
        m.quick_start();
        let mut t = Table { m, n };
        t.settle_answering_defaults();
        t
    }

    pub fn with_seed(chars: &[&str], seed: u64) -> Self {
        Self::with_data(data(), chars, seed, MatchMode::Solo)
    }

    /// [`Table::with_seed`] on a private `GameData` clone carrying an
    /// in-memory strategy book (the shared `data()` remains untouched). The
    /// book's `ruleset_sha256` is filled in from the running ruleset when it
    /// is empty, so a test only names the entries it cares about.
    pub fn with_book(
        chars: &[&str],
        mut book: game_core::strategy::StrategyBook,
        mode: MatchMode,
    ) -> Self {
        if book.ruleset_sha256.is_empty() {
            book.ruleset_sha256 = rules().ruleset_sha256().unwrap_or("stub").into();
        }
        let mut d = (*data()).clone();
        d.strategy_book = book;
        Self::with_data(Arc::new(d), chars, SEED, mode)
    }

    /// The shared constructor: private `GameData`, seats `k` = player `k`,
    /// opening run to P0's 运营. `mode` matters for prompt deadlines -- Solo
    /// never forces a local seat's answer, Casual (online) times out to the
    /// fallback after the prompt's clock + `REMOTE_GRACE`.
    pub fn with_data(
        data: Arc<game_core::data::GameData>,
        chars: &[&str],
        seed: u64,
        mode: MatchMode,
    ) -> Self {
        let n = chars.len();
        assert!((2..=10).contains(&n), "2..=10 players");
        let members: Vec<RoomMember> = (1..=n as i32)
            .map(|id| RoomMember {
                id,
                player: format!("P{}", id - 1),
                bot: false,
                ..Default::default()
            })
            .collect();
        let mut m = Match::new(data, rules(), &members, seed, mode, ScoreWeights::default());
        // Undo the random seating so seat == player number.
        {
            let w = m.world_mut();
            w.st.players.sort_by_key(|p| p.member);
        }
        for (k, c) in chars.iter().enumerate() {
            if !c.is_empty() {
                m.set_persona(k as i32 + 1, c);
            }
        }
        m.quick_start();
        let mut t = Table { m, n };
        t.settle_answering_defaults();
        t
    }

    /// Mark `who` as a bot seat under `mentality`. Standard / chaos flip `ai`
    /// on (the engine's bot schedule answers, via `Cx::fill_ai`); Advanced
    /// leaves `ai` off -- the seat is held for an external driver (bot-service
    /// / the browser worker) and its prompts time out to the fallback.
    pub fn make_bot(&mut self, who: usize, mentality: game_core::state::BotMentality) {
        use game_core::state::BotMentality;
        let p = &mut self.m.world_mut().st.players[who];
        p.bot = true;
        p.mentality = mentality;
        p.ai = mentality != BotMentality::Advanced;
    }

    /// Tick the host clock until `pred` (or the tick budget runs out). Lets a
    /// test wait for a bot seat's scheduled answer / a held seat's deadline.
    pub fn tick_until(&mut self, what: &str, mut pred: impl FnMut(&mut Self) -> bool) {
        for _ in 0..400 {
            if pred(self) {
                return;
            }
            self.m.tick(0.25);
        }
        panic!("tick_until: {what} never happened: {}", self.dump_prompt());
    }

    /// `n` players with no skills at all and a clean board -- the setting for a
    /// card's own behaviour. Personas are still set (exclusive cards read
    /// them; use [`Table::set_persona_raw`] to change one).
    pub fn vanilla(n: usize) -> Self {
        let names = personas();
        let chars: Vec<&str> = names.iter().take(n).map(String::as_str).collect();
        let mut t = Self::new(&chars);
        t.strip_skills();
        t.clean();
        t.begin_turn(0);
        t
    }

    // ------------------------------------------------------------ flow

    /// Tick until the match is at rest: a prompt is waiting, or the current
    /// player is in 运营 / 结束 with nothing queued.
    pub fn settle(&mut self) {
        for _ in 0..2000 {
            let st = self.m.state();
            if st.phase != "play" || st.prompt.id != 0 {
                return;
            }
            let w = self.m.world();
            if !w.next_turn_pending
                && w.leftovers.is_empty()
                && !st.busy
                && (st.step == stage::OPS || st.step == stage::END)
            {
                return;
            }
            self.m.tick(0.25);
        }
        panic!(
            "settle: the match did not come to rest: {:?}",
            self.m.state().step
        );
    }

    /// [`Table::settle`], answering every prompt that comes up with its
    /// default (for setup only -- a test should answer its own prompts).
    pub fn settle_answering_defaults(&mut self) {
        for _ in 0..200 {
            self.settle();
            let Some(p) = self.prompt() else { return };
            for who in self.asked() {
                let v = if p.kind == "tile" {
                    p.items.len() as i32
                } else {
                    p.fallback
                };
                let mut msg = NetMessage {
                    prompt: p.id,
                    value: v,
                    ..NetMessage::act("answer")
                };
                if p.kind == "pick" || p.kind == "mortgage" {
                    msg.cards = p
                        .items
                        .iter()
                        .take(p.count.max(0) as usize)
                        .cloned()
                        .collect();
                }
                let _ = self.m.act(who as i32 + 1, &msg);
            }
        }
        panic!("settle_answering_defaults: prompts never stopped");
    }

    /// Start `who`'s turn now, through the real turn-start flow (hooks,
    /// status ticks), and run to its 运营 stage or the first prompt.
    pub fn begin_turn(&mut self, who: usize) {
        self.settle();
        assert!(self.prompt().is_none(), "begin_turn with a prompt open");
        let n = self.n as i32;
        let w = self.m.world_mut();
        w.st.turn = (who as i32 - 1 + n) % n;
        w.extra_turns.clear();
        w.next_turn_pending = true;
        self.settle();
    }

    fn act(&mut self, who: usize, m: NetMessage) -> Result<(), String> {
        let r = self
            .m
            .act(who as i32 + 1, &m)
            .map_err(|e| e.key().to_string());
        self.settle();
        r
    }

    /// Play `card` from `who`'s hand (it must be there -- see [`Table::give`]).
    pub fn play(&mut self, who: usize, card: &str) -> Result<(), String> {
        self.act(
            who,
            NetMessage {
                card: card.into(),
                ..NetMessage::act("play")
            },
        )
    }

    /// Give `card` to `who` and play it.
    pub fn give_play(&mut self, who: usize, card: &str) -> Result<(), String> {
        self.give(who, &[card]);
        self.play(who, card)
    }

    /// Use a skill on `who`'s field (`skill:<owner>:<skill>` -- see
    /// [`Table::skills`]).
    pub fn skill(&mut self, who: usize, skill: &str) -> Result<(), String> {
        self.act(
            who,
            NetMessage {
                card: skill.into(),
                ..NetMessage::act("skill")
            },
        )
    }

    /// The main roll (loaded dice: [`Table::dice`]).
    pub fn roll(&mut self, who: usize) -> Result<(), String> {
        self.act(who, NetMessage::act("roll"))
    }

    /// End the turn; runs on to the next player's 运营 (or a prompt).
    pub fn end(&mut self, who: usize) -> Result<(), String> {
        self.act(who, NetMessage::act("end"))
    }

    /// Accept the settle's optional buy (the only buy path).
    pub fn buy(&mut self, who: usize) -> Result<(), String> {
        let _ = who;
        let p = self
            .prompt()
            .ok_or_else(|| "no buy prompt open".to_string())?;
        if !format!("{:?}", p.title).contains("buy")
            && !format!("{:?}", p.title).contains("ask.buy")
        {
            return Err(format!("open prompt is not a buy: {}", self.dump_prompt()));
        }
        self.answer_one(0)
    }

    /// Accept the settle's optional build (the only build path).
    pub fn build(&mut self, who: usize) -> Result<(), String> {
        let _ = who;
        let p = self
            .prompt()
            .ok_or_else(|| "no build prompt open".to_string())?;
        if !format!("{:?}", p.title).contains("build")
            && !format!("{:?}", p.title).contains("ask.build")
        {
            return Err(format!(
                "open prompt is not a build: {}",
                self.dump_prompt()
            ));
        }
        self.answer_one(0)
    }

    pub fn mortgage(&mut self, who: usize, tile: usize) -> Result<(), String> {
        self.act(
            who,
            NetMessage {
                value: tile as i32,
                ..NetMessage::act("mortgage")
            },
        )
    }

    pub fn redeem(&mut self, who: usize, tile: usize) -> Result<(), String> {
        self.act(
            who,
            NetMessage {
                value: tile as i32,
                ..NetMessage::act("redeem")
            },
        )
    }

    pub fn discard_card(&mut self, who: usize, card: &str) -> Result<(), String> {
        self.act(
            who,
            NetMessage {
                card: card.into(),
                ..NetMessage::act("discard")
            },
        )
    }

    // ------------------------------------------------------------ prompts

    /// The prompt waiting for an answer, if any.
    pub fn prompt(&self) -> Option<MatchPrompt> {
        let p = self.m.state().prompt;
        (p.id != 0).then_some(p)
    }

    /// Players the open prompt still waits on.
    pub fn asked(&self) -> Vec<usize> {
        self.prompt()
            .map(|p| {
                p.players
                    .iter()
                    .zip(&p.answers)
                    .filter(|(_, &a)| a < 0)
                    .map(|(&s, _)| s as usize)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The open prompt, panicking with the state if there is none.
    pub fn expect_prompt(&self) -> MatchPrompt {
        self.prompt()
            .unwrap_or_else(|| panic!("expected a prompt; events: {:?}", self.recent_keys(15)))
    }

    /// Answer the open prompt as `who`: an option index for `choice`-like
    /// prompts, an index into `items` for `tile` (`items.len()` = none).
    pub fn answer(&mut self, who: usize, value: i32) -> Result<(), String> {
        let p = self.expect_prompt();
        self.act(
            who,
            NetMessage {
                prompt: p.id,
                value,
                ..NetMessage::act("answer")
            },
        )
    }

    /// Answer whoever the open prompt waits on (there must be exactly one).
    pub fn answer_one(&mut self, value: i32) -> Result<(), String> {
        let asked = self.asked();
        assert_eq!(asked.len(), 1, "answer_one: asked {asked:?}");
        self.answer(asked[0], value)
    }

    /// Answer a `tile` prompt by board index.
    pub fn answer_tile(&mut self, who: usize, tile: usize) -> Result<(), String> {
        let p = self.expect_prompt();
        let k = p
            .items
            .iter()
            .position(|t| t == &tile.to_string())
            .unwrap_or_else(|| panic!("tile {tile} not offered: {:?}", p.items));
        self.answer(who, k as i32)
    }

    /// Answer a `pick` / `mortgage` prompt with these items.
    pub fn answer_items(&mut self, who: usize, items: &[&str]) -> Result<(), String> {
        let p = self.expect_prompt();
        self.act(
            who,
            NetMessage {
                prompt: p.id,
                value: 0,
                cards: items.iter().map(|s| s.to_string()).collect(),
                ..NetMessage::act("answer")
            },
        )
    }

    /// Answer every waiting player with the prompt's default (decline).
    pub fn decline(&mut self) {
        let p = self.expect_prompt();
        for who in self.asked() {
            let v = if p.kind == "tile" {
                p.items.len() as i32
            } else {
                p.fallback
            };
            let _ = self.m.act(
                who as i32 + 1,
                &NetMessage {
                    prompt: p.id,
                    value: v,
                    ..NetMessage::act("answer")
                },
            );
        }
        self.settle();
    }

    /// Index of the first option whose message (key or any argument) mentions
    /// `needle` -- e.g. a card id in a counteraction window (`ask.counteract.play`).
    pub fn option(&self, needle: &str) -> Option<i32> {
        let p = self.prompt()?;
        p.options
            .iter()
            .position(|o| format!("{o:?}").contains(needle))
            .map(|k| k as i32)
    }

    /// Does the open prompt's title or text mention `needle`? For `ask_yes`
    /// prompts the question lives there -- the options are bare `ask.yes` /
    /// `ask.no` and never name the asker.
    pub fn prompt_refers(&self, needle: &str) -> bool {
        self.prompt().is_some_and(|p| {
            format!("{:?}", p.title).contains(needle) || format!("{:?}", p.text).contains(needle)
        })
    }

    /// Is the open prompt a counteraction window offering `card`? Matches both the
    /// hand-card label (`ask.counteract.play`) and the field-skill label
    /// (`ask.counteract.skill`).
    pub fn counteract_offered(&self, card: &str) -> bool {
        self.prompt().is_some_and(|p| {
            p.title.key() == "ask.counteract.title"
                && p.options.iter().any(|o| {
                    (o.key() == "ask.counteract.play" || o.key() == "ask.counteract.skill")
                        && format!("{o:?}").contains(card)
                })
        })
    }

    /// Declare `card` in the open counteraction window as `who`.
    pub fn counteract(&mut self, who: usize, card: &str) -> Result<(), String> {
        let k = self
            .option(card)
            .unwrap_or_else(|| panic!("{card} not offered: {}", self.dump_prompt()));
        self.answer(who, k)
    }

    /// One-line description of the open prompt, for assertion messages.
    pub fn dump_prompt(&self) -> String {
        match self.prompt() {
            None => "<no prompt>".into(),
            Some(p) => format!(
                "kind={} players={:?} title={} text={:?} options=[{}] items={:?} count={} fallback={}",
                p.kind,
                p.players,
                p.title.key(),
                p.text,
                p.options
                    .iter()
                    .enumerate()
                    .map(|(i, o)| format!("{i}:{o:?}"))
                    .collect::<Vec<_>>()
                    .join(", "),
                p.items,
                p.count,
                p.fallback
            ),
        }
    }

    // ------------------------------------------------------------ arrange

    /// Strip every bound skill (persona and group) and the state they set.
    pub fn strip_skills(&mut self) -> &mut Self {
        let w = self.m.world_mut();
        for p in w.st.players.iter_mut() {
            p.field.retain(|f| !f.card.starts_with("skill:"));
            p.state = MatchPlayer::default().state;
            p.tokens.clear();
        }
        self
    }

    /// A clean board: empty hands and piles, 10000 each, everyone on the start square,
    /// no deeds / houses / mortgages / marks, no non-skill field cards, no
    /// statuses, no pending turn-end callbacks. Start-time resources (charge pots
    /// 「初始N」, P✽P fans) are wiped too -- their caps remain, so a skill's
    /// 「上限M」 still reads back. Use [`Table::set_charge`] to put a specific pot
    /// back.
    pub fn clean(&mut self) -> &mut Self {
        let w = self.m.world_mut();
        for h in w.hidden.iter_mut() {
            h.hand.clear();
            h.draw.clear();
            h.discard.clear();
            h.next_steps = None;
        }
        for p in w.st.players.iter_mut() {
            p.money = 10_000;
            p.pos = 0;
            // Keep bound skills only. Returns' [特]（2） places it at match
            // start, but its [持续]（2） re-prompts at every one of the owner's
            // turn starts, so a stray Returns is not a clean field. A test
            // that wants Returns places it itself.
            p.field.retain(|f| f.card.starts_with("skill:"));
            for k in [K_LAYER_A, K_LAYER_B, K_LAYER_C, K_LAYER_D, K_LAYER_E] {
                p.state.remove(k);
            }
            // A start-time gate like the charge pots: a group skill's 「无法获取
            // [the pass reward]」 armed during `quick_start` must not leak into a
            // `clean()`-ed board's the start square pass. The veto is `prop::NO_REWARD`
            // on the source's rule instance now (`docs/TILES.md`), not a
            // per-player state latch.
            for f in p.field.iter_mut() {
                f.props.remove(prop::NO_REWARD);
            }
            p.state_set(K_TILE_SLOT, -1);
            // Start-time resources: fire is a held token of the persona-skill
            // instance (MARKERS.md §7), not a state key.
            p.tokens.retain(|t| t.name != K_CHARGE && !t.name.starts_with("P✽P粉丝"));
        }
        for f in w.st.board_field.iter_mut() {
            f.props.remove(prop::NO_REWARD);
        }
        let n = w.st.owners.len();
        w.st.owners = vec![-1; n];
        w.st.houses = vec![0; n];
        w.st.mortgaged = vec![false; n];
        w.st.embers = vec![0; n];
        w.st.marks.clear();
        w.scheduled.clear();
        w.rule_param_bonus.clear();
        self
    }

    /// Set a player's persona **without** binding its skills.
    pub fn set_persona_raw(&mut self, who: usize, name: &str) {
        self.m.world_mut().st.players[who].persona = name.into();
    }

    pub fn set_money(&mut self, who: usize, money: i32) {
        self.m.world_mut().st.players[who].money = money;
    }

    pub fn set_pos(&mut self, who: usize, tile: usize) {
        self.m.world_mut().st.players[who].pos = tile as i32;
    }

    pub fn set_owner(&mut self, tile: usize, who: Option<usize>) {
        self.m.world_mut().st.owners[tile] = who.map_or(-1, |w| w as i32);
    }

    pub fn own(&mut self, who: usize, tiles: &[usize]) {
        for &t in tiles {
            self.set_owner(t, Some(who));
        }
    }

    pub fn set_houses(&mut self, tile: usize, houses: i32) {
        self.m.world_mut().st.houses[tile] = houses;
    }

    pub fn set_mortgaged(&mut self, tile: usize, on: bool) {
        self.m.world_mut().st.mortgaged[tile] = on;
    }

    pub fn give(&mut self, who: usize, cards: &[&str]) {
        self.m.give_cards(who as i32 + 1, cards);
    }

    pub fn set_hand(&mut self, who: usize, cards: &[&str]) {
        self.m.world_mut().hidden[who].hand = cards.iter().map(|s| s.to_string()).collect();
    }

    /// Replace the draw pile; `top_first[0]` is drawn first.
    pub fn set_draw(&mut self, who: usize, top_first: &[&str]) {
        self.m.world_mut().hidden[who].draw =
            top_first.iter().rev().map(|s| s.to_string()).collect();
    }

    pub fn set_discard(&mut self, who: usize, cards: &[&str]) {
        self.m.world_mut().hidden[who].discard = cards.iter().map(|s| s.to_string()).collect();
    }

    pub fn set_state(&mut self, who: usize, key: &str, value: i32) {
        self.m.world_mut().st.players[who].state_set(key, value);
    }

    pub fn set_state_var(&mut self, who: usize, key: &str, v: StateVar) {
        self.m.world_mut().st.players[who]
            .state
            .insert(key.into(), v);
    }

    /// Charge pots: `value` held, `max` cap. Arranges the held-token row.
    pub fn set_charge(&mut self, who: usize, value: i32, max: i32) {
        self.m
            .world_mut()
            .set_tok(who as i32, K_CHARGE, value, -1, max);
    }

    /// Load dice: the next dice rolled anywhere (engine or card) show these
    /// faces in order, each clamped to its die.
    pub fn dice(&mut self, faces: &[i32]) {
        self.m.world_mut().rng.load_dice(faces);
    }

    /// Loaded faces not rolled yet.
    pub fn dice_left(&self) -> usize {
        self.m.world().rng.loaded_dice()
    }

    /// Put `card` on `who`'s field directly (no play, no hooks) -- for
    /// arranging an interaction. Returns the field uid. The card rule's
    /// declared properties ride the instance, as in a real placement.
    pub fn place_raw(&mut self, who: usize, card: &str) -> i32 {
        let d = data();
        let props = rules().card_props(card);
        self.m
            .world_mut()
            .place_card(&d, who as i32, card, Msg::default(), props)
    }

    // ------------------------------------------------------- event deck

    /// Replace the event deck; `top_first[0]` is drawn first.
    ///
    /// The engine's live deck is `World::event_deck`, a `Vec` whose **end** is
    /// the top (`draw_event` pops from the end). There is no other event-deck
    /// seam in this harness -- `MatchState::event_top` is only the public view
    /// of face-down cards pushed on top -- so this is the test setter for it.
    /// Ids are the roster's bare event ids (`"对邦"`), not the `event:*` rule ids.
    pub fn set_event_deck(&mut self, top_first: &[&str]) {
        self.m.world_mut().event_deck = top_first.iter().rev().map(|s| s.to_string()).collect();
    }

    /// The live event deck, top first.
    pub fn event_deck(&self) -> Vec<String> {
        self.m.world().event_deck.iter().rev().cloned().collect()
    }

    /// The event discard pile (`World::event_discard`).
    pub fn event_discard(&self) -> Vec<String> {
        self.m.world().event_discard.clone()
    }

    /// Events permanently removed from the game (`World::event_removed`).
    pub fn event_removed(&self) -> Vec<String> {
        self.m.world().event_removed.clone()
    }

    /// The `ActiveEvent` row for `id` (either face), if it is in play.
    pub fn active_event(&self, id: &str) -> Option<ActiveEvent> {
        self.m
            .world()
            .st
            .event_active
            .iter()
            .find(|e| e.id == id)
            .cloned()
    }

    /// Is `id` on the active list (「将此卡放置于场地中央」)?
    pub fn event_in_play(&self, id: &str) -> bool {
        self.active_event(id).is_some()
    }

    /// Named bonus counter count on the event's board-owner rule instance (the public
    /// `ActiveEvent::counter` mirrors it).
    pub fn event_counters(&self, id: &str) -> Option<i32> {
        self.m
            .world()
            .st
            .board_field
            .iter()
            .find(|f| f.card == format!("event:{id}"))
            .map(|f| f.counters.values().sum::<i32>())
    }

    // ------------------------------------------------------------ observe

    pub fn st(&self) -> MatchState {
        self.m.state()
    }

    pub fn p(&self, who: usize) -> MatchPlayer {
        self.m.world().st.players[who].clone()
    }

    pub fn money(&self, who: usize) -> i32 {
        self.m.world().st.players[who].money
    }

    pub fn pos(&self, who: usize) -> usize {
        self.m.world().st.players[who].pos as usize
    }

    pub fn turn(&self) -> usize {
        self.m.world().st.turn as usize
    }

    pub fn step(&self) -> i32 {
        self.m.world().st.step
    }

    pub fn round(&self) -> i32 {
        self.m.world().st.round
    }

    pub fn owner(&self, tile: usize) -> Option<usize> {
        let o = self.m.world().st.owners[tile];
        (o >= 0).then_some(o as usize)
    }

    pub fn houses(&self, tile: usize) -> i32 {
        self.m.world().st.houses[tile]
    }

    pub fn mortgaged(&self, tile: usize) -> bool {
        self.m.world().st.mortgaged[tile]
    }

    pub fn hand(&self, who: usize) -> Vec<String> {
        self.m.world().hidden[who].hand.clone()
    }

    /// The draw pile, top first.
    pub fn draw_pile(&self, who: usize) -> Vec<String> {
        self.m.world().hidden[who]
            .draw
            .iter()
            .rev()
            .cloned()
            .collect()
    }

    pub fn discard(&self, who: usize) -> Vec<String> {
        self.m.world().hidden[who].discard.clone()
    }

    pub fn field(&self, who: usize) -> Vec<FieldCard> {
        self.m.world().st.players[who].field.clone()
    }

    pub fn field_ids(&self, who: usize) -> Vec<String> {
        self.field(who).into_iter().map(|f| f.card).collect()
    }

    pub fn on_field(&self, who: usize, card: &str) -> bool {
        self.field(who).iter().any(|f| f.card == card)
    }

    /// Named bonus counters on `who`'s first field copy of `card` (None if not placed).
    pub fn counters(&self, who: usize, card: &str) -> Option<i32> {
        self.field(who)
            .iter()
            .find(|f| f.card == card)
            .map(|f| f.counters.values().sum::<i32>())
    }

    /// Put `n` [named counters] on `who`'s first field copy of `card`. No-op when the
    /// card is not placed. This is where group-card named counters go now -- the group
    /// skill's own instance, not a keyed state.
    pub fn set_counters(&mut self, who: usize, card: &str, n: i32) {
        let w = self.m.world_mut();
        if let Some(f) = w.st.players[who].field.iter_mut().find(|f| f.card == card) {
            f.counters.insert(bandori_std::counter::CRYSTALS.into(), n);
        }
    }

    /// On-card [tile marks] on `who`'s first field copy of `card` (None if not
    /// placed) -- the on-card mark counter, 「自己fieldN个[tile marks]」 (user
    /// ruling 2026-10-07: the mark points attached to the card). This is the number the
    /// view's field-card counter badge shows.
    pub fn marks_on_card(&self, who: usize, card: &str) -> Option<i32> {
        self.field(who).iter().find(|f| f.card == card).map(|f| {
            f.counters
                .get(bandori_std::counter::CP)
                .copied()
                .unwrap_or(0)
        })
    }

    /// Put `n` on-card [tile marks] on `who`'s first field copy of `card` (test seam,
    /// the same shape as [`Self::set_counters`]). No-op when the card is not
    /// placed. Does not raise `cpChanged` -- that rides a real write through
    /// `ctx::add_cp` / `add_cp_at`; use this only to arrange a starting count.
    pub fn set_marks_on_card(&mut self, who: usize, card: &str, n: i32) {
        let w = self.m.world_mut();
        if let Some(f) = w.st.players[who].field.iter_mut().find(|f| f.card == card) {
            f.counters.insert(bandori_std::counter::CP.into(), n);
        }
    }

    /// `who`'s bound skills (`skill:<owner>:<skill>`).
    pub fn skills(&self, who: usize) -> Vec<String> {
        self.field_ids(who)
            .into_iter()
            .filter(|c| c.starts_with("skill:"))
            .collect()
    }

    /// The skill on `who`'s field whose id contains `needle`.
    ///
    /// Group skills win over persona skills when both match: a needle like
    /// `"CRYCHIC"` names the group (`skill:CRYCHIC:美好的往日幻影`) but also
    /// appears inside 若叶睦（CRYCHIC）'s persona-skill id, and the group one is
    /// what a group-skill clause means. Unique needles (a skill's own name)
    /// resolve the same either way.
    pub fn skill_id(&self, who: usize, needle: &str) -> String {
        let mut hits: Vec<(bool, String)> = self
            .field(who)
            .into_iter()
            .filter(|f| f.card.starts_with("skill:") && f.card.contains(needle))
            .map(|f| (f.group_skill, f.card))
            .collect();
        // Group skills first (stable within each group).
        hits.sort_by_key(|(is_group, _)| !is_group);
        hits.into_iter()
            .next()
            .map(|(_, c)| c)
            .unwrap_or_else(|| panic!("no skill matching {needle:?}: {:?}", self.skills(who)))
    }

    pub fn state(&self, who: usize, key: &str) -> i32 {
        self.m.world().st.players[who].state_get(key)
    }

    pub fn state_var(&self, who: usize, key: &str) -> StateVar {
        self.m.world().st.players[who].state_var(key)
    }

    pub fn charge(&self, who: usize) -> i32 {
        // Fire is a held token of the persona-skill instance.
        self.token(who, K_CHARGE)
    }

    pub fn token(&self, who: usize, name: &str) -> i32 {
        self.m.world().st.players[who].token(name)
    }

    pub fn marks(&self) -> Vec<TileMark> {
        self.m.world().st.marks.clone()
    }

    pub fn marks_on(&self, tile: usize) -> Vec<TileMark> {
        self.marks()
            .into_iter()
            .filter(|m| m.tile == tile as i32)
            .collect()
    }

    /// The id of the newest event; pass it to [`Table::events_since`].
    pub fn mark(&self) -> i32 {
        self.m.world().recent.back().map_or(0, |e| e.id)
    }

    pub fn events_since(&self, mark: i32) -> Vec<MatchEvent> {
        self.m.events_since(mark)
    }

    pub fn keys_since(&self, mark: i32) -> Vec<String> {
        self.events_since(mark)
            .into_iter()
            .map(|e| log_key(&e))
            .collect()
    }

    /// The last `n` event keys (for assertion messages).
    pub fn recent_keys(&self, n: usize) -> Vec<String> {
        let all: Vec<String> = self.m.world().recent.iter().map(|e| log_key(e)).collect();
        all[all.len().saturating_sub(n)..].to_vec()
    }
}

/// The key a log event speaks as: its own, or the `what` inside an in-play
/// card's attribution (「<卡名> 的效果：<what>」, `log.card_effect`). A
/// flash-only `"card"` event (an activation with no line of its own) speaks as
/// `""`.
pub fn log_key(e: &MatchEvent) -> String {
    let m = if e.msg.key() == "log.card_effect" {
        match e.msg.a.get("what") {
            Some(Arg::Msg(m)) => m.as_ref(),
            _ => &e.msg,
        }
    } else {
        &e.msg
    };
    m.key().to_string()
}

/// An integer argument of a message (`.i` / `.n`).
pub fn int_arg(msg: &Msg, name: &str) -> Option<i64> {
    match msg.a.get(name) {
        Some(Arg::I(v)) | Some(Arg::N(v)) => Some(*v),
        _ => None,
    }
}

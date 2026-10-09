import { strict as assert } from "node:assert";
import { test } from "node:test";
import { executeCommand, parseCommand, type ConsoleContext, type ConsoleSession } from "./commands.ts";
import type { Command, MatchView } from "../core/types.ts";

function harness(kind: ConsoleSession["kind"] = "solo", cheats = true) {
  const sent: Command[] = [];
  const sess: ConsoleSession = {
    kind, id: "test", readOnly: kind === "replay", autoMode: "off",
    view: { playerId: 0, hand: ["c1"], state: {
      phase: "play", round: 2, turn: 1, step: 2, seq: 12, busy: false, debugOpen: false,
      players: [
        { player: "A", character: "Kasumi", money: 10_000, pos: 0, hand: 1, bankrupt: false, left: false },
        { player: "B", character: "Yukina", money: 9_000, pos: 10, hand: 2, bankrupt: false, left: true },
      ], prompt: { id: 0 },
    } } as MatchView,
    act: async (cmd) => { sent.push(cmd); return null; },
    setAutoMode: (mode) => { sess.autoMode = mode; },
  };
  const ctx: ConsoleContext = {
    session: () => sess, cards: () => [
      { id: "c1", name: "Card One", band: "B" }, { id: "c2", name: "Card One", band: "B" },
      { id: "c3", name: "Unique Card", band: "Roselia" },
    ],
    tiles: () => Array.from({ length: 40 }, (_, index) => ({ index, name: "T", kind: "property" })),
    profile: () => ({}), engine: () => ({}), cheatsEnabled: () => cheats,
    clear: () => {}, t: (key) => key, errorMessage: String,
  };
  return { sent, sess, ctx, run: (line: string) => executeCommand(line, ctx) };
}

test("quoted arguments and raw command JSON retain their contents", () => {
  assert.deepEqual(parseCommand('GIVE "Card One" 2 1'), { name: "give", args: ["Card One", "2", "1"] });
  assert.deepEqual(parseCommand("cards 'A\\'s card'"), { name: "cards", args: ["A's card"] });
  assert.deepEqual(parseCommand('act {"act":"play", "card":"A B"}'), { name: "act", args: ['{"act":"play", "card":"A B"}'] });
  assert.throws(() => parseCommand('give "unclosed'), /unclosedQuote/);
});

test("cheats select explicit seats and defaults, with validated card IDs", async () => {
  const h = harness();
  await h.run("money 50000");
  await h.run("tp 10 1");
  await h.run("give c1 2 1");
  await h.run("draw 2");
  await h.run("state fire 5");
  assert.equal(h.sent[0].target, 0);
  assert.equal(h.sent[1].target, 1);
  assert.equal(h.sent[2].card, "c1");
  assert.equal(h.sent[2].value, 2);
  assert.equal(h.sent[3].debug, "draw");
  assert.equal(h.sent[4].character, "fire");
});

test("malformed inputs never dispatch an action", async () => {
  const h = harness();
  for (const line of ["money 1.5", "money -1", "money 100000001", "tp 40", "tp 1 -1", "draw 0", "draw 101", 'give "Card One"', "money 1 0 extra", "act []", 'act {"act":"roll","value":null}', 'act {"act":"roll","cards":[3]}', "act {bad}", "unknown"]) {
    await assert.rejects(h.run(line));
  }
  assert.equal(h.sent.length, 0);
});

test("online cheats dispatch in debug builds; ordinary online actions work", async () => {
  const h = harness("online");
  await h.run("money 1");
  await h.run('act {"act":"debug","debug":"money","value":1}');
  await h.run('act {"act":"roll"}');
  assert.deepEqual(h.sent, [
    { act: "debug", debug: "money", value: 1, target: 0 },
    { act: "debug", debug: "money", value: 1 },
    { act: "roll" },
  ]);
});

test("replays and autopilot block commands, and auto off restores manual control", async () => {
  const replay = harness("replay");
  await assert.rejects(replay.run('act {"act":"end"}'), /readOnly/);
  await assert.rejects(replay.run("auto bot"), /readOnly/);
  await replay.run("players");
  assert.equal(replay.sent.length, 0);
  const h = harness();
  await h.run("auto bot");
  await assert.rejects(h.run("money 1"), /autoActive/);
  await h.run("auto off");
  await h.run("money 1");
  assert.equal(h.sent.length, 1);
});

test("engine refusals and absent match are visible command errors", async () => {
  const h = harness();
  h.sess.act = async () => "pending prompt";
  await assert.rejects(h.run("draw 2"), /pending prompt/);
  h.ctx.session = () => null;
  await assert.rejects(h.run("players"), /noMatch/);
  await h.run("help");
  await h.run("status");
});

test("parsing handles empty arguments, escapes and multiline raw JSON", () => {
  assert.deepEqual(parseCommand(" \n\t "), { name: "", args: [] });
  assert.deepEqual(parseCommand('cards ""'), { name: "cards", args: [""] });
  assert.deepEqual(parseCommand("cards A\\ B"), { name: "cards", args: ["A B"] });
  assert.deepEqual(parseCommand('cards "A\\\\B"'), { name: "cards", args: ["A\\B"] });
  const json = '{\n"act": "play", "card": "A\\\\B"\n}';
  assert.deepEqual(parseCommand(`ACT ${json}`), { name: "act", args: [json] });
  for (const line of ['cards "unfinished', "cards 'unfinished", "cards trailing\\"]) {
    assert.throws(() => parseCommand(line), /console.unclosedQuote/);
  }
});

test("cheats accept their boundaries and default to the viewing seat", async () => {
  const h = harness();
  h.sess.view!.playerId = 1;
  for (const line of ["money 0", "money 100000000", "tp 0", "tp 39", "draw 1", "draw 100",
    'give "Unique Card"', "give c1 100 0", "state fire -1000000", "state fire 1000000"]) {
    assert.equal(await h.run(line), "console.ok");
  }
  assert.deepEqual(h.sent, [
    { act: "debug", debug: "money", value: 0, target: 1 },
    { act: "debug", debug: "money", value: 100_000_000, target: 1 },
    { act: "debug", debug: "tp", value: 0, target: 1 },
    { act: "debug", debug: "tp", value: 39, target: 1 },
    { act: "debug", debug: "draw", value: 1, target: 1 },
    { act: "debug", debug: "draw", value: 100, target: 1 },
    { act: "debug", debug: "give", card: "c3", value: 1, target: 1 },
    { act: "debug", debug: "give", card: "c1", value: 100, target: 0 },
    { act: "debug", debug: "state", character: "fire", value: -1_000_000, target: 1 },
    { act: "debug", debug: "state", character: "fire", value: 1_000_000, target: 1 },
  ]);
});

test("an exact card ID takes precedence over another card's matching name", async () => {
  const h = harness();
  h.ctx.cards = () => [
    { id: "c1", name: "First", band: "B" }, { id: "c2", name: "c1", band: "B" },
  ];
  await h.run("give c1");
  assert.equal(h.sent[0].card, "c1");
});

test("missing, extra and out-of-range cheat arguments never dispatch", async () => {
  const h = harness();
  for (const line of ["money", "tp", "draw", "give", "state fire", "money +1", "money 1e2",
    "money 9007199254740992", "tp 1 2", "draw 1 0 extra", "give missing", "give c1 0", "give c1 101",
    "give c1 1 2", "give c1 1 0 extra", "state fire -1000001", "state fire 1000001",
    "state fire 1.5", "state fire 1 2", "state fire 1 0 extra"]) {
    await assert.rejects(h.run(line), undefined, line);
  }
  assert.deepEqual(h.sent, []);
});

test("raw actions validate every protocol field before dispatching", async () => {
  const h = harness();
  const invalid: unknown[] = [null, [], "roll", 1, {}, { act: "" }, { act: 1 }];
  for (const field of ["value", "target", "prompt"]) {
    for (const value of [null, "1", 0.5, -2_147_483_649, 2_147_483_648]) invalid.push({ act: "roll", [field]: value });
  }
  for (const field of ["character", "card", "debug"]) {
    for (const value of [null, 1, []]) invalid.push({ act: "roll", [field]: value });
  }
  for (const cards of [null, "c1", [1], ["c1", null]]) invalid.push({ act: "deck", cards });
  await assert.rejects(h.run("act"), /console.invalidJson/);
  for (const cmd of invalid) {
    await assert.rejects(h.run(`act ${JSON.stringify(cmd)}`), /console.invalidJson/);
  }
  assert.deepEqual(h.sent, []);
  const cmd = { act: "answer", value: -2_147_483_648, target: -1, prompt: 2_147_483_647,
    character: "A", card: "Card\\Name", debug: "", cards: ["c1", "c2"] };
  await h.run(`act ${JSON.stringify(cmd)}`);
  assert.deepEqual(h.sent, [cmd]);
});

test("inspection reports the visible match and works during replay and autopilot", async () => {
  const h = harness("replay");
  h.sess.autoMode = "bot";
  h.sess.view!.playerId = 1;
  const state = h.sess.view!.state;
  assert.deepEqual(await h.run("status"), { session: "replay", id: "test", auto: "bot", phase: "play",
    round: 2, turn: 1, step: 2, seq: 12, busy: false, prompt: 0, cheated: false });
  assert.deepEqual(await h.run("players"), [
    { seat: 0, name: "A", character: "Kasumi", money: 10_000, pos: 0, hand: 1, out: false },
    { seat: 1, name: "B", character: "Yukina", money: 9_000, pos: 10, hand: 2, out: true },
  ]);
  assert.deepEqual(await h.run("hand"), [{ index: 0, id: "c1", name: "Card One" }]);
  assert.equal(await h.run("inspect"), state);
  assert.equal(await h.run("inspect state"), state);
  assert.equal(await h.run("inspect prompt"), state.prompt);
  assert.equal(await h.run("inspect player"), state.players[1]);
  assert.equal(await h.run("inspect player 0"), state.players[0]);
  assert.deepEqual(h.sent, []);
});

test("data search is case-insensitive, bounded and independent of a match", async () => {
  const h = harness();
  h.ctx.session = () => null;
  h.ctx.cards = () => Array.from({ length: 150 }, (_, index) => ({
    id: `c${index}`, name: `Card ${index}`, band: index % 2 ? "Roselia" : "Poppin'Party",
  }));
  const all = await h.run("cards") as { total: number; cards: unknown[] };
  assert.equal(all.total, 150);
  assert.equal(all.cards.length, 100);
  const band = await h.run("cards ROSELIA") as { total: number };
  assert.equal(band.total, 75);
  const expected = { total: 1, cards: [{ id: "c149", name: "Card 149" }] };
  assert.deepEqual(await h.run("cards C149"), expected);
  assert.deepEqual(await h.run('cards "CARD 149"'), expected);
  assert.deepEqual(await h.run("cards missing"), { total: 0, cards: [] });
  assert.deepEqual(await h.run("tiles"), h.ctx.tiles());
});

test("local commands work without a match and missing views fail clearly", async () => {
  const h = harness();
  let clears = 0;
  const profile = { name: "Test" }, engine = { version: "test" };
  h.ctx.clear = () => { clears++; };
  h.ctx.profile = () => profile;
  h.ctx.engine = () => engine;
  h.ctx.session = () => null;
  assert.equal(await h.run(" \n "), undefined);
  assert.equal(await h.run("help"), "console.help");
  assert.equal(await h.run("clear"), undefined);
  assert.equal(clears, 1);
  assert.equal(await h.run("inspect profile"), profile);
  assert.equal(await h.run("inspect engine"), engine);
  assert.equal((await h.run("status") as { session: unknown }).session, null);
  await assert.rejects(h.run("auto off"), /console.noMatch/);
  h.ctx.session = () => h.sess;
  h.sess.view = null;
  for (const line of ["players", "hand", "inspect", "inspect prompt", "inspect player", "money 1"]) {
    await assert.rejects(h.run(line), /console.noView/, line);
  }
  assert.deepEqual(h.sent, []);
});

test("read-only solo sessions reject all writes while autopilot modes can be changed", async () => {
  const h = harness();
  h.sess.readOnly = true;
  for (const line of ["money 1", "tp 1", "give c1", "draw 1", "state fire 1", 'act {"act":"roll"}', "auto off"]) {
    await assert.rejects(h.run(line), /console.readOnly/, line);
  }
  assert.deepEqual(h.sent, []);
  h.sess.readOnly = false;
  for (const mode of ["bot", "chaos", "advanced"]) {
    assert.equal(await h.run(`auto ${mode}`), "console.ok");
    assert.equal(h.sess.autoMode, mode);
    await assert.rejects(h.run('act {"act":"roll"}'), /console.autoActive/);
  }
  await assert.rejects(h.run("auto unknown"), /console.usage/);
  assert.equal(h.sess.autoMode, "advanced");
  await h.run("auto off");
  await h.run('act {"act":"roll"}');
  assert.deepEqual(h.sent, [{ act: "roll" }]);
});

test("command errors use translations and propagate formatted engine errors", async () => {
  const h = harness();
  h.ctx.t = (key) => `translated:${key}`;
  await assert.rejects(h.run('cards "unfinished'), /translated:console.unclosedQuote/);
  await assert.rejects(h.run("money 1.5"), /translated:console.integer/);
  await assert.rejects(h.run("money -1"), /translated:console.range/);
  const refusal = { k: "err.debug_busy" };
  h.ctx.errorMessage = (error) => {
    assert.equal(error, refusal);
    return "Cannot cheat during a prompt";
  };
  h.sess.act = async () => refusal;
  await assert.rejects(h.run("draw 2"), /Cannot cheat during a prompt/);
  h.sess.act = async () => { throw new Error("connection lost"); };
  await assert.rejects(h.run('act {"act":"roll"}'), /connection lost/);
});

test("an engine built without cheats refuses the cheat commands and says so in help", async () => {
  for (const kind of ["solo", "online"] as const) {
    const h = harness(kind, false);
    for (const line of ["money 1", "tp 1 0", "give c1", "draw 1", "state fire 1",
      'act {"act":"debug","debug":"money","value":1}']) {
      await assert.rejects(h.run(line), /console\.cheatsDisabled/, `${kind}: ${line}`);
    }
    assert.deepEqual(h.sent, []);
    assert.match(String(await h.run("help")), /console\.cheatsDisabled/);
    await h.run('act {"act":"roll"}');
    assert.deepEqual(h.sent, [{ act: "roll" }]);
  }
});

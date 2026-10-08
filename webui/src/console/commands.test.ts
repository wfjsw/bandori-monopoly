import { strict as assert } from "node:assert";
import { test } from "node:test";
import { executeCommand, parseCommand, type ConsoleContext, type ConsoleSession } from "./commands.ts";
import type { Command, MatchView } from "../core/types.ts";

function harness(kind: ConsoleSession["kind"] = "solo") {
  const sent: Command[] = [];
  const sess: ConsoleSession = {
    kind, id: "test", readOnly: kind === "replay", autoMode: "off",
    view: { playerId: 0, hand: ["c1"], state: { players: [{ player: "A" }, { player: "B" }], prompt: { id: 0 } } } as MatchView,
    act: async (cmd) => { sent.push(cmd); return null; },
    setAutoMode: (mode) => { sess.autoMode = mode; },
  };
  const ctx: ConsoleContext = {
    session: () => sess, cards: () => [{ id: "c1", name: "Card One", band: "B" }, { id: "c2", name: "Card One", band: "B" }],
    tiles: () => Array.from({ length: 40 }, (_, index) => ({ index, name: "T", kind: "property" })),
    profile: () => ({}), engine: () => ({}), clear: () => {}, t: (key) => key, errorMessage: String,
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

test("online cheats and raw debug JSON are blocked; ordinary online actions work", async () => {
  const h = harness("online");
  await assert.rejects(h.run("money 1"), /soloOnly/);
  await assert.rejects(h.run('act {"act":"debug","debug":"money","value":1}'), /soloOnly/);
  await h.run('act {"act":"roll"}');
  assert.deepEqual(h.sent, [{ act: "roll" }]);
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

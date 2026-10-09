// The solo engine worker protocol (`soloProtocol.ts` / `soloEngine.ts` /
// `soloWorker.ts`). Run with:
//   node --test webui/src/game/soloEngine.test.ts
//
// Message-level tests against a fake worker (the shape `botPool.test.ts` uses
// for the pool's `workerFactory`), plus the fixed-step tick accumulator the
// worker runs every 50 ms -- the one piece of match timing that must keep the
// pre-worker semantics exactly.

import { strict as assert } from "node:assert";
import { test } from "node:test";

import { SoloEngine } from "./soloEngine.ts";
import { tickQuanta, TICK_STEP, type SaveSnap, type SoloPush, type SoloWorkerLike } from "./soloProtocol.ts";
import type { Msg } from "../i18n/msg.ts";

// ---------------------------------------------------------------- fake worker

/** An in-process worker: `postMessage` runs the handler, which answers through
 *  `onmessage` exactly like `soloWorker.ts` does (pushes first, then the reply). */
class FakeWorker implements SoloWorkerLike {
  onmessage: ((e: { data: unknown }) => void) | null = null;
  onerror: ((e: unknown) => void) | null = null;
  sent: Record<string, unknown>[] = [];
  terminated = false;
  private readonly handle: (msg: Record<string, unknown>, w: FakeWorker) => void;

  constructor(handle: (msg: Record<string, unknown>, w: FakeWorker) => void) {
    this.handle = handle;
  }

  postMessage(msg: unknown): void {
    const m = msg as Record<string, unknown>;
    this.sent.push(m);
    this.handle(m, this);
  }

  terminate(): void {
    this.terminated = true;
  }

  reply(id: number, value?: unknown): void {
    this.onmessage?.({ data: { id, ok: true, value } });
  }

  fail(id: number, error: string): void {
    this.onmessage?.({ data: { id, ok: false, error } });
  }

  push(p: SoloPush): void {
    this.onmessage?.({ data: p });
  }

  lastOp(): string {
    return String(this.sent[this.sent.length - 1]?.op ?? "");
  }
}

function open(opts: { handle: (msg: Record<string, unknown>, w: FakeWorker) => void }): { engine: SoloEngine; w: FakeWorker } {
  let w: FakeWorker | null = null;
  const engine = new SoloEngine({
    workerFactory: () => {
      w = new FakeWorker(opts.handle);
      return w;
    },
  });
  return { engine, w: w! };
}

const ok = (msg: Record<string, unknown>, w: FakeWorker) => w.reply(msg.id as number, true);
const boot = { stamp: { format: 1, bundle: "b1" }, cheats: true };

// ---------------------------------------------------------------- boot / open

test("boot sends init and resolves the worker's engine identity", async () => {
  const { engine, w } = open({
    handle: (m, fw) => (m.op === "init" ? fw.reply(m.id as number, boot) : ok(m, fw)),
  });
  assert.deepEqual(await engine.boot(), boot);
  assert.equal(w.sent.length, 1);
  assert.equal(w.sent[0].op, "init");
  assert.equal(typeof w.sent[0].id, "number");
});

test("start / restore carry the open spec the worker builds the match from", async () => {
  const specs: Record<string, unknown>[] = [];
  const { engine } = open({
    handle: (m, fw) => {
      specs.push(m);
      fw.reply(m.id as number, m.op === "init" ? boot : true);
    },
  });
  await engine.boot();
  await engine.start({
    members: [{ id: 1, player: "P", character: "美竹兰", cnId: "", ready: true, host: true, bot: false, away: false, mentality: "standard" }],
    mode: 0,
    weights: { exp: 1 },
    you: 1,
    seed256: "a".repeat(64),
  });
  await engine.restore({ save: '{"v":1}', rec: '{"log":[]}', last: 12, you: 1 });
  assert.equal(specs[1].op, "start");
  assert.equal(specs[1].seed256, "a".repeat(64));
  assert.equal(specs[1].you, 1);
  assert.equal((specs[1].members as unknown[]).length, 1);
  assert.equal(specs[2].op, "restore");
  assert.equal(specs[2].save, '{"v":1}');
  assert.equal(specs[2].rec, '{"log":[]}');
  assert.equal(specs[2].last, 12);
});

// ---------------------------------------------------------------- act / pushes

test("act resolves with the engine Msg and sees the post-act pushes first", async () => {
  const order: string[] = [];
  const err: Msg = { k: "err.no_money" };
  const view = { state: { seq: 2 }, playerId: 0, you: 1 };
  const { engine } = open({
    handle: (m, fw) => {
      if (m.op === "init") return fw.reply(m.id as number, boot);
      if (m.op === "act") {
        // The worker pumps (sync + save) before it answers -- the ordering the
        // page relies on when it reads `session.view` after `act`.
        fw.push({ push: "sync", events: [{ id: 3, type: "x" }], view: view as never });
        fw.push({ push: "save", match: "M", rec: "R", last: 3 });
        return fw.reply(m.id as number, err);
      }
      ok(m, fw);
    },
  });
  engine.onPush = (p) => order.push(p.push);
  await engine.boot();
  const got = await engine.act(1, { act: "buy", value: 1 });
  assert.deepEqual(got, err);
  assert.deepEqual(order, ["sync", "save"], "pushes land before the reply resolves");
});

test("act resolves null on success; error replies reject", async () => {
  const { engine, w } = open({
    handle: (m, fw) => {
      if (m.op === "init") return fw.reply(m.id as number, boot);
      if (m.op === "act") return fw.reply(m.id as number, null);
      if (m.op === "view") return fw.fail(m.id as number, "no match open (start / restore first)");
      ok(m, fw);
    },
  });
  await engine.boot();
  assert.equal(await engine.act(1, { act: "end" }), null);
  await assert.rejects(() => engine.view(2), /no match open/);
  assert.equal(w.lastOp(), "view");
});

test("save pushes refresh the snapshot and save() asks for a fresh one", async () => {
  const pushes: SoloPush[] = [];
  const { engine } = open({
    handle: (m, fw) => {
      if (m.op === "init") return fw.reply(m.id as number, boot);
      if (m.op === "save") return fw.reply(m.id as number, { match: "M2", rec: "R2", last: 9 } satisfies SaveSnap);
      ok(m, fw);
    },
  });
  engine.onPush = (p) => pushes.push(p);
  await engine.boot();
  const w = (engine as unknown as { w: FakeWorker }).w;
  w.push({ push: "save", match: "M1", rec: "R1", last: 4 });
  assert.deepEqual(pushes, [{ push: "save", match: "M1", rec: "R1", last: 4 }]);
  assert.deepEqual(await engine.save(), { match: "M2", rec: "R2", last: 9 });
});

test("export returns the sealed record bytes", async () => {
  const bytes = new Uint8Array([1, 2, 3, 4]);
  const { engine } = open({
    handle: (m, fw) => {
      if (m.op === "init") return fw.reply(m.id as number, boot);
      if (m.op === "export") {
        assert.equal(m.created, "2026-10-08 12:00");
        return fw.reply(m.id as number, bytes);
      }
      ok(m, fw);
    },
  });
  await engine.boot();
  assert.deepEqual(await engine.export("2026-10-08 12:00"), bytes);
});

test("close terminates the worker and rejects anything still in flight", async () => {
  let held: Record<string, unknown> | null = null;
  const { engine, w } = open({
    handle: (m, fw) => {
      if (m.op === "init") return fw.reply(m.id as number, boot);
      if (m.op === "act") return void (held = m); // never answered
      ok(m, fw);
    },
  });
  await engine.boot();
  const inFlight = engine.act(1, { act: "end" });
  engine.close();
  await assert.rejects(() => inFlight, /closed/);
  assert.ok(w.terminated);
  await assert.rejects(() => engine.act(1, { act: "end" }), /closed/);
  void held;
});

test("visibility forwards the page's hidden state for the tick pacing", async () => {
  const seen: unknown[] = [];
  const { engine } = open({
    handle: (m, fw) => {
      if (m.op === "visibility") seen.push(m.hidden);
      fw.reply(m.id as number, m.op === "init" ? boot : true);
    },
  });
  await engine.boot();
  engine.setVisibility(true);
  engine.setVisibility(false);
  await Promise.resolve(); // the fire-and-forget rpcs settle
  assert.deepEqual(seen, [true, false]);
});

// ---------------------------------------------------------------- tick quanta

test("tickQuanta turns elapsed time into whole TICK_STEP quanta, capped at 10", () => {
  // 50 ms per wake (the worker's interval): one quantum a wake, nothing owed.
  let acc = 0;
  for (let i = 0; i < 5; i++) {
    const s = tickQuanta(50, acc);
    assert.equal(s.k, 1);
    acc = s.acc;
  }
  assert.ok(acc >= 0 && acc < TICK_STEP);
  // A 0.5 s+ wake (the hidden-tab throttle) advances exactly 10 quanta and
  // drops the rest -- the cap that keeps a long stall from teleporting a match.
  const big = tickQuanta(5000, 0);
  assert.equal(big.k, 10);
  assert.ok(Math.abs(big.acc - 0) < 1e-9, "the capped remainder is not carried: 5.0 - 10*0.05 is dropped with the cap");
  // The carried remainder only comes from sub-quantum time.
  const s1 = tickQuanta(30, 0.01);
  assert.equal(s1.k, 0);
  assert.ok(Math.abs(s1.acc - 0.04) < 1e-9);
  const s2 = tickQuanta(20, 0.04);
  assert.equal(s2.k, 1);
  assert.ok(Math.abs(s2.acc - 0.01) < 1e-9);
  // 0.5 s of wall time is 10 quanta exactly; just under still carries.
  const half = tickQuanta(500, 0);
  assert.equal(half.k, 10);
  assert.ok(Math.abs(half.acc) < 1e-9);
  const under = tickQuanta(499, 0);
  assert.equal(under.k, 9);
  assert.ok(Math.abs(under.acc - 0.049) < 1e-9);
});
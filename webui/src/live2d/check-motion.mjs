// Standalone validation of the `.mtn` player, the physics port and the
// RenderFrame semantics (run with `node`):
//
//   node src/live2d/check-motion.mjs
//
// The known-frame numbers for model 001's `idle.mtn` were read off the file by
// hand (see the comments next to each expectation) -- they are the reference,
// not a re-run of this implementation.

import { readFileSync } from "node:fs";
import {
  animQueue,
  animValues,
  createAnim,
  playReaction,
  renderFrame,
  tickAnim,
  ParamStore,
} from "./anim.ts";
import { createMotionEnt, fadeWeight, Live2DMotion, parseMotion } from "./motion.ts";
import { L2DPhysics, parsePhysics, PhysicsHair } from "./physics.ts";

let failures = 0;
let checks = 0;

function ok(name, cond, detail = "") {
  checks += 1;
  if (cond) {
    console.log(`  ok   ${name}`);
  } else {
    failures += 1;
    console.log(`  FAIL ${name}${detail ? ` -- ${detail}` : ""}`);
  }
}

function close(name, a, b, eps = 1e-6) {
  const bad = [];
  for (let i = 0; i < a.length; i++) {
    if (!Number.isFinite(a[i]) || Math.abs(a[i] - b[i]) > eps) {
      bad.push(`[${i}] ${a[i]} != ${b[i]}`);
    }
  }
  ok(name, bad.length === 0, bad.slice(0, 3).join(", "));
}

const asset = (p) => new URL(`../../public/assets/live2d/${p}`, import.meta.url);
const read = (p) => readFileSync(asset(p), "utf8");

const idleText = read("001/idle.mtn");
const physicsText = read("001/physics.json");
const model = JSON.parse(read("001/model.json"));

// --------------------------------------------------------------- parser

console.log("parse idle.mtn");

const idle = parseMotion(idleText);
ok("idle.mtn reports fps=30", idle.fps === 30, `fps=${idle.fps}`);
ok("idle.mtn has 180 samples per curve", idle.maxLength === 180, `maxLength=${idle.maxLength}`);
const angleX = idle.curves.find((c) => c.paramId === "PARAM_ANGLE_X");
const eyeL = idle.curves.find((c) => c.paramId === "PARAM_EYE_L_OPEN");
ok("PARAM_ANGLE_X is a 180-sample curve", angleX && angleX.values.length === 180, `${angleX && angleX.values.length}`);
ok("PARAM_EYE_L_OPEN is a 180-sample curve", eyeL && eyeL.values.length === 180, `${eyeL && eyeL.values.length}`);
// File header: $fadein=0 / $fadeout=1000 plus $fadeout:PARAM_*=500 overrides.
ok("idle global fades are 0 / 1000", idle.fadeInMsec === 0 && idle.fadeOutMsec === 1000, `${idle.fadeInMsec}/${idle.fadeOutMsec}`);
ok("idle loop duration is 6000ms", idle.loopDurationMSec === 6000, `${idle.loopDurationMSec}`);
ok("eye curves carry the 500ms fade-out override", eyeL.fadeOutMsec === 500 && eyeL.fadeInMsec === -1, `${eyeL.fadeInMsec}/${eyeL.fadeOutMsec}`);
// Values read off the file: PARAM_ANGLE_X starts 13,13.003,13.01,13.023,…
close("PARAM_ANGLE_X samples match the file", Array.from(angleX.values.slice(0, 5)), [13, 13.003, 13.01, 13.023, 13.041], 1e-9);
// PARAM_EYE_L_OPEN is 1 across the head and dips to 0 around frame 24-29.
close("PARAM_EYE_L_OPEN samples match the file", [eyeL.values[0], eyeL.values[23], eyeL.values[24], eyeL.values[29], eyeL.values[30], eyeL.values[31]], [1, 1, 0.74, 0, 0, 0.24], 1e-9);

// A single-value line is a constant curve; $-directives and comments parse.
{
  const m = parseMotion("# comment\n$fps=30\n$fadein=250\n$fadeout:PARAM_X=500\nPARAM_X=1\nPARAM_Y=0,1,2\nLAYOUT:X=0,1\nVISIBLE:PARTS_A=1,0\n");
  ok("constant curve keeps one sample", m.curves.find((c) => c.paramId === "PARAM_X").values.length === 1);
  ok("per-parameter fade override lands on the curve", m.curves.find((c) => c.paramId === "PARAM_X").fadeOutMsec === 500);
  ok("layout curves parse and stay layout", m.curves.find((c) => c.paramId === "X").motionType === 100);
  ok("VISIBLE curves keep their prefix id", m.curves.find((c) => c.paramId === "VISIBLE:PARTS_A").motionType === 1);
  ok("global fades parse", m.fadeInMsec === 250 && m.fadeOutMsec === 1000);
  ok("$fps accepts the value", m.fps === 30);
}

// ------------------------------------------------------- known frame

console.log("known frames of idle.mtn");

function sampleAt(now) {
  const store = new ParamStore(model.params);
  const motion = Live2DMotion.fromText(idleText);
  const ent = createMotionEnt(motion);
  ent.startTimeMSec = 0;
  ent.fadeInStartTimeMSec = 0;
  ent.endTimeMSec = -1; // no fade-out: isolate the sampling math
  motion.updateParam(store, now, ent);
  return store;
}

{
  // t = 1000ms -> i = 30, frac = 0 -> frame 30 exactly.
  // PARAM_ANGLE_X[30] = 14.76 (read off the file).
  const store = sampleAt(1000);
  close("PARAM_ANGLE_X at frame 30 is 14.76", [store.getParam("PARAM_ANGLE_X")], [14.76], 1e-9);
  // PARAM_EYE_L_OPEN[30] = 0 (the baked blink in the idle is closed here).
  close("PARAM_EYE_L_OPEN at frame 30 is 0", [store.getParam("PARAM_EYE_L_OPEN")], [0], 1e-9);
}
{
  // t = 1000 + 500/30 ms -> i = 30, frac = 0.5 -> lerp(frame 30, frame 31).
  // PARAM_ANGLE_X: 14.76 -> 14.86 => 14.81. PARAM_EYE_L_OPEN: 0 -> 0.24 => 0.12.
  const store = sampleAt(1000 + 500 / 30);
  close("PARAM_ANGLE_X at frame 30.5 is 14.81", [store.getParam("PARAM_ANGLE_X")], [14.81], 1e-6);
  close("PARAM_EYE_L_OPEN at frame 30.5 is 0.12", [store.getParam("PARAM_EYE_L_OPEN")], [0.12], 1e-6);
}
{
  // Past the end the index clamps to the last sample (179).
  const store = sampleAt(10_000);
  close("samples clamp to the last frame", [store.getParam("PARAM_ANGLE_X")], [angleX.values[179]], 1e-9);
}

// ---------------------------------------------- discontinuity guard

console.log("40% discontinuity guard");

function synth(text, params) {
  const store = new ParamStore(params);
  const motion = Live2DMotion.fromText(text);
  const ent = createMotionEnt(motion);
  ent.startTimeMSec = 0;
  ent.fadeInStartTimeMSec = 0;
  ent.endTimeMSec = -1;
  return { store, motion, ent };
}

{
  // range 0..10 -> threshold 4. A 0->10 jump is over it: hold frame 0.
  const { store, motion, ent } = synth("$fadein=0\n$fps=10\nPARAM_X=0,10\n", [
    { id: "PARAM_X", min: 0, max: 10, def: 0, repeat: false },
  ]);
  motion.updateParam(store, 50, ent); // i=0, frac=0.5
  close("jump above the threshold holds the sample", [store.getParam("PARAM_X")], [0], 1e-9);
}
{
  // A 0->3 jump is under the threshold: lerp to 1.5 at frac 0.5.
  const { store, motion, ent } = synth("$fadein=0\n$fps=10\nPARAM_X=0,3\n", [
    { id: "PARAM_X", min: 0, max: 10, def: 0, repeat: false },
  ]);
  motion.updateParam(store, 50, ent);
  close("jump below the threshold interpolates", [store.getParam("PARAM_X")], [1.5], 1e-9);
}
{
  // Exactly 0.4 * (max - min) still interpolates (the guard is strict >).
  const { store, motion, ent } = synth("$fadein=0\n$fps=10\nPARAM_X=0,4\n", [
    { id: "PARAM_X", min: 0, max: 10, def: 0, repeat: false },
  ]);
  motion.updateParam(store, 50, ent);
  close("jump at the threshold interpolates", [store.getParam("PARAM_X")], [2], 1e-9);
}

// ------------------------------------------------------------ fades

console.log("fade weights");

close("fadeWeight(0) = 0", [fadeWeight(0)], [0], 1e-12);
close("fadeWeight(1) = 1", [fadeWeight(1)], [1], 1e-12);
close("fadeWeight(0.5) = 0.5", [fadeWeight(0.5)], [0.5], 1e-12);
// The shipped runtime eases fades with 0.5 - 0.5*cos(t*pi), not a linear ramp.
close("fadeWeight(0.25) is the cosine ease", [fadeWeight(0.25)], [0.5 - 0.5 * Math.cos(0.25 * Math.PI)], 1e-12);
close("fadeWeight clamps outside [0,1]", [fadeWeight(-0.5), fadeWeight(2)], [0, 1], 1e-12);

{
  // Global fade-in over 1000ms, no fade-out (ent has no end time). The curve
  // is long enough that the motion is still running at every boundary.
  const long = `PARAM_X=${new Array(100).fill(1).join(",")}`;
  const { store, motion, ent } = synth(`$fadein=1000\n$fps=10\n${long}\n`, [
    { id: "PARAM_X", min: -10, max: 10, def: 0, repeat: false },
  ]);
  motion.updateParam(store, 0, ent);
  close("fade-in boundary: weight 0 at start", [store.getParam("PARAM_X")], [0], 1e-9);
  motion.updateParam(store, 500, ent);
  close("fade-in midpoint: half the sample", [store.getParam("PARAM_X")], [fadeWeight(0.5)], 1e-9);
  motion.updateParam(store, 1000, ent);
  close("fade-in boundary: weight 1 at fadeInMsec", [store.getParam("PARAM_X")], [1], 1e-9);
}
{
  // Fade-out runs against ent.endTimeMSec: (end - now) / fadeOutMsec, eased.
  // Each boundary gets a fresh store so the weights do not compound.
  const fresh = () => {
    const s = synth("$fadein=0\n$fadeout=1000\n$fps=10\nPARAM_X=1\n", [
      { id: "PARAM_X", min: -10, max: 10, def: 0, repeat: false },
    ]);
    s.ent.endTimeMSec = 2000;
    return s;
  };
  {
    const { store, motion, ent } = fresh();
    motion.updateParam(store, 1000, ent);
    close("fade-out boundary: weight 1 one fadeOut before the end", [store.getParam("PARAM_X")], [1], 1e-9);
  }
  {
    const { store, motion, ent } = fresh();
    motion.updateParam(store, 1500, ent);
    close("fade-out midpoint: half the sample", [store.getParam("PARAM_X")], [fadeWeight(0.5)], 1e-9);
  }
  {
    const { store, motion, ent } = fresh();
    motion.updateParam(store, 2000, ent);
    close("fade-out boundary: weight 0 at endTime", [store.getParam("PARAM_X")], [0], 1e-9);
  }
}
{
  // A per-parameter override replaces the global fades for that curve.
  const text = "$fadein=1000\n$fps=10\nPARAM_A=1\nPARAM_B=1\n$fadein:PARAM_B=250\n";
  const store = new ParamStore([
    { id: "PARAM_A", min: -10, max: 10, def: 0, repeat: false },
    { id: "PARAM_B", min: -10, max: 10, def: 0, repeat: false },
  ]);
  const motion = Live2DMotion.fromText(text);
  const ent = createMotionEnt(motion);
  ent.startTimeMSec = 0;
  ent.fadeInStartTimeMSec = 0;
  ent.endTimeMSec = -1;
  motion.updateParam(store, 250, ent);
  close(
    "per-parameter fade replaces the global one",
    [store.getParam("PARAM_A"), store.getParam("PARAM_B")],
    [fadeWeight(0.25), 1],
    1e-9,
  );
}

// ------------------------------------------------------------- loop

console.log("loop restart");

{
  // 2 frames at 10fps = 200ms per loop ($fps must be in (5, 121), like the
  // runtime's check -- $fps=1 would stay at the 30fps default).
  const text = "$fadein=0\n$fps=10\nPARAM_X=10,20\n";
  const { store, motion, ent } = synth(text, [{ id: "PARAM_X", min: -100, max: 100, def: 0, repeat: false }]);
  motion.setLoop(true);
  motion.updateParam(store, 150, ent);
  close("mid-loop reads the clamped next sample", [store.getParam("PARAM_X")], [20], 1e-9);
  ok("looping motion does not finish", ent.finished === false);
  motion.updateParam(store, 250, ent);
  ok("loop restarts the ent clock", ent.startTimeMSec === 250 && ent.fadeInStartTimeMSec === 250, `start=${ent.startTimeMSec}`);
  ok("looping motion still does not finish", ent.finished === false);
  motion.updateParam(store, 275, ent);
  close("after the restart the sample index wraps", [store.getParam("PARAM_X")], [12.5], 1e-9);
}
{
  const text = "$fadein=0\n$fps=10\nPARAM_X=10,20\n";
  const { store, motion, ent } = synth(text, [{ id: "PARAM_X", min: -100, max: 100, def: 0, repeat: false }]);
  motion.setLoop(false);
  motion.updateParam(store, 2500, ent);
  ok("non-looping motion finishes past maxLength", ent.finished === true);
  ok("duration is 200ms", motion.getDurationMSec() === 200 && motion.getLoopDurationMSec() === 200);
}

// ------------------------------------------------- RenderFrame semantics

console.log("frame semantics (idle restart, bake, blink, reactions)");

const rand = (() => {
  // Deterministic, never 0 or 1: blink scheduling + reaction choice.
  let s = 12345;
  return () => {
    s = (s * 1103515245 + 12345) % 2147483648;
    return (s + 1) / 2147483649;
  };
})();

{
  const state = createAnim({
    params: model.params,
    idle: parseMotion(idleText),
    reactions: [],
    physics: parsePhysics(physicsText),
    now: 0,
    rand,
  });
  // Drive to just past the idle's 6000ms duration.
  const dt = 1000 / 60;
  for (let i = 0; i < 370; i++) tickAnim(state, dt);
  const ents = animQueue(state);
  ok("idle restarts when the queue empties", ents.length === 1, `${ents.length} ents`);
  ok("the restarted idle has a fresh start time", ents.length === 1 && ents[0].startTimeMSec > 6000, `${ents.length && ents[0].startTimeMSec}`);
  ok("reaction flag stays false while idling", state.isPlayingReaction === false);
  ok("600+ frames of idle+physics keep the params finite", state.store.allFinite());

  // A known frame through the full RenderFrame path: the motion writes
  // PARAM_ANGLE_X = 14.76 at frame 30 of the restarted idle... but the idle
  // restarted at ~6033ms, so land exactly 1000ms after that start instead.
  const start = animQueue(state)[0].startTimeMSec;
  while (state.now < start + 1000) tickAnim(state, dt);
  const values = animValues(state);
  close("RenderFrame bakes PARAM_ANGLE_X = 14.76 at frame 30", [values.PARAM_ANGLE_X], [14.76], 1e-3);
  // The idle's own eye curve is closed at frame 30, but the game runs
  // L2DEyeBlink enabled and it overwrites the eye params after the bake.
  ok(
    "blink overrides the idle's baked eye curves",
    values.PARAM_EYE_L_OPEN === 1 && values.PARAM_EYE_R_OPEN === 1,
    `L=${values.PARAM_EYE_L_OPEN} R=${values.PARAM_EYE_R_OPEN}`,
  );
}

{
  const reaction = parseMotion("$fadein=250\n$fps=10\nPARAM_X=5\n");
  const state = createAnim({
    params: model.params,
    idle: parseMotion(idleText),
    reactions: [reaction, reaction, reaction],
    physics: null,
    now: 0,
    rand,
  });
  tickAnim(state, 16);
  ok("a click starts a reaction", playReaction(state) === true);
  ok("reactions do not stack", playReaction(state) === false);
  for (let i = 0; i < 30; i++) tickAnim(state, 16);
  ok("the reaction suppresses the idle bake", state.isPlayingReaction === true);
  ok("reaction frames keep params finite", state.store.allFinite());
  for (let i = 0; i < 200; i++) tickAnim(state, 16);
  ok("the reaction ends and the idle comes back", state.isPlayingReaction === false && animQueue(state).length === 1);
}

{
  // Real shipped reactions, not a synthetic one-sample curve: they run
  // 3-8 seconds and hold their last samples before the queue drops them.
  // (A synthetic 100ms motion hides "the reaction never finishes" bugs.)
  const realFiles = [
    ["001/smile01.mtn", 118],
    ["036/smile02.mtn", 135],
    ["036/kandou03.mtn", 240], // the longest reaction in the set: 8s
  ];
  for (const [file, wantMax] of realFiles) {
    const parsed = parseMotion(read(file));
    ok(
      `real ${file} parses to ${wantMax} samples`,
      parsed.maxLength === wantMax && parsed.fps === 30,
      `maxLength=${parsed.maxLength} fps=${parsed.fps}`,
    );
    ok(
      `real ${file} duration is 1000*maxLength/fps`,
      parsed.loopDurationMSec === Math.trunc((1000 * parsed.maxLength) / parsed.fps),
      `dur=${parsed.loopDurationMSec}`,
    );

    const state = createAnim({
      params: model.params,
      idle: parseMotion(idleText),
      reactions: [parseMotion(read(file))],
      physics: null,
      now: 0,
      rand,
    });
    // Stand still for a moment so the idle bakes a base, then click.
    for (let i = 0; i < 30; i++) tickAnim(state, 1000 / 60);
    ok(`real ${file} starts`, playReaction(state) === true);
    const before = animValues(state);
    // Step just past the reaction's own duration + its 350ms fade-out.
    const frames = Math.ceil((parsed.loopDurationMSec + 500) / (1000 / 60));
    for (let i = 0; i < frames; i++) tickAnim(state, 1000 / 60);
    ok(
      `real ${file} ends and the idle comes back`,
      state.isPlayingReaction === false && animQueue(state).length === 1,
      `reaction=${state.isPlayingReaction} ents=${animQueue(state).length}`,
    );
    ok(`real ${file} leaves the params finite`, state.store.allFinite());

    // The pose must actually move during the reaction -- a frozen pose means
    // the samples never reached the model.
    const state2 = createAnim({
      params: model.params,
      idle: parseMotion(idleText),
      reactions: [parseMotion(read(file))],
      physics: null,
      now: 0,
      rand,
    });
    for (let i = 0; i < 30; i++) tickAnim(state2, 1000 / 60);
    playReaction(state2);
    for (let i = 0; i < 30; i++) tickAnim(state2, 1000 / 60);
    const mid = animValues(state2);
    for (let i = 0; i < 60; i++) tickAnim(state2, 1000 / 60);
    const late = animValues(state2);
    const moved = Object.keys(mid).some((k) => Number.isFinite(mid[k]) && Math.abs((late[k] ?? 0) - mid[k]) > 1e-3);
    ok(`real ${file} pose moves while playing`, moved, `before ANGLE_X=${before.PARAM_ANGLE_X}`);
  }
}

{
  // The `params` prop is layered under the motion: an untouched parameter
  // keeps its value through the bake.
  const state = createAnim({
    params: model.params,
    idle: null,
    reactions: [],
    physics: null,
    now: 0,
    rand,
    props: { PARAM_TEAR: 0.5 },
  });
  renderFrame(state);
  close("prop params survive with no motion", [animValues(state).PARAM_TEAR], [0.5], 1e-9);
}

// ------------------------------------------------------------ physics

console.log("physics.json + spring stability");

const specs = parsePhysics(physicsText);
ok("physics.json parses the shipped hairs", specs.length >= 3, `${specs.length} hairs`);
ok(
  "setup/src/targets survive the parse",
  specs[0].setup.length === 0.2 && specs[0].src.length >= 2 && specs[0].targets.length >= 1,
  JSON.stringify(specs[0].setup),
);

{
  const store = new ParamStore(model.params);
  const phys = L2DPhysics.fromSpecs(specs, 0);
  let worst = 0;
  for (let i = 0; i < 600; i++) {
    const t = i * (1000 / 60);
    store.setParam("PARAM_BODY_ANGLE_X", 10 * Math.sin((2 * Math.PI * t) / 2000));
    store.setParam("PARAM_BODY_ANGLE_Z", 10 * Math.cos((2 * Math.PI * t) / 2000));
    phys.updateParam(store, t);
    for (const id of ["PARAM_CLOTHES_A", "PARAM_CLOTHES_B"]) {
      const v = store.getParam(id);
      if (!Number.isFinite(v)) worst = Infinity;
      else worst = Math.max(worst, Math.abs(v));
    }
  }
  ok("600 frames of physics stay finite", worst !== Infinity, `worst=${worst}`);
  // Source params swing ±10 through scales ~0.005-0.025: the driven params
  // must stay near that neighbourhood, not drift off to infinity.
  ok("physics outputs stay near the source params", worst < 1, `worst |out| = ${worst}`);
}

{
  // A single hair driven only by gravity must settle, not explode.
  const hair = PhysicsHair.fromSpec({
    setup: { length: 0.3, regist: 1, mass: 0.3 },
    src: [{ id: "PARAM_BODY_ANGLE_X", ptype: "x", scale: 0.01, weight: 1 }],
    targets: [{ id: "PARAM_CLOTHES_A", ptype: "angle", scale: 0.02, weight: 1 }],
  });
  const store = new ParamStore(model.params);
  for (let i = 0; i < 600; i++) hair.update(store, i * (1000 / 60));
  const v = store.getParam("PARAM_CLOTHES_A");
  ok("a resting hair settles near zero", Number.isFinite(v) && Math.abs(v) < 0.5, `angle param = ${v}`);
}

console.log(`check-motion: ${checks - failures}/${checks} passed`);
process.exit(failures ? 1 : 0);
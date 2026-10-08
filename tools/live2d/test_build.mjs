import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const scripts = dirname(fileURLToPath(import.meta.url));

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "bandori-live2d-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  for (const dir of ["tools/live2d", "live2d", "live2d-src/001", "webui/public/assets/live2d/001", "game/001"])
    mkdirSync(join(root, dir), { recursive: true });
  for (const script of ["build.mjs", "prepare.mjs"])
    copyFileSync(join(scripts, script), join(root, "tools/live2d", script));
  writeFileSync(join(root, "live2d/001.cxx3"), "existing archive");
  writeFileSync(join(root, "live2d-src/001/main.xml"), "existing extract");
  writeFileSync(join(root, "webui/public/assets/live2d/001/model.json"), '{"textures":[]}');
  writeFileSync(join(root, "game/001/model.moc"), "newer game source");
  const stamp = Date.now() / 1000;
  for (const [path, offset] of [["live2d/001.cxx3", -4], ["live2d-src/001/main.xml", -3],
                              ["webui/public/assets/live2d/001/model.json", -2], ["game/001/model.moc", -1]])
    utimesSync(join(root, path), stamp + offset, stamp + offset);
  return root;
}

function run(root, script, options = {}) {
  const env = { ...process.env };
  for (const name of ["FORCE", "CHECK", "QUADRISM", "PYTHON", "GAME_DATA_DIR", "GAME_LIVE2D"])
    delete env[name];
  return spawnSync(process.execPath, [join(root, "tools/live2d", script)], {
    cwd: root, encoding: "utf8", env: { ...env, GAME_LIVE2D: join(root, "game"), ...options },
  });
}

test("cached build ignores newer moc and does not invoke Quadrism or Python", (t) => {
  const root = fixture(t);
  const result = run(root, "build.mjs", { QUADRISM: join(root, "no-converter"), PYTHON: join(root, "no-python") });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(readFileSync(join(root, "live2d/001.cxx3"), "utf8"), "existing archive");
  assert.equal(readFileSync(join(root, "webui/public/assets/live2d/001/model.json"), "utf8"), '{"textures":[]}');
});

test("explicit preparation keeps existing archives despite newer sources", (t) => {
  const root = fixture(t);
  const result = run(root, "prepare.mjs");
  assert.equal(result.status, 0, result.stderr);
  assert.equal(readFileSync(join(root, "live2d/001.cxx3"), "utf8"), "existing archive");
});

test("normal build does not bootstrap a missing archive", (t) => {
  const root = fixture(t);
  rmSync(join(root, "live2d/001.cxx3"));
  const result = run(root, "build.mjs");
  assert.equal(result.status, 0, result.stderr);
  assert.equal(existsSync(join(root, "live2d/001.cxx3")), false);
});

test("explicit missing-archive preparation requires a converter", (t) => {
  const root = fixture(t);
  rmSync(join(root, "live2d/001.cxx3"));
  const result = run(root, "prepare.mjs");
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /set QUADRISM/);
  assert.equal(existsSync(join(root, "live2d/001.cxx3")), false);
});

test("failed explicit replacement preserves the old archive and cleans temporary files", (t) => {
  const root = fixture(t);
  const result = run(root, "prepare.mjs", { FORCE: "1", QUADRISM: join(root, "no-converter") });
  assert.notEqual(result.status, 0);
  assert.equal(readFileSync(join(root, "live2d/001.cxx3"), "utf8"), "existing archive");
  assert.deepEqual(readdirSync(join(root, "live2d")), ["001.cxx3"]);
});

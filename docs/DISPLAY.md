# Display export

Printed text and other display-only columns are **authored in the Rust rule
code** beside the rule bodies (data lives with the rule) but no longer ride
`ruleset_metadata`. They are serialized at build time into a generated JSON the
webui bundles.

```
rules/cards/**  +  rules/personas/**  +  rules/skills/**  +  rules/events/**
        │  CardMeta / PersonaDef / GroupDef (authoring surface)
rules/bandori-std/src/marker.rs
        │  MARKERS (card-face marker rows)
        ▼
cargo run -q -p rules-native --example dump_display
        ▼
webui/src/generated/display.json   (generated; gitignored)
        ▼
webui bundle
```

## Regenerate

```sh
node tools/export-display.mjs              # default out: webui/src/generated/display.json
node tools/export-display.mjs --out PATH   # a .json file, or a directory
```

This is wired into `webui`'s `predev` / `prebuild`, so `npm run dev` and
`npm run build` refresh it automatically. The wrapper runs
`cargo run -q -p rules-native --example dump_display` (no `CARGO_TARGET_DIR`).
Unlike `tools/roster-dump.mjs` there is no cache: the dump is rewritten on every
run so it always tracks rule-source edits.

Direct invocation:

```sh
cargo run -q -p rules-native --example dump_display                  # stdout
cargo run -q -p rules-native --example dump_display -- /tmp/display.json
```

## Shape

```json
{
  "cards":    { "<cardId>":      { "text": "...", "body": [ … ],
                                   "rawName": "...", "counterReason": "..." } },
  "personas": { "<personaName>": { "display": "...", "text": "...", "color": "...",
                                   "variantTag": "...", "art": "...", "costume": "...",
                                   "cnId": "..." } },
  "groups":   { "<groupName>":   { "text": "...", "shortName": "...", "color": "...",
                                   "logo": "..." } },
  "events":   { "<eventId>":     { "name": "...", "text": "...", "body": [ … ] } },
  "markers": [ { "key": "turnsLeft", "source": "prop", "tone": "turns",
                 "glyph": "⌛{n}", "countdown": true,
                 "note": { "key": "board.turnsLeft" },
                 "label": { "key": "board.turnsLabel" } }, … ]
}
```

* Keys are camelCase. Empty / absent fields are omitted.
* Event keys are the **bare** event id (`对邦`), not the `event:` rule-id
  prefix. `derived` is engine data and is not exported.
* Ordering is deterministic (sorted map keys, pretty JSON, one trailing
  newline) so the file is byte-stable across runs and machines.
* Cards without a `CardMeta` (rule-only pseudo cards: tiles / marks / status)
  are absent. Every roster card / persona / group / event that carries the
  relevant fields appears.
* `markers` is an **array**, not a map: its order is the face's print order.
  It is `bandori_std::marker::MARKERS` verbatim -- one row per marker kind,
  authored in the rule code (see 「Card-face counter markers」).

### Rule text: `text` + `body`

Card and event rule text is authored as a `CardText` -- sections and
bulletpoints as data (`crates/card-sdk/src/text.rs`). Two views land here:

* `text` -- the rulebook string form (`CardText::flat()`). Prose consumers and
  the rule queries (`card_text_mentions`, 「效果包含…」) substring-search this.
* `body` -- the structure, so the client renders sections / bulletpoints
  directly instead of re-deriving them from `text` with regex heuristics.

`body` is a flat block list in render order:

```json
"body": [
  { "t": "label", "text": "[手]：" },
  { "t": "p",     "text": "依次进行以下效果：" },
  { "t": "steps", "items": ["…；", "…。"] },
  { "t": "ol",    "items": [{ "text": "…", "sub": ["…"] }] }
]
```

`label` heads a section; `p` is free prose; `ol` items are the numbered
effects `（1）（2）…` (the marker is positional -- it is not in `text`, the list
renders its own numbers); `steps` is a standalone `1. 2. 3.` run (the
「依次进行以下操作：」 lists, also nested as `items[].sub`). Persona and group
skill text is still a flat `text` only -- the client parses it at render time.

### Field list

| kind | key | authored as | notes |
| --- | --- | --- | --- |
| card | `text` | `CardMeta::text` (`flat()`) | rule text, rulebook string form |
| card | `body` | `CardMeta::text` | rule text as sections / bulletpoints |
| card | `rawName` | `CardMeta::raw_name` | untrimmed sheet name |
| card | `counterReason` | `On` reason field (`Counteract` / `Action` / `Play`) | 「…时」 clause the counteraction prompt shows, as a **locale key** the client resolves (`cards:<crate>.…` / `counter.*`, ABI v95); several entries join their keys with `\n` |
| persona | `display` | `PersonaDef::display` | compact display label |
| persona | `text` | `PersonaDef::text` | skill text |
| persona | `color` | `PersonaDef::color` | accent colour |
| persona | `variantTag` | `PersonaDef::variant_tag` | short variant label |
| persona | `art` | `PersonaDef::art` | art id |
| persona | `costume` | `PersonaDef::costume` | costume id |
| persona | `cnId` | `PersonaDef::cn_id` | CN client id |
| group | `text` | `GroupDef::text` | band skill text |
| group | `shortName` | `GroupDef::short_name` | short name |
| group | `color` | `GroupDef::color` | accent colour |
| group | `logo` | `GroupDef::logo` | logo id |
| event | `name` | `CardMeta::name` | event name |
| event | `text` | `CardMeta::text` (`flat()`) | event rulebook body |
| event | `body` | `CardMeta::text` | event body as sections / bulletpoints |
| marker | `key` | `MarkerDef::key` | counter / prop name the row describes; absent on the fallback row |
| marker | `source` | `MarkerDef::source` | `prop` (a card prop) or `counter` (a named counter) |
| marker | `tone` | `MarkerDef::tone` | chip style class: `turns` / `count` / `crystal` / `cp` |
| marker | `glyph` | `MarkerDef::glyph` | face glyph, `{n}` = count, `{name}` = named counter |
| marker | `note` / `label` | `MarkerDef::note` / `label` | `{ "key": … }` or `{ "text": … }` |
| marker | `countdown` | `MarkerDef::countdown` | suppress when another marker carries the number |
| marker | `fallback` | `MarkerDef::fallback` | takes every counter no other row covers |

The sheet cell a row was lifted from is **not** a field any more -- it is a
`// sheet cell X` comment on the card impl. A row whose sheet name line is
blank carries that cell as its `name` instead (`#J11`), so it is never lost.

### Card-face counter markers

The field sheet renders an instance's markers as small chips in the face's
marker zone: `⌛N` remaining turns, `×N` unnamed counters, `◆N` crystals, `CPN`
counteraction points, then any named counters. A marker kind is one row of
`bandori_std::marker::MARKERS` (`rules/bandori-std/src/marker.rs`) -- its print
order (the array order), style tone, glyph (`{n}` = the count, `{name}` = a
named counter) and its note / list sentences (a `tr` key or a literal
template). A new marker is that row and nothing else -- the JSON export carries
it and the client knows no kinds; adding only a new **tone** also needs the
matching CSS class in `FieldSheet.module.css`. A surface renders the resulting
`Marker[]` and knows no kinds itself -- the face (`FieldSheet`), the live note,
the field / event popups, the player marker list and a skill card's corner
chips all draw the same plan.

The marker vocabulary -- the counter names (`bandori_std::counter::CP` /
`CRYSTALS`) and the countdown prop (`bandori_std::marker::TURNS_LEFT`) -- lives
with the table, not in `card-sdk`. The client's planner
(`scenes/board/markerKinds.ts`) walks the exported rows in order, reads each
row's count from `props[key]` / `counters[key]`, and hands every counter no row
covers to the fallback row with its `log.counter.*` phrase (an unlabelled
counter is dropped, never printed as its wire key).

`⌛N` comes from the display-only prop `props.turnsLeft` on a `FieldCard`, or
`ActiveEvent.turnsLeft` for an event row (`docs/EVENTS.md`); it appears only
while positive. It is a countdown hint, never the expiry itself. Where another
marker on the same face already carries that number -- a crystal count that is
itself the countdown, a phase counter -- the `⌛N` is **not** printed again
(`showTurnsChip`): the existing chip already shows how many turns are left. The
countdown still stays in the data, so the hiding is display-only.

Set a **fixed** initial count as a static card prop --
`CardDef::props(&[(marker::TURNS_LEFT, N)])`
-- so `place_card` / `bind_event` stamp it with everything else on the instance
(`R:Fire bird`'s self-chosen X and `Sumimi:Here, the World!`'s draw-armed value
are the exceptions: those stay in the body). The body's own turn-boundary tick
is `ctx::dec_turns_left()`, which clamps at 0. A rule that clears the crystals
early (`HHW:笑容大游行` (2) 「移除其上全部奇迹水晶」) clears the marker with
them (`ctx::set_turns_left(0)`).

## Split from the runtime roster

The display columns above are the authoring surface, and `display.json` is
what the **webui** reads. Runtime `ruleset_metadata` (`docs/CARDS.md`
「Card metadata」) carries the engine's roster instead:

| kind | stays in `ruleset_metadata` |
| --- | --- |
| card | `name`, `group`, `owner`, `derived`, `tags`, **`text`** |
| persona | `name`, `group`, `skill`, `exclusiveCards`, `preset`, `variantOf`, **`text`** |
| group | `name`, `skill`, `sharesExclusives`, **`text`** |
| event | `name`, `derived`, **`text`** |

**Rule-body `text` is dual-role and stays in both.** The ruleset queries it at
runtime (`card_text_mentions` -- 「效果包含[奇迹水晶]」, 「技能中含有火罐」), so it
is a rule input, not just display. `display.json` carries the same string so
the client never has to ask the engine for it. Everything else in the table
above is display-only and lives only in `display.json`.

Builder calls (`.text(...)` / `.display(...)` / `.color(...)` / `.art(...)` /
…) stay in the rule sources. `dump_display` reads the defs directly; only the
emission into `MetaRow` stops (ABI v86).
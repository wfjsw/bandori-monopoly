# Display export

Printed text and other display-only columns are **authored in the Rust rule
code** beside the rule bodies (data lives with the rule) but no longer ride
`ruleset_metadata`. They are serialized at build time into a generated JSON the
webui bundles.

```
rules/cards/**  +  rules/personas/**  +  rules/skills/**  +  rules/events/**
        │  CardMeta / PersonaDef / GroupDef (authoring surface)
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
  "cards":    { "<cardId>":      { "text": "...", "rawName": "...", "cell": "...",
                                   "counterReason": "..." } },
  "personas": { "<personaName>": { "display": "...", "text": "...", "color": "...",
                                   "variantTag": "...", "art": "...", "costume": "...",
                                   "cnId": "..." } },
  "groups":   { "<groupName>":   { "text": "...", "shortName": "...", "color": "...",
                                   "logo": "..." } },
  "events":   { "<eventId>":     { "name": "...", "text": "...", "cell": "..." } }
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

### Field list

| kind | key | authored as | notes |
| --- | --- | --- | --- |
| card | `text` | `CardMeta::text` | rule text |
| card | `rawName` | `CardMeta::raw_name` | untrimmed sheet name |
| card | `cell` | `CardMeta::cell` | sheet cell the row was lifted from |
| card | `counterReason` | `On` reason field (`Counteract` / `Action` / `Play`) | 「…时」 clause the counteraction prompt shows, as a **locale key** the client resolves (`cards:<crate>.…` / `counter.*`, ABI v94); several entries join their keys with `\n` |
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
| event | `text` | `CardMeta::text` | event rulebook body |
| event | `cell` | `CardMeta::cell` | sheet cell the row was lifted from |

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
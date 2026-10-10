# CSS hierarchy

The web UI is React 19 + rsbuild with CSS Modules (`*.module.css`) plus one global
sheet, `webui/src/styles/base.css`. The CSS grew through many feature passes; this
document is the target hierarchy and the rules the modules follow after the
`css-hierarchy` refactor. The refactor is visual-no-op: every number below is the
value the UI already used, only lifted into a named token.

## Layers

```
base.css (:root)          design tokens + reset + the scaled stage
  └─ styles/layout.ts     the same layout numbers for TypeScript
ui/primitives.module.css  shared primitives (composes: from component modules)
  └─ *.module.css         one component per file; tokens and primitives only
```

Component modules never import each other's class names or copy their magic
numbers. Anything shared is a token or a primitive.

## (a) Tokens — `base.css :root`

### Colour

| Token | Value | Use |
| --- | --- | --- |
| `--pink` family | `--pink`, `--pink-light`, `--pink-deep`, `--pink-pale` | brand accent |
| `--text` / `--sub` / `--faint` | grey text scale | body, secondary, disabled |
| `--surface` | `#fff` | cards, panels, peeks |
| `--panel` | `rgba(255,255,255,.93)` | floating panels over the board |
| `--backdrop` | `#f4e4ea` | page background |
| `--scrim` | `rgba(60,40,50,.38)` | modal dim |
| `--line` | `#ddd` | hairline borders |
| `--dark-pill` / `--light-pill` | pill greys | chips, tracks |
| accents | `--purple --blue --teal --gold --green --exp --note --shield --tag-red` | markers, kinds |
| `--danger` | `#d23a3a` | destructive / error text |
| `--ok` / `--ok-deep` | `#6c6` / `#3a3` | success |
| `--me-blue` | `#2f7df6` | "your seat" ring |
| `--money-pay` / `--money-optional` / `--money-receive` | `#9e2b1f` / `#a67c00` / `#0a5f7a` | money-direction discriminators (`TileQuote.flow`: must pay / may pay / receive) |
| `--ink` | `#4b2233` | text on coloured flashes |

Colour literals inside component modules are bugs; use the token (or
`color-mix` / `rgba` of one) instead.

### Radius, shadow, spacing, type

Radii `--r-sm 6px` … `--r-pill 999px`. Shadows `--shadow` (panel), `--float-shadow`
(peek card), `--pop-shadow` (modal window), `--lift`/`--lift-down` (pressed
button). Spacing `--sp-1 4px` … `--sp-8 32px`. Type `--fs-xs 11px` … `--fs-3xl 26px`.
Motion `--dur-fast .12s` / `--dur-mid .16s` / `--dur-slow .28s` and the three
easings `--ease-out`, `--ease-sheet` (the sheet / dock slide),
`--ease-snap` (link snap).

### Layout — the shared geometry

These are the numbers that used to be duplicated across `Board`, `Side`,
`FieldSheet`, `Modal` with "must match Board.module.css" comments. One place now.

| Token | Value | Meaning |
| --- | --- | --- |
| `--stage-w` / `--stage-h` | `1600px` / `900px` | design canvas (Stage.tsx) |
| `--topbar-h` | `110px` | TopBar height |
| `--board-inset-x` | `38px` | board body's side inset |
| `--board-col-left` | `350px` | players / actions column |
| `--board-col-right` | `300px` | stand + log column |
| `--board-gap` | `9px` | column gap |
| `--board-chrome-x` | `744px` | both insets + both columns + both gaps |
| `--board-mid-offset` | `25px` | centre column's centre − the stage's |
| `--sheet-strip` | `60px` | prompt sheet's retracted title strip |
| `--hand-peek` | `30px` | hand fan peek above the bottom edge |
| `--skill-peek` | `28px` | skill-stack header peek |
| `--skill-card-w` | `96px` | skill-stack card face width |
| `--skill-step` | `34px` | skill-stack dense horizontal step |
| `--skill-fan-gap` | `24px` | gap between the skill stack and the fan |
| `--map-margin-top` | `18px` | map window's top margin |

`--board-mid-offset` is width-invariant: the centre column's centre sits at
`stage/2 + 25px` on any stage width, because the side chrome is symmetric.
Modal's `left: calc(50% + 25px)` and `width: calc(100% - 744px)` are
`calc(50% + var(--board-mid-offset))` and `calc(100% - var(--board-chrome-x))`.

### Z-index scale

One scale, named layers. Component modules reference the name; a raw `z-index`
number in a module is a bug. Local 0/1 stacking inside a widget (a tile inside
the ring, a dialogue over a stand that owns a stacking context) is exempt.

| Token | Value | Layer |
| --- | --- | --- |
| `--z-lift` | `1` | inside-widget stacking (tile, dialogue over stand) |
| `--z-topbar` | `5` | TopBar |
| `--z-chrome` | `6` | loose scene chrome (select quick-filter) |
| `--z-field` | `15` | field sheet (cards-in-play row) |
| `--z-fx` | `20` | match FX base |
| `--z-fx-banner` | `20` | status banner |
| `--z-fx-phase` | `22` | phase flash |
| `--z-fx-flash` | `25` | card activation flash |
| `--z-dock` | `30` | hand dock, player peek, mark tip, hover preview |
| `--z-peek` | `31` | draw-pile peek |
| `--z-preview` | `32` | hand-card preview |
| `--z-transport` | `35` | replay transport bar + show handle |
| `--z-chrome-banner` | `36` | replay status banners (divergence / verify) |
| `--z-modal` | `40` | modal back, inline modal, prompt sheet, zoom control |
| `--z-auto-float` | `41` | 托管 float, "thinking" pill |
| `--z-modal-fore` | `42` | restore tray over the log |
| `--z-auto-banner` | `45` | auto-mode banner |
| `--z-hosted` | `60` | stage-hosted card peek (escapes scroll boxes) |
| `--z-toast` | `80` | toasts |
| `--z-fader` | `90` | scene fader |
| `--z-console` | `100` | dev console (fixed, over everything) |

`--z-transport` is the replay playback bar (and its show handle). It sits
above every in-board surface the viewer may still be inspecting -- hand dock
(30), field row (15), peeks / previews (31–32), card flash (25) -- so
play / pause / seek stay clickable while a hand card is up. It sits below the
interruptions: a modal (40), toast (80) or the scene fader (90) covers the
transport, because those own the screen until answered. The status banners
(`--z-chrome-banner`) ride one step above the transport so a divergence /
verify notice is never buried under the bar.

### Runtime layout variables

Not design tokens -- measured or state-driven values a scene publishes on a
common ancestor; modules read them with a `0` fallback so a scene that does
not set them is unchanged.

| Variable | Set by | Meaning |
| --- | --- | --- |
| `--replay-bar-h` | `ReplayBar` (ResizeObserver on the bar) | measured transport height, `0` when hidden |
| `--replay-bar-bottom` | `ReplayBar` | height while the transport is docked at the **bottom**, else `0` |
| `--replay-bar-top` | `ReplayBar` | board-body top inset while the transport is docked at the **top**, else `0` |

Bottom-edge chrome (hand dock / peek / skill stack / sheet strip, the left
column's actions and player list, the log) adds `--replay-bar-bottom` to its
`bottom`. A top-docked transport rides **between the TopBar and the board**
(`.bar.dockTop { top: var(--topbar-h) }`), not over the TopBar; `--replay-bar-top`
is then `--topbar-h` + the bar's height, and Board `.body` takes it as its
`top`, dropping the whole board (field row, zoom control, card preview, …)
below the strip in one move. Floating transport: both are `0`, no offset.

## (b) Shared primitives — `ui/primitives.module.css`

A small set, composed with `composes: x from "../ui/primitives.module.css"`.
Each primitive is the shared skeleton only; a component module keeps its own
size and colour.

| Primitive | What it is | Adopted by |
| --- | --- | --- |
| `floatCard` | white peek/preview card: surface, `--r-lg`, `--float-shadow`, `--sp-2` padding, click-through | Players peek, Side peek/preview, CardPreview |
| `pill` | full-round pill skeleton: `--r-pill`, centred, bold | Chips, lang switch, AutoToggle, list labels |
| `chip` | small rounded label (tag / badge) | Card tags, FieldSheet markers, Replay badges |
| `panel` | surface panel: `--panel`, `--shadow`, `--r-md` | scene panels (Deck, Room, Boot…) |

New copies of these shapes should compose the primitive, not restate it.

## (c) Component modules

* **Tokens and primitives only.** No hex colours, no raw `z-index`, no magic
  numbers that another file also needs (those are layout tokens).
* **One concern per rule.** A rule is either layout, or surface, or type — not
  all three in one 900-character line.
* **Formatting.** Multi-line, one declaration per line, for any rule with more
  than ~3 declarations. Short rules may stay on one line.
* **Naming.** camelCase class names per module. State is `is*` or a
  `data-*` attribute (`[data-state="raised"]`), never a deep
  `.aboveSheet:not(.dockUp) .fan` chain. Deliberate state hooks are fine; the
  chain is what goes.
* **No cross-file coupling.** No "must match X.module.css" comments — if two
  files need a number, it is a layout token.
* **No dead rules.** Empty selectors, selectors nothing renders, and superseded
  hacks go.

## Incremental adoption

1. tokens (`base.css` + `styles/layout.ts`)
2. z-index scale (every raw number → a name)
3. primitives (`ui/primitives.module.css`, the five float cards, pills, chips)
4. per-area cleanup (geometry tokens in Board/Side/FieldSheet/Modal first, then
   the rest); `Ring.module.css` last (another agent is in it)
5. dead-rule removal and multi-line formatting as each area is touched

## Metrics

| | before | after |
| --- | --- | --- |
| lines | 2059 | 3201 (multi-line formatting) |
| rules | ~1144 | ~1154 |
| raw `z-index` numbers | 51 | 0 (outside Ring's in-tile 0..10) |
| hex colours in modules | 304 | 242 (the rest are the map tile palette) |
| `!important` | 1 | 0 |
| `:root` tokens | 21 | 96 |
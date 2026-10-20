// Full-screen deck adjuster for the separated `deck` window. After the character
// picks are public, load a seed (the preset or any named saved deck -- even an
// incomplete one), tweak it against the legal pool, and submit one `deck` act.
// The match gets just the card list: tweaks are match-local and never written
// back into saved deck slots. Lives for the whole `deck` phase -- Play swaps
// away when the phase moves on.

import { useMemo, useState } from "react";
import { navigate } from "../../app/router";
import { sceneImg } from "../../core/assets";
import { sfx } from "../../core/audio";
import { cx } from "../../core/cx";
import { D, cardTitle, rules } from "../../core/data";
import { deckLabelWith, parseDeckList, type DeckEntry } from "../../core/deckNames";
import { useAutoplay, useMatchView, useTick, useWakeLock } from "../../core/hooks";
import { hasProfile, profileJson, updateProfile } from "../../core/store";
import type { CardData, Command, MatchPlayer, MatchView, PersonaData } from "../../core/types";
import { endSession, type GameSession } from "../../game/session";
import { useCardTap } from "../../hooks/doubleTap";
import { Btn } from "../../ui/Button";
import { CARD_KIND_LABEL, cardKind, EmptySlot, InspectCard, KindChip, type CardKind } from "../../ui/Card";
import { cardStandClass, CardPreview, stickCard, useStandingPreviewOn, useStickyCard } from "../../ui/CardPreview";
import { Avatar, BandMark, NamePlate } from "../../ui/Character";
import { Chips } from "../../ui/Chips";
import { Icon } from "../../ui/Icon";
import { Live2DStand } from "../../ui/Live2DStand";
import { ask } from "../../ui/Modal";
import { toast } from "../../ui/Toast";
import { TopBar } from "../../ui/TopBar";
import { fmtMsg } from "../../i18n/msg";
import { namesOf } from "../../core/names";
import s from "./DeckAdjust.module.css";
import { t as tr } from "../../i18n/t";

const SIZE = 10;
// Derived cards never enter a starting deck (`err.deck_derived`), so they are
// not offered here -- unlike the menu deck editor, which shows them read-only.
const FILTERS: (CardKind | "all")[] = ["all", "exclusive", "band", "general"];
type Filter = (typeof FILTERS)[number];

const filterLabel = (f: Filter) => (f === "all" ? tr("common.all") : tr(CARD_KIND_LABEL[f]));
const matches = (c: CardData, f: Filter) => (f === "all" ? cardKind(c) !== "derived" : cardKind(c) === f);
const sameList = (a: readonly string[], b: readonly string[]) => a.length === b.length && a.every((x, i) => x === b[i]);

const modeChip = (mode: number): [string, string] => ([[tr("mode.solo"), "solo"], [tr("mode.casual"), "casual"], [tr("mode.ranked"), "ranked"]] as [string, string][])[mode] ?? [tr("mode.solo"), "solo"];

/** A starting point the player can load: the preset, or a named saved deck. */
type Seed = { id: number; name: string; cards: string[] };

export function DeckAdjust({ sess }: { sess: GameSession }) {
  const { view, at } = useMatchView(sess);
  useTick(500); // the deck-window countdown moves with the seconds
  const auto = useAutoplay(sess); // 托管
  useWakeLock(true); // a match is on from the ban / pick phase
  // The right-hand standing card panel replaces the floating hover popups here.
  useStandingPreviewOn();

  const me = view?.state.players[view.playerId];
  const c = me?.persona ? D.persona(me.persona) : undefined;
  if (!view || !me || !c) return null;
  return <Adjust sess={sess} view={view} at={at} auto={auto} me={me} c={c} />;
}

function Adjust({ sess, view, at, auto, me, c }: {
  sess: GameSession; view: MatchView; at: number; auto: boolean; me: MatchPlayer; c: PersonaData;
}) {
  // Legal pool in canonical order (exclusives → band → general).
  const pool = useMemo(() => {
    const ids: string[] = JSON.parse(rules.deck_pool(c.name));
    return ids.map((id) => D.card(id)!).filter(Boolean);
  }, [c.name]);
  const poolIds = useMemo(() => new Set(pool.map((x) => x.id)), [pool]);

  const list: DeckEntry[] = hasProfile() ? parseDeckList(rules.deck_list(profileJson(), c.name)) : [];
  const label = (d: { id: number; name: string }) => deckLabelWith(d, (n) => tr("deck.autoName", { n }));
  const seeds: Seed[] = [
    { id: 0, name: tr("deck.presetName"), cards: JSON.parse(rules.deck_preset(c.name)) as string[] },
    ...list.map((d) => ({ id: d.id, name: label(d), cards: d.cards })),
  ];

  // Working buffer, seeded from the profile's default (or the preset).
  const [seedId, setSeedId] = useState<number>(() => {
    const n = hasProfile() ? rules.deck_chosen_slot(profileJson(), c.name) : 0;
    return seeds.some((r) => r.id === n) ? n : 0;
  });
  const [buffer, setBuffer] = useState<string[]>(() => seeds.find((r) => r.id === seedId)?.cards.slice() ?? []);
  const [filter, setFilter] = useState<Filter>("all");

  const st = view.state;
  const secs = Math.max(0, Math.ceil(st.timeLeft - (performance.now() - at) / 1000));
  const full = buffer.length >= SIZE;
  // Submitted (here, by the timer, or after a refresh elsewhere): lock editing.
  const ready = me.deckReady;
  const locked = auto || ready;
  const canConfirm =
    !locked && buffer.length === SIZE && new Set(buffer).size === SIZE && buffer.every((id) => poolIds.has(id));

  const act = async (cmd: Command) => {
    if (sess.autoMode !== "off") return false;
    const err = await sess.act(cmd);
    if (err) toast(fmtMsg(err, namesOf(sess.view?.state)), "error");
    return !err;
  };

  const leave = async () => {
    if (!(await ask(tr("select.leaveTitle"), tr("select.leaveAsk"), tr("select.leave")))) return;
    endSession();
    navigate(sess.kind === "solo" ? { name: "menu" } : { name: "lobby" });
  };

  function toggle(id: string): void {
    if (locked) return;
    const why = rules.deck_why_not(c.name, id);
    if (why) return toast(why);
    if (!poolIds.has(id)) return toast(tr("deck.cannotAdd"));
    const has = buffer.includes(id);
    if (!has && full) return toast(tr("deck.fullToast", { n: SIZE }));
    sfx("place");
    setBuffer(has ? buffer.filter((x) => x !== id) : [...buffer, id]);
  }

  function loadSeed(r: Seed): void {
    if (locked) return;
    setSeedId(r.id);
    setBuffer(r.cards.slice());
  }

  function reset(): void {
    if (locked) return;
    setBuffer(seeds.find((r) => r.id === seedId)?.cards.slice() ?? []);
  }

  function clear(): void {
    if (locked) return;
    setBuffer([]);
  }

  /** Pad from the pool, keeping the player's insertion order on screen. */
  function fill(): void {
    if (locked) return;
    if (buffer.length >= SIZE) return toast(tr("deck.filledNone"));
    const next = [...buffer];
    for (const x of pool) {
      if (next.length >= SIZE) break;
      if (!x.derived && !next.includes(x.id) && !rules.deck_why_not(c.name, x.id)) next.push(x.id);
    }
    setBuffer(next);
    toast(tr("deck.filled", { n: next.length - buffer.length }));
  }

  // Whole-face add / remove: PC single click, touch double tap (first tap only
  // previews). `toggle` already guards 托管 (`locked`); preview never mutates.
  const cardTap = useCardTap({ onPreview: stickCard, onToggle: toggle });
  const card = useStickyCard();

  async function confirm(): Promise<void> {
    if (!canConfirm) return;
    const submitted = buffer.slice();
    if (!(await act({ act: "deck", cards: submitted }))) return;
    // Remember the seed as this character's default only when the list is
    // unchanged -- a tweak is match-local and must not touch saved slots.
    if (!hasProfile()) return;
    const hit = seeds.find((r) => sameList(submitted, r.cards));
    if (hit) updateProfile(rules.deck_choose(profileJson(), c.name, hit.id));
  }

  const [modeLabel, modeCls] = modeChip(st.mode);
  const stepIdx = st.phase === "order" ? 0 : st.phase === "ban" ? 1 : st.phase === "pick" ? 2 : 3;
  const step = (n: number, text: string, skip = false) => (
    <span className={cx(s.step, skip ? s.skip : n === stepIdx ? s.stepOn : n < stepIdx && s.stepDone)}>{n} {text}</span>
  );

  return (
    <>
      <TopBar
        section={tr("select.section")}
        title={tr("deckPick.heading", { char: c.display })}
        onBack={leave}
        right={
          <div className={s.steps}>
            <span className={cx(s.mode, s[modeCls])}>{modeLabel}</span>
            {step(1, tr("select.stepBan"), st.mode !== 2)}<i>›</i>{step(2, tr("select.stepPick"))}<i>›</i>{step(3, tr("select.stepDeck"))}
            {sess.kind !== "solo" && (
              <span className={s.clock}>
                <Icon name="timer" />
                {tr("deckPick.deckWindow")}
                <b>{secs}</b>
                {tr("common.unitSec")}
              </span>
            )}
          </div>
        }
      />
      {/* No 托管 toggle before the match (see Select). */}

      <div className={s.body}>
        <div className={s.left}>
          <div className={s.char}>
            <div className={s.glow} style={{ background: c.color }} />
            <Live2DStand key={D.artId(c)} id={D.artId(c)} className={s.stand} zoom={1.25} focusTop={0.13} headroom={0.103} />
            <div className={s.band}><BandMark band={c.group} /></div>
            <div className={s.plate}><NamePlate c={c} width={350} /></div>
          </div>

          {/* Public picks this window is reacting to. Opponent decks stay hidden. */}
          <div className={s.picks}>
            <div className={s.picksHead}>{tr("deckPick.picks")}</div>
            <div className={s.pickList}>
              {st.players.map((x, i) => {
                const pc = D.persona(x.persona);
                const mine = i === view.playerId;
                return (
                  <div key={i} className={cx(s.seat, mine && s.mine)}>
                    <div className={s.av}>
                      <Avatar c={pc} size={40} />
                      {x.bot && <span className={s.bot}><Icon name="smart_toy" size={12} /></span>}
                      {x.deckReady && <span className={s.ok}><Icon name="check" size={12} /></span>}
                    </div>
                    <div className={s.who}>
                      <b>{mine ? tr("common.you") : x.player}</b>
                      <small>{pc?.display ?? x.persona}</small>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        </div>

        <div className={s.right}>
          <p className={s.desc}>{tr("deckPick.desc")}</p>

          {/* Seeds are starting points, not submits. Incomplete ones are fine. */}
          <div className={s.seeds}>
            {seeds.map((r) => (
              <button
                key={r.id}
                type="button"
                className={cx(s.seed, sameList(buffer, r.cards) && s.seedOn)}
                disabled={locked}
                title={r.id === 0 ? (c.preset?.length ? tr("deckPick.presetCustom") : tr("deckPick.presetAuto")) : undefined}
                onClick={() => loadSeed(r)}
              >
                <b>{r.name}</b>
                <small>{r.cards.length}/{SIZE}</small>
              </button>
            ))}
          </div>

          <div className={s.strip}>
            <div className={s.stripHead}>
              <b>{tr("deckPick.starting")}</b>
              <div className={s.count}>
                <span className={buffer.length === SIZE ? s.full : ""}>{buffer.length}</span> / {SIZE}
              </div>
              <div className={s.row}>
                <Btn size="small" icon="auto_awesome" disabled={locked} onClick={fill}>{tr("deck.fill")}</Btn>
                <Btn size="small" icon="redo" disabled={locked} onClick={reset}>{tr("deckPick.reset")}</Btn>
                <Btn size="small" icon="close" disabled={locked || !buffer.length} onClick={clear}>{tr("deck.clear")}</Btn>
              </div>
            </div>
            <div className={s.stripCards}>
              {Array.from({ length: SIZE }, (_, k) => (buffer[k]
                ? <InspectCard key={k} id={buffer[k]} size="strip" onClick={cardTap(buffer[k])} title={tr("deck.remove")} />
                : <EmptySlot key={k} star={sceneImg("star5")} />))}
            </div>
            <div className={s.hint}>{tr("deckPick.hint")}</div>
          </div>

          <div className={s.pool}>
            <div className={s.poolHead}>
              <Chips
                items={FILTERS.map(filterLabel)}
                on={filterLabel(filter)}
                count={(l) => pool.filter((x) => matches(x, FILTERS.find((f) => filterLabel(f) === l) ?? "all")).length}
                onPick={(l) => setFilter(FILTERS.find((f) => filterLabel(f) === l) ?? "all")}
              />
              <small>{tr("deckPick.poolHint")}</small>
            </div>
            <div className={s.grid}>
              {pool.filter((x) => matches(x, filter)).map((x) => {
                const on = buffer.includes(x.id);
                return (
                  <InspectCard key={x.id} id={x.id} size="pool" on={on} onClick={cardTap(x.id)} title={cardTitle(x.id)}>
                    <KindChip kind={cardKind(x)} className={s.kind} />
                  </InspectCard>
                );
              })}
            </div>
          </div>

          <div className={s.foot}>
            {ready
              ? <div className={s.done}>{tr("select.deckDone")}</div>
              : <Btn kind="pink" className={s.go} disabled={!canConfirm} onClick={() => void confirm()}>{tr("common.confirm")}</Btn>}
          </div>
        </div>

        {/* Standing card preview (the board's CardStand): the one detail this
            screen shows, fed by hover / tap. No floating InspectPreviewHost. */}
        <div className={s.standCol} aria-live="polite">
          <div className={s.standFrame}>
            {card ? (
              <CardPreview id={card.id} note={card.note} pills={card.pills} className={cardStandClass} scrollable />
            ) : (
              <div className={s.standEmpty}>{tr("prompt.cardHover")}</div>
            )}
          </div>
        </div>
      </div>
    </>
  );
}
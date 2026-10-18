// Quick deck adjuster inside the separated `deck` window. After the character
// picks are public, load a seed (the preset or any named saved deck -- even an
// incomplete one), tweak it against the legal pool, and submit one `deck` act.
// The match gets just the card list: tweaks are match-local and never written
// back into saved deck slots.

import { useMemo, useState } from "react";
import { sfx } from "../../core/audio";
import { sceneImg } from "../../core/assets";
import { cx } from "../../core/cx";
import { D, cardTitle, rules } from "../../core/data";
import { deckLabelWith, parseDeckList, type DeckEntry } from "../../core/deckNames";
import { useAutoplay, useMatchView, useTick } from "../../core/hooks";
import { useCloseWhen } from "../../hooks/mount";
import { hasProfile, profileJson, updateProfile } from "../../core/store";
import type { CardData, Command } from "../../core/types";
import type { GameSession } from "../../game/session";
import { Btn } from "../../ui/Button";
import { CARD_KIND_LABEL, cardKind, EmptySlot, InspectCard, KindChip, showCard, type CardAction, type CardKind } from "../../ui/Card";
import { Avatar } from "../../ui/Character";
import { Chips } from "../../ui/Chips";
import { Icon } from "../../ui/Icon";
import { openModal } from "../../ui/Modal";
import { toast } from "../../ui/Toast";
import s from "./DeckPick.module.css";
import { t as tr } from "../../i18n/t";

const SIZE = 10;
// Derived cards never enter a starting deck (`err.deck_derived`), so they are
// not offered here -- unlike the menu deck editor, which shows them read-only.
const FILTERS: (CardKind | "all")[] = ["all", "exclusive", "band", "general"];
type Filter = (typeof FILTERS)[number];

const filterLabel = (f: Filter) => (f === "all" ? tr("common.all") : tr(CARD_KIND_LABEL[f]));
const matches = (c: CardData, f: Filter) => (f === "all" ? cardKind(c) !== "derived" : cardKind(c) === f);
const sameList = (a: readonly string[], b: readonly string[]) => a.length === b.length && a.every((x, i) => x === b[i]);

/** A starting point the player can load: the preset, or a named saved deck. */
type Seed = { id: number; name: string; cards: string[] };

function DeckPick({ sess, character, act, close }: { sess: GameSession; character: string; act: (c: Command) => Promise<boolean>; close: () => void }) {
  const c = D.persona(character)!;
  const auto = useAutoplay(sess); // 托管
  const { view, at } = useMatchView(sess);
  useTick(500); // the deck-window countdown moves with the seconds

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

  // Submitted (here, by the timer, or after a refresh elsewhere): close.
  const ready = view?.state.players[view.playerId]?.deckReady || view?.state.phase !== "deck";
  useCloseWhen(ready, close);

  const st = view?.state;
  const secs = st ? Math.max(0, Math.ceil(st.timeLeft - (performance.now() - at) / 1000)) : 0;
  const full = buffer.length >= SIZE;
  const canConfirm =
    !auto && buffer.length === SIZE && new Set(buffer).size === SIZE && buffer.every((id) => poolIds.has(id));

  function toggle(id: string): void {
    if (auto) return;
    const why = rules.deck_why_not(c.name, id);
    if (why) return toast(why);
    if (!poolIds.has(id)) return toast(tr("deck.cannotAdd"));
    const has = buffer.includes(id);
    if (!has && full) return toast(tr("deck.fullToast", { n: SIZE }));
    sfx("place");
    setBuffer(has ? buffer.filter((x) => x !== id) : [...buffer, id]);
  }

  function loadSeed(r: Seed): void {
    if (auto) return;
    setSeedId(r.id);
    setBuffer(r.cards.slice());
  }

  function reset(): void {
    if (auto) return;
    setBuffer(seeds.find((r) => r.id === seedId)?.cards.slice() ?? []);
  }

  function clear(): void {
    if (auto) return;
    setBuffer([]);
  }

  /** Pad from the pool, keeping the player's insertion order on screen. */
  function fill(): void {
    if (auto) return;
    if (buffer.length >= SIZE) return toast(tr("deck.filledNone"));
    const next = [...buffer];
    for (const x of pool) {
      if (next.length >= SIZE) break;
      if (!x.derived && !next.includes(x.id) && !rules.deck_why_not(c.name, x.id)) next.push(x.id);
    }
    setBuffer(next);
    toast(tr("deck.filled", { n: next.length - buffer.length }));
  }

  function detail(id: string): void {
    const card = D.card(id)!;
    const has = buffer.includes(id);
    const acts: CardAction[] = card.derived
      ? [{ label: tr("deck.cannotAdd"), enabled: false }]
      : has
        ? [{ label: tr("deck.remove"), enabled: !auto, run: () => toggle(id) }]
        : [{ label: full ? tr("deck.full") : tr("deck.add"), enabled: !full && !auto, run: () => toggle(id) }];
    showCard(id, acts);
  }

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

  return (
    <div className={s.pick}>
      <div className={s.head}>
        <h3>{tr("deckPick.heading", { char: c.display })}</h3>
        {sess.kind !== "solo" && (
          <span className={s.clock}>
            <Icon name="timer" />
            {tr("deckPick.deckWindow")}
            <b>{secs}</b>
            {tr("common.unitSec")}
          </span>
        )}
      </div>
      <p>{tr("deckPick.desc")}</p>

      {/* Public picks this window is reacting to. Opponent decks stay hidden. */}
      <div className={s.picks}>
        <small className={s.picksLabel}>{tr("deckPick.picks")}</small>
        <div className={s.pickRow}>
          {st?.players.map((x, i) => {
            const pc = D.persona(x.persona);
            const mine = i === view?.playerId;
            return (
              <div key={i} className={cx(s.seat, mine && s.mine)}>
                <div className={s.av}>
                  <Avatar c={pc} size={36} />
                  {x.bot && <span className={s.bot}><Icon name="smart_toy" size={12} /></span>}
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

      {/* Seeds are starting points, not submits. Incomplete ones are fine. */}
      <div className={s.seeds}>
        {seeds.map((r) => (
          <button
            key={r.id}
            type="button"
            className={cx(s.seed, sameList(buffer, r.cards) && s.seedOn)}
            disabled={auto}
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
            <Btn size="small" icon="auto_awesome" disabled={auto} onClick={fill}>{tr("deck.fill")}</Btn>
            <Btn size="small" icon="redo" disabled={auto} onClick={reset}>{tr("deckPick.reset")}</Btn>
            <Btn size="small" icon="close" disabled={auto || !buffer.length} onClick={clear}>{tr("deck.clear")}</Btn>
          </div>
        </div>
        <div className={s.stripCards}>
          {Array.from({ length: SIZE }, (_, k) => (buffer[k]
            ? <InspectCard key={k} id={buffer[k]} size="strip" onClick={() => toggle(buffer[k])} title={tr("deck.remove")} />
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
          <small>{tr("deck.poolHint")}</small>
        </div>
        <div className={s.grid}>
          {pool.filter((x) => matches(x, filter)).map((x) => {
            const on = buffer.includes(x.id);
            return (
              <InspectCard key={x.id} id={x.id} size="pool" on={on} onClick={() => detail(x.id)} title={cardTitle(x.id)}>
                <KindChip kind={cardKind(x)} className={s.kind} />
                {!x.derived && (
                  <button type="button" className={cx(s.toggle, on && s.toggleOn)} disabled={auto} onClick={(e) => { e.stopPropagation(); toggle(x.id); }}>
                    <Icon name={on ? "check" : "add"} />
                  </button>
                )}
              </InspectCard>
            );
          })}
        </div>
      </div>

      <div className={s.foot}>
        <Btn kind="pink" wide disabled={!canConfirm} onClick={() => void confirm()}>{tr("common.confirm")}</Btn>
      </div>
    </div>
  );
}

export function showDeckPick(sess: GameSession, character: string, act: (c: Command) => Promise<boolean>): void {
  openModal(tr("deckPick.title"), (close) => <DeckPick sess={sess} character={character} act={act} close={close} />, { size: "xl", key: "deck" });
}
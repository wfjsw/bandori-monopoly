// Compact turn controls at the bottom left; the Hand is docked below the map.

import { memo, useCallback, useRef, useState, type CSSProperties } from "react";
import { handLimit } from "../../core/hand";
import { cardArt, sceneImg } from "../../core/assets";
import { cx } from "../../core/cx";
import { D, cardTitle, cardText } from "../../core/data";
import { useAutoplay } from "../../core/hooks";
import { useEventListener, useHotkeys } from "../../hooks/dom";
import { useDockRaise } from "../../hooks/dock";
import { holdPeek } from "../../hooks/hold";
import { PICK_ATTR, usePickDeselect } from "../../hooks/pick";
import { useResetScroll } from "../../hooks/measure";
import type { GameSession } from "../../game/session";
import { AutoToggle, ThinkingPill } from "../../ui/AutoToggle";
import { Btn } from "../../ui/Button";
import { CardFace, type CardAction, showCard, TagChip } from "../../ui/Card";
import { bandColor } from "../../ui/Character";
import { PanelTab } from "../../ui/Chips";
import { usePromptSheet } from "../../ui/Modal";
import { useStandingMode, stickCard } from "../../ui/CardPreview";
import { SkillBody } from "../../ui/SkillBody";
import { act, type Model } from "./model";
import { byTitle, showDeck, showDeedList } from "./Popups";
import { useCompactValue } from "../../app/Stage";
import { BOARD_MID, COMPACT_MID, SKILL_CARD_W, SKILL_FAN_GAP, SKILL_STEP } from "../../styles/layout";
import s from "./Side.module.css";
import { t as tr } from "../../i18n/t";
import { fmtMsg } from "../../i18n/msg";
import { namesOf } from "../../core/names";

export function Side({ sess }: { sess: GameSession }) {
  const auto = useAutoplay(sess);
  // A replay is read-only: every action control (skill, roll / continue,
  // mortgage, redeem) is hidden, not disabled, and the grid collapses away --
  // the left column is just the players. Inspection (tiles, cards, log) stays.
  if (sess.readOnly) return null;
  return (
    <div className={s.actions}>
      <Btn icon="account_balance" className={s.act} disabled={auto} onClick={() => showDeedList(sess)}>{tr("board.deeds")}</Btn>
    </div>
  );
}

/** Only appears when somebody has already requested an online settlement. */
export const SettleVote = memo(function SettleVote({ m, sess }: { m: Model; sess: GameSession }) {
  const auto = useAutoplay(sess);
  const vote = m.S.vote;
  const k = vote.players.indexOf(m.playerId);
  if (!vote.id) return null;
  return (
    <div className={s.vote}>
      <span>{tr("board.voteStatus", { n: vote.answers.filter((a) => a === 1).length, total: vote.players.length })}</span>
      {/* Replay keeps the tally as read-out and drops the buttons. */}
      {!sess.readOnly && k >= 0 && vote.answers[k] < 0 && <div className={s.voteActions}>
        <Btn kind="pink" size="small" disabled={auto} onClick={() => void act(sess, { act: "vote", value: 1 })}>{tr("board.voteFor")}</Btn>
        <Btn size="small" disabled={auto} onClick={() => void act(sess, { act: "vote", value: 0 })}>{tr("board.voteAgainst")}</Btn>
      </div>}
    </div>
  );
});

/**
 * One pressable skill on the stack. `id` / `enabled` / `reason` come from the
 * engine's `MatchView.skills` (gated through the same `why_not_act` the
 * `act: "skill"` request takes). Name / body resolve from the rule id.
 */
type SkillSlot = {
  id: string;
  enabled: boolean;
  /** Already-formatted reason, or empty when the card is usable. */
  reason: string;
  /** Already-formatted skill body (the hover preview's text). */
  text: string;
  /** Tooltip: body when usable, the refusal when not. */
  tip: string;
};

/**
 * The live markers a skill card carries, for its top-right corner: the field
 * instance's crystals, on-card CP and named counters, plus the player's
 * counter units bound to that instance (fire pots and the like when a skill
 * keeps them as its own counter). Zero counts are dropped.
 */
export function skillMarks(p: Model["S"]["players"][number] | undefined, id: string): { key: string; label: string; title: string }[] {
  const fc = p?.field?.find((c) => c.card === id);
  if (!p || !fc) return [];
  const out: { key: string; label: string; title: string }[] = [];
  if (fc.counters["crystals"] ?? 0 > 0) out.push({ key: "crystals", label: `◆${fc.counters["crystals"] ?? 0}`, title: tr("board.crystals", { n: fc.counters["crystals"] ?? 0 }) });
  if (fc.counters["cp"] ?? 0 > 0) out.push({ key: "cp", label: `CP${fc.counters["cp"] ?? 0}`, title: tr("board.cp", { n: fc.counters["cp"] ?? 0 }) });
  for (const [name, n] of Object.entries(fc.counters ?? {})) {
    if (n > 0) out.push({ key: `c:${name}`, label: `${name}×${n}`, title: `${name} ×${n}` });
  }
  for (const tok of p.tokens ?? []) {
    if (tok.instance === fc.uid && tok.value > 0) out.push({ key: `t:${tok.name}`, label: `${tok.name}×${tok.value}`, title: `${tok.name} ×${tok.value}` });
  }
  return out;
}

/** The viewer's pressable skills, straight from the engine's list. */
export function skillSlots(m: Model): SkillSlot[] {
  const names = namesOf(m.S);
  return (m.v.skills ?? []).map((a) => {
    const text = cardText(a.id) || fmtMsg(a.text, names);
    const reason = a.enabled ? "" : fmtMsg(a.reason, names) || tr("skills.unavailable");
    return { id: a.id, enabled: a.enabled, reason, text, tip: a.enabled ? text : reason };
  });
}

/**
 * The player's pressable skills as a dense horizontal stack on the left of the
 * hand fan, inside the dock so it rises and retracts with the hand. Straight
 * row -- no arc, no rotation -- overlapping tighter than the fan so 1-4 skills
 * (character + band + any granted) read as one short strip. Hover lifts a card
 * to the top of the stack and feeds the same standing preview the hand uses.
 * Click is the hand's two-step: first click highlights the card, clicking it
 * again opens the detail popup, whose button is the confirmation that fires
 * the skill (inspect-only when disabled, 托管, replay, a prompt sheet up, or a
 * use already in flight). The pick is a lens, not a mode: a pointer-down
 * anywhere outside that card drops it again.
 * The list is the engine's (`MatchView.skills`): only rules with an
 * activatable `On::Play` appear, each already gated. Passive / hook rules
 * never land here -- they have no activation and are read from the
 * character's skill text instead.
 */
function SkillStack({ m, sess, peek, unpeek }: {
  m: Model;
  sess: GameSession;
  peek: (id: string, note: string, text?: string) => void;
  unpeek: () => void;
}) {
  const auto = useAutoplay(sess);
  const sheet = usePromptSheet();
  const slots = skillSlots(m);
  const me = m.me;
  // One use in flight: a second click before the engine answers is dropped.
  const [using, setUsing] = useState(false);
  // First click selects (highlighted border); clicking the selected card opens
  // the detail popup -- same two-step as the hand and the field row. Clicking
  // anywhere else drops the pick again -- it is a lens, not a mode.
  const [picked, setPicked] = useState<string | null>(null);
  const unpick = useCallback(() => setPicked(null), []);
  usePickDeselect(picked, unpick);
  if (!slots.length) return null;
  const fire = async (id: string) => {
    if (using) return;
    setUsing(true);
    try {
      await act(sess, { act: "skill", card: id });
    } finally {
      setUsing(false);
    }
  };
  return (
    <div className={s.skillStack}>
      <div className={s.skillRow}>
        {slots.map((a) => {
          const usable = a.enabled && !auto && !sess.readOnly && !sheet.open;
          const marks = skillMarks(me, a.id);
          return (
            <div
              key={a.id}
              {...{ [PICK_ATTR]: a.id }}
              className={s.skillSlot}
              title={a.tip}
            >
              <CardFace
                id={a.id}
                size="mini"
                width={SKILL_CARD_W}
                className={s.skillCard}
                on={picked === a.id}
                playable={usable}
                onClick={() => {
                  if (using) return;
                  if (picked !== a.id) return void setPicked(a.id);
                  setPicked(null);
                  // The detail popup is the confirmation: its button fires the skill.
                  const acts: CardAction[] = usable ? [{ label: tr("board.useSkill"), enabled: true, run: () => fire(a.id) }] : [];
                  showCard(a.id, acts, a.enabled ? a.text : a.reason);
                }}
                onMouseEnter={() => peek(a.id, a.enabled ? "" : a.reason, a.text)}
                onMouseLeave={unpeek}
                onFocus={() => peek(a.id, a.enabled ? "" : a.reason, a.text)}
                onBlur={unpeek}
              />
              {marks.length > 0 && (
                <div className={s.skillMarks}>
                  {marks.map((x) => <span key={x.key} className={s.skillMark} title={x.title}>{x.label}</span>)}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

/**
 * One card's place in the hand's circular-sector fan: a tilt around a pivot
 * below the hand, a gentle vertical arc (outer cards lower, centre highest),
 * and a spread that tightens as the hand grows so 1-10 cards all fit the
 * centre column. `spread` is the budget for the gaps between card centres --
 * the caller shrinks it to whatever the skill stack leaves free. Returns the
 * CSS custom properties `.fanCard` reads.
 */
export function fanArc(n: number, k: number, spread = 660): Record<string, string> {
  const step = n > 1 ? Math.min(8, 52 / (n - 1)) : 0; // degrees per card, total ≤ 52°
  const a = (k - (n - 1) / 2) * step;
  const aMax = ((n - 1) / 2) * step;
  // The arc drop: R(1 - cos a) against a 420px pivot radius, normalised so the
  // outer cards sit on the baseline and the centre one rides highest.
  const drop = (deg: number) => 420 * (1 - Math.cos((deg * Math.PI) / 180));
  const rise = drop(aMax) - drop(a);
  const slot = n > 1 ? Math.min(112, spread / (n - 1)) : 0; // px between card centres
  return {
    "--fan-a": `${a.toFixed(2)}deg`,
    "--fan-y": `${(-rise).toFixed(1)}px`,
    "--fan-slot": `${slot.toFixed(1)}px`,
  };
}

/** How high the centre card rides above the outer ones -- the retracted peek
 *  measures from there, so the fan shows 30px of its tallest point. */
export function fanRise(n: number, spread = 660): string {
  return `${Math.abs(parseFloat(fanArc(n, (n - 1) / 2, spread)["--fan-y"]))}px`;
}

/**
 * The match hand, docked at the bottom of the middle column like Master Duel's:
 * a fanned row of full card faces, retracted so only the top strip of each card
 * peeks above the bottom edge. Hovering the dock (or focusing a card) raises
 * the hand; leaving retracts it after a short delay. Hovering one card lifts it
 * clear of its neighbours and floats the full text. The slim bar under the fan
 * (hand count, hint, draw pile, 托管) stays visible either way.
 *
 * A prompt sheet owns the bottom edge while it is up: the fan stays retracted
 * behind it (no hover-raise over the sheet), and the hand is inspect-only --
 * looking at cards is fine, play / discard are not. Once the sheet retracts the
 * hand raises again for looking; once the question is answered it is normal.
 */
export const Hand = memo(function Hand({ m, sess, busy }: { m: Model; sess: GameSession; busy: boolean }) {
  // One hover target for both the hand fan and the skill stack: the dock's own
  // floating preview and the standing card panel read this.
  const [hover, setHover] = useState<{ id: string; note: string; text?: string } | null>(null);
  const [peek, setPeek] = useState(false);
  const auto = useAutoplay(sess); // 托管: play / discard are locked (inspect stays)
  const sheet = usePromptSheet();
  const stand = useStandingMode();
  // Raise on hover / focus; retract only after a short delay, so moving between
  // cards (or to the bar and back) does not drop the hand out from under you.
  // A raised sheet owns the band above the bar -- the fan stays down behind it.
  const dock = useDockRaise({ blocked: sheet.raised });
  const S = m.S;
  const limit = handLimit(m.me);
  const canPlay = m.myTurn && S.step === 2 && !S.busy && !m.asking && !busy && !auto;
  // Hovering a card lifts it and feeds the standing card panel.
  const peekCard = (id: string, note: string, text?: string) => {
    setHover({ id, note, text });
    stickCard(id, note);
  };
  const peekHand = (k: number, id: string) => peekCard(id, fmtMsg(m.v.handNotes[k], namesOf(S)));
  // Playable right now -- the play action's own gate. Glows blue on the face;
  // everything else stays an ordinary card (no dimming). Over the hand limit
  // the action is discard, not play, so nothing glows. Absent `playable` (solo
  // before merge / failed online extras) keeps the card eligible.
  const cardPlayable = (k: number) =>
    canPlay && !sess.readOnly && !sheet.open && !m.overHand && m.v.playable?.[k] !== false;
  // First click selects (highlighted border); clicking the selected card (or
  // Enter) plays it directly; while over the limit it instead opens the detail
  // popup, whose button discards (discarding needs that confirmation). Clicking
  // anywhere else drops the pick again -- it is a lens, not a mode.
  const [picked, setPicked] = useState<string | null>(null);
  const unpick = useCallback(() => setPicked(null), []);
  usePickDeselect(picked, unpick);
  const activate = (id: string, k: number) => {
    const key = `${id}:${k}`;
    if (picked !== key) return void setPicked(key);
    setPicked(null);
    setHover(null);
    if (sess.readOnly || sheet.open || auto) return;
    if (m.overHand) {
      const note = fmtMsg(m.v.handNotes[k], namesOf(m.S));
      return void showCard(id, [{ label: tr("board.discardThis"), enabled: true, kind: "white", run: () => act(sess, { act: "discard", card: id }) }], note);
    }
    if (cardPlayable(k)) void act(sess, { act: "play", card: id });
  };
  const hc = hover ? D.card(hover.id) : undefined;
  const deck = byTitle(m.v.draw ?? []);
  // Left skill stack: the fan stays centred on the column regardless; its
  // spread reserves the stack's width on *both* sides, so a wide hand tightens
  // symmetrically and never runs under the stack.
  const compact = useCompactValue();
  const skillN = skillSlots(m).length;
  const skillStep = skillN > 1 ? Math.min(SKILL_STEP, (240 - SKILL_CARD_W) / (skillN - 1)) : SKILL_STEP;
  const skillW = skillN ? SKILL_CARD_W + (skillN - 1) * skillStep + SKILL_FAN_GAP : 0;
  const fanSpread = Math.min(660, Math.max(0, (compact ? COMPACT_MID : BOARD_MID) - 2 * skillW - 120));
  // The preview is `pointer-events: none` (it sits above the hand, not under
  // the cursor), so a long card text is scrolled from the hovered card:
  // the wheel over the dock (fan or skill stack), or PgUp/PgDn/↑/↓ while a
  // card is hovered.
  const dockRef = useRef<HTMLDivElement>(null);
  const textRef = useRef<HTMLDivElement>(null);
  useResetScroll(textRef, hover?.id);
  const scrollableText = () => {
    const el = textRef.current;
    return el && el.scrollHeight > el.clientHeight ? el : null;
  };
  useEventListener(dockRef, "wheel", (e) => {
    const el = scrollableText();
    if (!el) return;
    e.preventDefault();
    const ev = e as WheelEvent;
    el.scrollTop += ev.deltaMode === 1 ? ev.deltaY * 21 : ev.deltaMode === 2 ? ev.deltaY * el.clientHeight : ev.deltaY;
  }, { passive: false });
  const line = 21; // `.pText` line-height: 13px × 1.6
  const scrollText = (deltaOf: (el: HTMLElement) => number) => (e: KeyboardEvent) => {
    const el = scrollableText();
    if (!el) return;
    e.preventDefault();
    el.scrollTop += deltaOf(el);
  };
  useHotkeys([
    { key: "ArrowDown", run: scrollText(() => line) },
    { key: "ArrowUp", run: scrollText(() => -line) },
    { key: "PageDown", run: scrollText((el) => Math.max(line, el.clientHeight - line)) },
    { key: "PageUp", run: scrollText((el) => -Math.max(line, el.clientHeight - line)) },
  ]);
  return (
    <>
      <div
        // A retracted prompt sheet's title strip sits on the bottom edge: lift the
        // whole hand above it so the cards stay easy to reach.
        className={s.dock}
        data-state={dock.raised ? "raised" : "retracted"}
        data-sheet={sheet.open && !sheet.raised ? "down" : "up"}
        style={{ ["--skill-step" as string]: `${skillStep}px` }}
        ref={dockRef}
        {...dock.hover}
      >
        {/* Fan + skill stack share one box above the bar, so the skill cards
            sit at the same height as the hand's when the dock raises. */}
        <div className={s.cards}>
          <div className={s.fan} style={{ ["--fan-rise" as string]: fanRise(m.v.hand.length, fanSpread) }}>
            {m.v.hand.map((id, k) => (
              // Playable cards glow blue (`cardPlayable`); the rest keep a
              // plain face. Inspect / pick still work either way.
              <div
                key={`${id}:${k}`}
                {...{ [PICK_ATTR]: `${id}:${k}` }}
                className={s.fanCard}
                style={fanArc(m.v.hand.length, k, fanSpread) as CSSProperties}
              >
                <CardFace
                  id={id}
                  size="mini"
                  title={cardTitle(id)}
                  on={picked === `${id}:${k}`}
                  playable={cardPlayable(k)}
                  onClick={() => activate(id, k)}
                  onMouseEnter={() => peekHand(k, id)}
                  onMouseLeave={() => setHover(null)}
                  onFocus={() => peekHand(k, id)}
                  onBlur={() => setHover(null)}
                />
              </div>
            ))}
          </div>
          {/* Pressable skills as a dense stack on the left of the fan. */}
          <SkillStack m={m} sess={sess} peek={peekCard} unpeek={() => setHover(null)} />
        </div>
        <div className={s.bar}>
          <PanelTab>{tr("board.hand", { n: m.v.hand.length, max: limit ?? "∞" })}</PanelTab>
          {/* Your draw pile, at the head of the hand it feeds (it used to sit in
              the board centre). Hover (or touch-hold) lists what is left; only a
              short tap opens the cards -- releasing a hold is not a tap.
              Beside it, on the compact canvas only: the 托管 toggle, which the
              TopBar title has no room for there (`skin="host"` wears this
              bar's chip chrome). */}
          <div className={s.handTools}>
            <button type="button" className={s.deck} title={tr("board.drawPile")} onFocus={() => setPeek(true)} onBlur={() => setPeek(false)} onClick={() => { setPeek(false); showDeck(m); }} {...holdPeek(() => setPeek(true), () => setPeek(false))}>
              <img src={sceneImg("card_back")} alt="" /><b>{m.me.draw}</b>
            </button>
            {compact && <AutoToggle sess={sess} skin="host" className={s.autoChip} />}
            <div className={s.thinkingSlot}><ThinkingPill sess={sess} /></div>
          </div>
        </div>
        {/* Hovering or focusing a card reveals the full card above the raised
            hand -- unless the match screen's standing panel is showing it. */}
        <div className={cx(s.preview, hover && !stand && s.previewOn)}>
          {hover && !stand && (
            <>
              <div className={s.pArt} style={{ borderColor: hc ? bandColor(hc.group) : "#ED4E76" }}><img src={cardArt(hover.id)} alt="" /></div>
              <div className={s.pTitle}>{cardTitle(hover.id)}</div>
              {!!hc?.tags.length && <div className={s.pTags}>{hc.tags.map((t) => <TagChip key={t} tag={t} />)}</div>}
              <div className={s.pText} ref={textRef}><SkillBody blocks={hover.text ? undefined : hc?.body} text={hover.text ?? hc?.text ?? ""} /></div>
              {hover.note && <div className={s.pNote}>{hover.note}</div>}
            </>
          )}
        </div>
      </div>
      {/* The draw pile, alphabetical -- never in the order it will be drawn. */}
      <div className={cx(s.peek, peek && s.peekOn)}>
        <div className={s.peekHead}>{tr("board.drawPile")}<b>×{m.me.draw}</b></div>
        {deck.length
          ? deck.map((id, k) => <div key={k} className={s.peekRow}><img src={cardArt(id)} alt="" /><span>{cardTitle(id)}</span></div>)
          : <div className={s.peekEmpty}>{tr("board.drawPileEmpty")}</div>}
      </div>
    </>
  );
});

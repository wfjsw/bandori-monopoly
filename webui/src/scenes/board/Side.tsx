// Compact turn controls at the bottom left; the Hand is docked below the map.

import { useRef, useState, type CSSProperties } from "react";
import { cardArt, sceneImg } from "../../core/assets";
import { cx } from "../../core/cx";
import { D, cardTitle, cardText } from "../../core/data";
import { useAutoplay, useSessionOther } from "../../core/hooks";
import { useEventListener, useHotkeys } from "../../hooks/dom";
import { useDockRaise } from "../../hooks/dock";
import { useResetScroll } from "../../hooks/measure";
import type { GameSession } from "../../game/session";
import { AutoToggle, ThinkingPill, autoFloat } from "../../ui/AutoToggle";
import { Btn } from "../../ui/Button";
import { CardFace, type CardAction, showCard, TagChip } from "../../ui/Card";
import { bandColor } from "../../ui/Character";
import { PanelTab } from "../../ui/Chips";
import { Icon } from "../../ui/Icon";
import { usePromptSheet } from "../../ui/Modal";
import { useStandingMode, stickCard } from "../../ui/CardPreview";
import { SkillBody } from "../../ui/SkillBody";
import type { Animator } from "./anim";
import { act, type Model } from "./model";
import { byTitle, showDeck, showDeedList } from "./Popups";
import { BOARD_MID, SKILL_CARD_W, SKILL_FAN_GAP, SKILL_STEP } from "../../styles/layout";
import { movementControl } from "./turnFlow";
import s from "./Side.module.css";
import { t as tr } from "../../i18n/t";
import { fmtMsg } from "../../i18n/msg";
import { namesOf, stateOf } from "../../core/names";

export function Side({ m, sess, anim }: { m: Model; sess: GameSession; anim: Animator }) {
  const auto = useAutoplay(sess);
  // A replay is read-only: every action control (skill, roll / continue,
  // mortgage, redeem) is hidden, not disabled, and the grid collapses away --
  // the left column is just the players. Inspection (tiles, cards, log) stays.
  if (sess.readOnly) return null;
  const S = m.S;
  const cur = S.players[S.turn];
  const names = namesOf(S);
  const control = movementControl(m, { auto, animating: anim.animating, readOnly: sess.readOnly, connected: sess.connected });
  const can = control !== null;
  const advance = control === "end";

  let hint: string;
  if (S.phase !== "play") hint = "";
  else if (m.out) hint = m.me.bankrupt ? tr("board.spectating") : tr("board.youLeft");
  else if (S.turn < 0) hint = m.asking ? tr("board.redrawAsk") : tr("board.startingSoon");
  else if (S.busy || anim.animating) hint = tr("board.settling");
  else if (S.roller === m.playerId && !m.myTurn && S.step === 2) hint = tr("board.rolledFor", { who: names.playerId(S.turn) });
  else if (m.myTurn && S.step === 2 && S.skipMove) hint = tr("board.stayHint");
  else if (advance) hint = tr("board.moved");
  else if (m.myTurn && S.step === 2 && S.roller !== m.playerId && S.roller >= 0) hint = tr("board.waitRoller", { who: names.playerId(S.roller) });
  else if (m.myTurn) hint = S.step === 2 ? tr("board.clickRoll") : S.step >= 3 ? tr("board.moved") : tr("board.turnStart");
  else hint = tr("board.waiting", { who: names.playerId(S.turn), bot: cur?.bot ? tr("board.botSuffix") : "" });

  const caption = advance ? tr(S.skipMove ? "board.skipMovement" : "board.continueTurn") : tr("board.dice");
  return (
    <div className={s.actions}>
      {/* The die itself lives in the board's roll zone (Ring). This slot keeps
          only the "skip movement / continue" step, and collapses away when
          there is nothing to advance. */}
      {advance && (
        <button type="button" className={cx(s.dice, can && s.can)} title={hint} disabled={!can} onClick={() => control && void act(sess, { act: control })}>
          <Icon name="arrow_forward" className={s.advanceIcon} />
          <div className={s.diceCap}>{caption}</div>
          <div className={s.diceHint}>{hint}</div>
        </button>
      )}
      <Btn icon="account_balance" className={s.act} disabled={auto} onClick={() => showDeedList(sess, false)}>{tr("board.mortgageDeeds")}</Btn>
      <Btn icon="redo" className={s.act} disabled={auto} onClick={() => showDeedList(sess, true)}>{tr("board.redeemDeeds")}</Btn>
    </div>
  );
}

/** Only appears when somebody has already requested an online settlement. */
export function SettleVote({ m, sess }: { m: Model; sess: GameSession }) {
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
}

/**
 * One pressable skill on the stack. `id` / `enabled` / `reason` come from the
 * engine's `MatchView.skills` (gated through the same `why_not_act` the
 * `act: "skill"` request takes). Name / body resolve from the rule id so the
 * skill-text simple/full setting applies.
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
 * to the top of the stack and feeds the same standing preview the hand uses;
 * click uses the skill directly. A click that cannot use (disabled, 托管,
 * replay, a prompt sheet up, or a use already in flight) is inspect-only,
 * matching how a hand card behaves while a sheet owns the bottom edge.
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
  // One use in flight: a second click before the engine answers is dropped.
  const [using, setUsing] = useState(false);
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
      <div className={s.skillHead}>{tr("board.skillCards", { n: slots.length })}</div>
      <div className={s.skillRow}>
        {slots.map((a) => {
          const usable = a.enabled && !auto && !sess.readOnly && !sheet.open;
          return (
            <div
              key={a.id}
              className={cx(s.skillSlot, !a.enabled && s.skillOff)}
              title={a.tip}
            >
              <CardFace
                id={a.id}
                size="tile"
                width={SKILL_CARD_W}
                className={s.skillCard}
                onClick={() => {
                  if (using) return;
                  if (usable) return void fire(a.id);
                  showCard(a.id, [], a.enabled ? a.text : a.reason);
                }}
                onMouseEnter={() => peek(a.id, a.enabled ? "" : a.reason, a.text)}
                onMouseLeave={unpeek}
                onFocus={() => peek(a.id, a.enabled ? "" : a.reason, a.text)}
                onBlur={unpeek}
              />
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
export function Hand({ m, sess, busy }: { m: Model; sess: GameSession; busy: boolean }) {
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
  const limit = stateOf(m.me, "handLimit") || 5;
  const canPlay = m.myTurn && S.step === 2 && !S.busy && !m.asking && !busy && !auto;
  // Hovering a card lifts it and feeds the standing card panel.
  const peekCard = (id: string, note: string, text?: string) => {
    setHover({ id, note, text });
    stickCard(id, note);
  };
  const peekHand = (k: number, id: string) => peekCard(id, fmtMsg(m.v.handNotes[k], namesOf(S)));
  const detail = (id: string, k: number) => {
    setHover(null);
    const note = fmtMsg(m.v.handNotes[k], namesOf(m.S));
    // Replay (and a live prompt sheet): inspection only -- no play / discard.
    if (sess.readOnly || sheet.open) return void showCard(id, [], note);
    // Per-card `playable` (online extras / solo `view_extra`): gate the play
    // action. Absent = unknown = keep the coarse gate's behaviour.
    const cardPlayable = m.v.playable?.[k] !== false;
    const acts: CardAction[] = [];
    if (m.overHand) acts.push({ label: tr("board.discardThis"), enabled: !auto, kind: "white", run: () => act(sess, { act: "discard", card: id }) });
    acts.push({
      label: canPlay ? tr("board.play") : tr("board.playOnlyOps"),
      enabled: canPlay && cardPlayable,
      run: () => act(sess, { act: "play", card: id }),
    });
    showCard(id, acts, note);
  };
  const hc = hover ? D.card(hover.id) : undefined;
  const deck = byTitle(m.v.draw ?? []);
  // Left skill stack: its width is reserved out of the fan's budget so the two
  // never collide. `--skill-stack-w` pads the fan's centring box; `spread`
  // tightens the fan's own slot step to whatever is left of the centre column.
  const skillN = skillSlots(m).length;
  const skillStep = skillN > 1 ? Math.min(SKILL_STEP, (240 - SKILL_CARD_W) / (skillN - 1)) : SKILL_STEP;
  const skillW = skillN ? SKILL_CARD_W + (skillN - 1) * skillStep + SKILL_FAN_GAP : 0;
  const fanSpread = Math.min(660, Math.max(0, BOARD_MID - skillW - 120));
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
        style={{ ["--skill-stack-w" as string]: `${skillW}px`, ["--skill-step" as string]: `${skillStep}px` }}
        ref={dockRef}
        {...dock.hover}
      >
        <div className={s.fan} style={{ ["--fan-rise" as string]: fanRise(m.v.hand.length, fanSpread) }}>
          {m.v.hand.map((id, k) => (
            // Per-card `playable` greys a card only while the coarse gate is
            // on (otherwise the whole hand is inspect-only anyway). Absent
            // `playable` (solo before merge / failed online extras) keeps the
            // card looking normal.
            <div
              key={`${id}:${k}`}
              className={cx(s.fanCard, canPlay && m.v.playable?.[k] === false && s.cardOff)}
              style={fanArc(m.v.hand.length, k, fanSpread) as CSSProperties}
            >
              <CardFace
                id={id}
                size="mini"
                title={cardTitle(id)}
                onClick={() => detail(id, k)}
                onMouseEnter={() => peekHand(k, id)}
                onMouseLeave={() => setHover(null)}
                onFocus={() => peekHand(k, id)}
                onBlur={() => setHover(null)}
              />
            </div>
          ))}
        </div>
        <div className={s.bar}>
          <PanelTab>{tr("board.hand", { n: m.v.hand.length, max: limit })}</PanelTab>
          <span className={cx(s.handHint, m.overHand && s.warn)}>{m.overHand ? tr("board.overHand") : canPlay ? tr("board.canPlay") : tr("board.tapCard")}</span>
          {/* Your draw pile, at the head of the hand it feeds (it used to sit in
              the board centre). Hover lists what is left; click opens the cards. */}
          <div className={cx(s.handTools, !sess.readOnly && s.withAuto)}>
            <button type="button" className={s.deck} title={tr("board.drawPile")} onMouseEnter={() => setPeek(true)} onMouseLeave={() => setPeek(false)} onFocus={() => setPeek(true)} onBlur={() => setPeek(false)} onClick={() => { setPeek(false); showDeck(m); }}>
              <img src={sceneImg("card_back")} alt="" /><b>{m.me.draw}</b>
            </button>
            <div className={cx(autoFloat, s.autoSlot)}>
              {!sess.readOnly && <AnimationSpeed sess={sess} />}
              <AutoToggle sess={sess} compact />
            </div>
            <div className={s.thinkingSlot}><ThinkingPill sess={sess} /></div>
          </div>
        </div>
        {/* Hovering or focusing a card reveals the full card above the raised
            hand -- unless the match screen's standing panel is showing it. */}
        <div className={cx(s.preview, hover && !stand && s.previewOn)}>
          {hover && !stand && (
            <>
              <div className={s.pArt} style={{ borderColor: hc ? bandColor(hc.band) : "#ED4E76" }}><img src={cardArt(hover.id)} alt="" /></div>
              <div className={s.pTitle}>{cardTitle(hover.id)}</div>
              {!!hc?.tags.length && <div className={s.pTags}>{hc.tags.map((t) => <TagChip key={t} tag={t} />)}</div>}
              <div className={s.pText} ref={textRef}><SkillBody text={hover.text ?? hc?.text ?? ""} /></div>
              {hover.note && <div className={s.pNote}>{hover.note}</div>}
            </>
          )}
        </div>
        {/* Pressable skills as a dense stack on the left of the fan. */}
        <SkillStack m={m} sess={sess} peek={peekCard} unpeek={() => setHover(null)} />
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
}

function AnimationSpeed({ sess }: { sess: GameSession }) {
  useSessionOther(sess);
  return (
    <div className={s.speeds} role="group" aria-label={tr("board.animationSpeed")}>
      {[1, 2, 4].map((speed) => (
        <button key={speed} type="button" className={cx(s.speed, sess.animSpeed === speed && s.speedOn)} aria-pressed={sess.animSpeed === speed} title={tr("board.animationSpeedAt", { n: speed })} onClick={() => sess.setAnimSpeed(speed)}>{speed}×</button>
      ))}
    </div>
  );
}

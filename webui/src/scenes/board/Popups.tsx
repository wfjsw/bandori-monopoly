// Board popups: deed card (DeedCardView), deed lists (DeedListView), skills,
// player info, event / discard piles, settle and leave. Popups that act on the
// match read the live state themselves, so they stay correct while open.

import { useEffect, useState } from "react";
import { sceneImg } from "../../core/assets";
import { cx } from "../../core/cx";
import { D, cardTitle, skillText } from "../../core/data";
import { isLight, n0, plain } from "../../core/format";
import { useAutoplay, useMatchView } from "../../core/hooks";
import type { MatchState } from "../../core/types";
import type { GameSession } from "../../game/session";
import { Btn } from "../../ui/Button";
import { CardFace, showCard } from "../../ui/Card";
import { Avatar } from "../../ui/Character";
import { openModal } from "../../ui/Modal";
import { SkillBody } from "../../ui/SkillBody";
import { act, buyable, canBuildOn, type Model, model, mortgageValue, RING_MULTIPLIER, redeemCost } from "./model";
import s from "./Popups.module.css";
import { t as tr } from "../../i18n/t";
import { fmtMsg } from "../../i18n/msg";
import { namesOf, stateOf, stateMax } from "../../core/names";

function useModel(sess: GameSession): Model | null {
  const { view } = useMatchView(sess);
  return view ? model(view) : null;
}

const kindLabel = (kind: string) => ({ ring: "RiNG", agent: tr("deed.dealer"), circle: tr("deed.start"), cafe: tr("deed.cafe"), edogawa: tr("deed.draw"), ryuseido: tr("deed.cafe") } as Record<string, string>)[kind] ?? tr("deed.land");

function deedAction(m: Model, i: number): { label: string; cmd: { act: string; value: number }; enabled: boolean } | null {
  const S = m.S;
  if (S.phase !== "play") return null;
  const t = D.tiles[i];
  if (buyable(m, i)) {
    const price = S.buyPrice >= 0 ? S.buyPrice : t.price + (S.houses[i] ?? 0) * t.house;
    const poor = m.me.money < price ? tr(S.canBuyHere ? "deed.fundByMortgage" : "deed.poor") : "";
    return { label: tr("deed.buy", { n: n0(price), poor }), cmd: { act: "buy", value: i }, enabled: S.canBuyHere };
  }
  if (S.owners[i] !== m.playerId) return null;
  if (canBuildOn(m, i)) return { label: tr("deed.buildN", { n: (S.houses[i] ?? 0) + 1, cost: n0(t.house) }), cmd: { act: "build", value: i }, enabled: S.canBuildHere };
  if (!S.mortgaged[i] && t.kind !== "ring" && m.myTurn) return { label: tr("deed.mortgage", { n: n0(mortgageValue(i)) }), cmd: { act: "mortgage", value: i }, enabled: true };
  if (S.mortgaged[i] && m.myTurn) return { label: tr("deed.redeem", { n: n0(redeemCost(i)) }), cmd: { act: "redeem", value: i }, enabled: true };
  return null;
}

function Deed({ sess, i, close }: { sess: GameSession; i: number; close: () => void }) {
  const m = useModel(sess);
  const auto = useAutoplay(sess); // 托管: the buy/build/mortgage/redeem button is locked
  // A new turn (or the end of the match) closes the deed card.
  const turnKey = m ? `${m.S.phase}:${m.S.round}:${m.S.turn}` : "";
  const [openedAt] = useState(turnKey);
  useEffect(() => {
    if (turnKey !== openedAt) close();
  }, [turnKey, openedAt, close]);
  if (!m) return null;
  const S = m.S;
  const t = D.tiles[i];
  const owner = S.owners[i] ?? -1;
  const houses = S.houses[i] ?? 0;
  const mortgaged = !!S.mortgaged[i];
  const deedKind = t.kind === "property" || t.kind === "ring";
  const notes: string[] = [];
  switch (t.kind) {
    case "circle": notes.push(tr("deed.note.circle")); break;
    case "cafe": case "ryuseido": notes.push(tr("deed.note.drawEvent")); break;
    case "edogawa": notes.push(tr("deed.note.draw")); break;
    case "agent": notes.push(tr("deed.note.dealer")); break;
    case "ring":
      notes.push(tr("deed.rentRing", { mult: RING_MULTIPLIER }));
      for (let k = 1; k <= 4; k++) notes.push(tr("deed.rentRingN", { n: k, min: n0(k * RING_MULTIPLIER), max: n0(k * RING_MULTIPLIER * 20) }));
      break;
  }
  if (owner < 0 && houses > 0) notes.unshift(tr("deed.note.leftHouses", { n: houses, total: n0(t.price + houses * t.house) }));
  const names = namesOf(S);
  const marks = (S.marks ?? []).filter((x) => x.tile === i).map((x) => {
    const who = x.owner >= 0 ? tr("deed.markOwner", { who: names.playerId(x.owner) }) : "";
    const note = x.note?.k ? fmtMsg(x.note, names) : "";
    return x.kind === "card"
      ? `${who}${tr("common.quotes", { x: cardTitle(x.card) })}${note ? tr("common.noteColon", { x: note }) : ""}`
      : `${who}${fmtMsg({ k: x.kind }, names)}${x.count > 1 ? ` ×${x.count}` : ""}${note ? tr("common.paren", { x: note }) : ""}`;
  });
  const art = sceneImg(`area_${t.area}`);
  const a = deedAction(m, i);
  const head = deedKind || t.kind === "agent" ? t.color : "#ED4E76";
  return (
    <div className={s.deed}>
      <div className={s.deedHead} style={{ background: head, color: isLight(head) ? "var(--text)" : "#fff" }}>
        <b>{plain(t.name)}</b>
        <span>{kindLabel(t.kind) + (t.tier > 0 && t.kind === "property" ? tr("deed.tier", { n: t.tier }) : "")}</span>
      </div>
      {art && <img className={s.deedArt} src={art} alt="" />}
      {deedKind && (
        <div className={s.deedGrid}>
          <div><small>{tr("deed.price")}</small><b>{t.price ? n0(t.price) : "—"}</b></div>
          <div><small>{tr("deed.house")}</small><b>{t.kind === "ring" ? tr("deed.noBuild") : t.house ? tr("deed.perHouse", { n: n0(t.house) }) : "—"}</b></div>
          <div className={s.wide}><small>{tr("prompt.mortgage")}</small><b>{mortgaged ? tr("deed.mortgagedLine", { n: n0(redeemCost(i)) }) : t.price && t.kind !== "ring" ? tr("deed.mortgageLine", { mortgage: n0(t.price / 2), redeem: n0(redeemCost(i)) }) : tr("deed.noMortgage")}</b></div>
          <div className={s.wide}><small>{tr("deed.owner")}</small><b>{owner >= 0 ? tr("deed.ownerLine", { who: names.playerId(owner), mort: mortgaged ? tr("common.mortgagedTag") : "" }) : tr("deed.unowned")}</b></div>
        </div>
      )}
      {t.rent.length >= 4 && (
        <div className={s.rent}>
          {t.rent.map((r, k) => <div key={k} className={cx(s.rentCell, owner >= 0 && k === houses && s.rentOn)}><span>{k === 0 ? tr("deed.empty") : tr("deed.housesN", { n: k })}</span><b>{n0(r)}</b></div>)}
        </div>
      )}
      {marks.length > 0 && <div className={cx(s.note, s.marks)}>{tr("deed.marks", { marks: marks.join(tr("common.listSep")) })}</div>}
      {notes.length > 0 && <div className={s.note}>{notes.join("\n")}</div>}
      {/* Replay: the deed card is inspection only -- the act button is hidden. */}
      {a && !sess.readOnly && <Btn kind="pink" className={s.deedAct} disabled={!a.enabled || auto} onClick={async () => { if (await act(sess, a.cmd)) close(); }}>{a.label}</Btn>}
    </div>
  );
}

export function openDeed(sess: GameSession, i: number): void {
  openModal(plain(D.tiles[i].name), (close) => <Deed sess={sess} i={i} close={close} />, { size: "small", key: "deed" });
}

function DeedList({ sess, redeem, close }: { sess: GameSession; redeem: boolean; close: () => void }) {
  const m = useModel(sess);
  const auto = useAutoplay(sess); // 托管
  if (!m) return null;
  const S = m.S;
  const mine = D.tiles.map((_, i) => i).filter((i) => S.owners[i] === m.playerId && !!S.mortgaged[i] === redeem && (redeem || D.tiles[i].kind !== "ring"));
  return (
    <div className={s.list}>
      <p className={s.hint}>
        {redeem ? tr("deedList.redeemHint")
          : tr("deedList.mortgageHint")}
      </p>
      {mine.length ? (
        <div className={s.rows}>
          {mine.map((i) => {
            const t = D.tiles[i];
            return (
              <div key={i} className={s.row}>
                <i className={s.strip} style={{ background: t.color }} />
                <div className={s.rowName}><b>{plain(t.name)}</b><small>{tr("deedList.deedInfo", { price: n0(t.price), houses: S.houses[i] ? tr("deedList.housesN", { n: S.houses[i] }) : "", mortgaged: redeem ? tr("deed.mortgagedTag") : "" })}</small></div>
                {!sess.readOnly && (
                  <Btn size="small" kind={redeem ? "white" : "pink"} disabled={auto} onClick={async () => { if (await act(sess, { act: redeem ? "redeem" : "mortgage", value: i })) close(); }}>
                    {redeem ? tr("deed.redeem", { n: n0(redeemCost(i)) }) : tr("deed.mortgage", { n: n0(mortgageValue(i)) })}
                  </Btn>
                )}
              </div>
            );
          })}
        </div>
      ) : <div className={s.empty}>{redeem ? tr("deedList.emptyRedeem") : tr("deedList.emptyMortgage")}</div>}
      <div className={s.status}>{tr("deedList.money", { n: n0(m.me.money) })}</div>
    </div>
  );
}

export function showDeedList(sess: GameSession, redeem: boolean): void {
  openModal(redeem ? tr("board.redeemDeeds") : tr("board.mortgageDeeds"), (close) => <DeedList sess={sess} redeem={redeem} close={close} />, { size: "mid" });
}

function Skills({ sess, close }: { sess: GameSession; close: () => void }) {
  const m = useModel(sess);
  const auto = useAutoplay(sess); // 托管
  const list = m?.me.actions ?? [];
  if (!list.length) return <div className={s.empty}>{tr("skills.none")}</div>;
  return (
    <div className={s.rows}>
      {list.map((a) => (
        <div key={a.id} className={s.row}>
          <div className={s.rowName}><b>{fmtMsg(a.title, namesOf(sess.view?.state))}<small> {a.source}</small></b><p>{fmtMsg(a.text, namesOf(sess.view?.state))}</p></div>
          {/* Replay: skill text stays readable, the use button is hidden. */}
          {!sess.readOnly && (
            <Btn kind="pink" size="small" disabled={!a.enabled || auto} title={fmtMsg(a.reason, namesOf(sess.view?.state))} onClick={async () => { if (await act(sess, { act: "skill", card: a.id })) close(); }}>{a.enabled ? tr("common.use") : a.reason?.k ? fmtMsg(a.reason, namesOf(sess.view?.state)) : tr("skills.unavailable")}</Btn>
          )}
        </div>
      ))}
    </div>
  );
}

export function showSkills(sess: GameSession): void {
  openModal(tr("board.useSkill"), (close) => <Skills sess={sess} close={close} />, { size: "mid" });
}

export function showPlayerInfo(m: Model, i: number): void {
  const S = m.S;
  const x = S.players[i];
  const c = m.charOf(i);
  const band = c ? D.band(c.band) : undefined;
  const deeds = D.tiles.map((_, k) => k).filter((k) => S.owners[k] === i);
  const lines = [
    tr("player.handStats", { money: n0(x.money), hand: x.hand, fire: stateOf(x, "fire"), fireMax: stateMax(x, "fire") }),
    tr("player.zones", { draw: x.draw, discard: x.discard.length }),
    stateOf(x, "stay") ? tr("player.stayN", { n: stateOf(x, "stay") }) : "", stateOf(x, "stun") + stateOf(x, "stunStart") ? tr("player.stunned") : "", stateOf(x, "exile") ? tr("player.exileN", { n: stateOf(x, "exile") }) : "",
    x.skillNote?.k ? tr("player.skillNote", { note: fmtMsg(x.skillNote, namesOf(S)) }) : "",
    ...(x.tokens ?? []).map((tok) => `${tok.name} ×${tok.value}`),
  ].filter(Boolean);
  openModal(`${namesOf(S).playerId(i)}${i === m.playerId ? tr("common.youSuffix") : ""}`, (
    <div className={s.info}>
      <div className={s.infoTop}>
        <Avatar c={c} size={92} />
        <div>
          <b>{c?.display ?? "—"}</b><small>{c?.band ?? ""}</small>
          {/* The two skill stand-ins `bind_skills` puts on the field (the
              character's own skill and its band's) belong here, not on the
              board: name in pink, body under it. */}
          <div className={s.skills}>
            {c && <SkillBody prefix={<span className={s.pink}>{c.skill}</span>} text={skillText(c)} />}
            {band && <SkillBody prefix={<span className={s.pink}>{band.skill}</span>} text={skillText(band)} />}
          </div>
        </div>
      </div>
      <div className={s.lines}>{lines.map((l) => <div key={l}>{l}</div>)}</div>
      {deeds.length ? (
        <div className={s.deeds}>
          {deeds.map((k) => <span key={k} className={s.deedChip} style={{ borderColor: D.tiles[k].color }}>{plain(D.tiles[k].name)}{S.houses[k] ? tr("player.deedHouse", { n: S.houses[k] }) : ""}{S.mortgaged[k] ? tr("player.deedMortgaged") : ""}</span>)}
        </div>
      ) : <div className={s.muted}>{tr("player.noDeeds")}</div>}
    </div>
  ), { size: "mid" });
}

/** Which effect a card just applied -- a popup window, in addition to the log
 *  line the same message already produced. Auto-closes after `ms`: these fire
 *  mid-resolution (a card can land three in a row), so the player should see
 *  each one without having to dismiss it. */
export function showEffect(body: string, ms: number): void {
  const close = openModal(tr("anim.effect"), <div className={s.info}><p className={s.pre}>{body}</p></div>, { size: "mid" });
  window.setTimeout(close, ms);
}

export function showEvent(id: string, note: string): void {
  openModal(tr("events.label", { id }), <div className={s.info}><p className={s.pre}>{D.event(id)?.text ?? ""}</p>{note && <p className={s.pink}>{note}</p>}</div>, { size: "mid" });
}

export function showEventPile(S: MatchState): void {
  openModal(tr("board.eventDeck"), (
    <div className={s.info}>
      <p>{tr("events.left", { n: S.eventDeck })}</p>
      {S.eventActive?.length ? (
        <div className={s.rows}>
          <b>{tr("events.active")}</b>
          {S.eventActive.map((e) => <Btn key={e.id} size="small" onClick={() => showEvent(e.id, fmtMsg(e.note, namesOf(S)))}>{e.id}{e.note?.k ? ` · ${fmtMsg(e.note, namesOf(S))}` : ""}</Btn>)}
        </div>
      ) : <p className={s.muted}>{tr("events.none")}</p>}
      {S.eventDiscard?.length > 0 && <p className={s.muted}>{tr("events.settled", { list: S.eventDiscard.slice(-8).join(tr("common.listSep")) })}</p>}
    </div>
  ), { size: "mid" });
}

/** A pile's cards alphabetical by name -- how both the graveyard and the draw
 *  pile are shown, so the draw pile's own order never leaks. */
export const byTitle = (ids: readonly string[]): string[] => [...ids].sort((a, b) => cardTitle(a).localeCompare(cardTitle(b)));

/** One player's graveyard, alphabetical by name -- the same treatment the draw
 *  pile gets. `showGraveyard` used to open every player's at once from the
 *  board centre; each player module now carries its own indicator. */
export function showGraveyard(m: Model, playerId: number): void {
  const cards = byTitle(m.S.players[playerId]?.discard ?? []);
  openModal(tr("board.graveyardPile"), cards.length ? (
    <div className={s.pile}>
      <div className={s.pileRow}>
        <div className={s.who}><Avatar c={m.charOf(playerId)} size={30} /><span>{m.nameOf(playerId)}</span></div>
        <div className={s.pileCards}>{cards.map((id, k) => <CardFace key={k} id={id} onClick={() => showCard(id)} />)}</div>
      </div>
    </div>
  ) : <div className={s.empty}>{tr("graveyard.empty")}</div>, { size: "wide" });
}

/** Every card in play (场上的卡), grouped by the player whose field it is on,
 *  drawn as card faces like the hand and graveyard. Skill rules (`skill:` ids) are bound on the field
 *  but are not cards, so they are left out; a face-down card stays face down.
 *  The active events (生效中的事件) are listed under the cards. */
export function showField(m: Model): void {
  const S = m.S;
  const names = namesOf(S);
  const rows = S.players
    .map((x, i) => [i, (x.field ?? []).filter((c) => !c.card.startsWith("skill:"))] as const)
    .filter(([, f]) => f.length);
  const events = S.eventActive ?? [];
  openModal(tr("board.field"), rows.length || events.length ? (
    <div className={s.pile}>
      {rows.map(([i, f]) => (
        <div key={i} className={s.pileRow}>
          <div className={s.who}><Avatar c={m.charOf(i)} size={30} /><span>{m.nameOf(i)}</span></div>
          <div className={s.pileCards}>
            {f.map((fc) => {
              const note = fc.note ? fmtMsg(fc.note, names) : [fc.crystals ? tr("board.crystals", { n: fc.crystals }) : "", fc.cp > 0 ? tr("board.cp", { n: fc.cp }) : ""].filter(Boolean).join(" · ");
              return fc.faceDown ? (
                <div key={fc.uid} className={s.fieldSlot}>
                  <div className={s.faceDown}><img src={sceneImg("card_back")} alt="" /></div>
                  <small>{tr("board.faceDown")}</small>
                </div>
              ) : (
                <div key={fc.uid} className={s.fieldSlot}>
                  <CardFace id={fc.card} onClick={() => showCard(fc.card, [], note)} />
                  {note && <small>{note}</small>}
                </div>
              );
            })}
          </div>
        </div>
      ))}
      {events.length > 0 && (
        <div className={s.pileRow}>
          <div className={s.who}><span>{tr("board.activeEvents")}</span></div>
          <div className={s.pileCards}>
            {events.map((e) => <Btn key={e.id} size="small" onClick={() => showEvent(e.id, fmtMsg(e.note, names))}>{tr("events.label", { id: e.id })}{e.counter ? ` ×${e.counter}` : ""}</Btn>)}
          </div>
        </div>
      )}
    </div>
  ) : <div className={s.empty}>{tr("board.fieldEmpty")}</div>, { size: "wide" });
}

/** What is left in the player's own draw pile, alphabetical by name.
 *  Mirrors {@link showGraveyard} -- the pile's own draw order is never shown. */
export function showDeck(m: Model): void {
  const cards = byTitle(m.v.draw ?? []);
  openModal(tr("board.drawPile"), cards.length ? (
    <div className={s.pile}>
      <div className={s.pileRow}>
        <div className={s.who}><Avatar c={m.charOf(m.playerId)} size={30} /><span>{m.nameOf(m.playerId)}</span></div>
        <div className={s.pileCards}>{cards.map((id, k) => <CardFace key={k} id={id} onClick={() => showCard(id)} />)}</div>
      </div>
    </div>
  ) : <div className={s.empty}>{tr("board.drawPileEmpty")}</div>, { size: "wide" });
}

function LeaveConfirm({ sess, close, exit }: { sess: GameSession; close: () => void; exit: () => void }) {
  const auto = useAutoplay(sess); // 托管: settle / forfeit are locked
  const solo = sess.kind !== "online";
  return (
    <div className={s.confirm}>
      <p>{solo ? tr("board.leaveSoloText") : tr("board.leaveOnlineText")}</p>
      <div className={s.btns}>
        {solo ? <Btn onClick={() => { close(); exit(); }}>{tr("board.leaveStay")}</Btn> : <Btn onClick={close}>{tr("board.continue")}</Btn>}
        <Btn kind="blue" disabled={auto} onClick={async () => { if (await act(sess, { act: "vote", value: 1 })) close(); }}>{solo ? tr("board.settleOk") : tr("board.voteStart")}</Btn>
        <Btn kind="pink" disabled={auto} onClick={async () => { close(); await act(sess, { act: "leave" }); exit(); }}>{tr("board.leaveForfeit")}</Btn>
      </div>
    </div>
  );
}

export function showLeave(sess: GameSession, exit: () => void): void {
  openModal(tr(sess.kind === "online" ? "board.leaveVoteTitle" : "board.leaveSettleTitle"), (close) => <LeaveConfirm sess={sess} close={close} exit={exit} />, { size: "mid", key: "leave" });
}

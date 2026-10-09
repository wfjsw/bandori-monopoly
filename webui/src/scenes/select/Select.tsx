// Character select (CharacterSelectController): turn order, ban (Ranked), pick
// and the tr("deckPick.title") deck popup. Left: preview of the highlighted character;
// right: band chips + card grid; bottom: turn order strip and the big button.

import { useEffect, useRef, useState } from "react";
import { navigate } from "../../app/router";
import { sfx } from "../../core/audio";
import { cx } from "../../core/cx";
import { D, skillText } from "../../core/data";
import { useAutoplay, useMatchView, useTick, useWakeLock } from "../../core/hooks";
import { useLatestRef } from "../../hooks/latest";
import { getProfile } from "../../core/store";
import type { CharacterData, Command } from "../../core/types";
import { endSession, type GameSession, SoloSession } from "../../game/session";
import { AutoBanner, AutoToggle, autoFloat } from "../../ui/AutoToggle";
import { Btn } from "../../ui/Button";
import { SkillBody } from "../../ui/SkillBody";
import { SkillTextToggle } from "../../ui/SkillTextToggle";
import { Avatar, BandMark, CharCard, inTab, NamePlate, tabLabels } from "../../ui/Character";
import { Chips, PanelTab } from "../../ui/Chips";
import { Icon } from "../../ui/Icon";
import { Live2DStand } from "../../ui/Live2DStand";
import { isModalOpen } from "../../ui/Modal";
import { toast } from "../../ui/Toast";
import { fmtMsg } from "../../i18n/msg";
import { namesOf } from "../../core/names";
import { TopBar } from "../../ui/TopBar";
import { showDeckPick } from "./DeckPick";
import s from "./Select.module.css";
import { t as tr } from "../../i18n/t";

const modeChip = (mode: number): [string, string] => ([[tr("mode.solo"), "solo"], [tr("mode.casual"), "casual"], [tr("mode.ranked"), "ranked"]] as [string, string][])[mode] ?? [tr("mode.solo"), "solo"];

export function Select({ sess }: { sess: GameSession }) {
  const { view, at } = useMatchView(sess);
  useTick(500);
  const auto = useAutoplay(sess); // 托管: ban / pick / deck input is locked
  useWakeLock(true); // a match is on from the ban / pick phase
  const [tab, setTab] = useState<string>(tr("common.all"));
  const [picked, setPicked] = useState("");
  const deckOpenedFor = useRef("");

  const act = async (cmd: Command) => {
    if (sess.autoMode !== "off") return false;
    const err = await sess.act(cmd);
    if (err) toast(fmtMsg(err, namesOf(sess.view?.state)), "error");
    return !err;
  };

  // Open tr("deckPick.title") once when the deck phase starts.
  // While 托管 is on the autopilot submits the preset deck itself, so the
  // modal is not opened (it would block the board for nothing).
  const me = view?.state.players[view.playerId];
  const actRef = useLatestRef(act);
  useEffect(() => {
    if (!view || !me || sess.autoMode !== "off") return;
    if (view.state.phase === "deck" && !me.deckReady && deckOpenedFor.current !== me.character && !isModalOpen("deck")) {
      deckOpenedFor.current = me.character;
      showDeckPick(sess, me.character, actRef.current);
    }
  });

  if (!view || !me) return null;
  const st = view.state;
  const myTurn = st.turn === view.playerId;
  const chosen = me.character || picked || getProfile().homeCharacter || D.characters[0].name;
  const preview = D.character(chosen);
  const secs = Math.max(0, Math.ceil(st.timeLeft - (performance.now() - at) / 1000));
  const takenBy = new Map(st.players.map((x, playerId) => [x.character, { name: x.player, playerId }] as const).filter(([c]) => c));
  const free = (n: string) => !st.bans.includes(n) && !takenBy.has(n);
  const cur = st.players[st.turn]?.player ?? "";
  const leave = () => {
    endSession();
    navigate(sess.kind === "solo" ? { name: "menu" } : { name: "lobby" });
  };

  // hint + big button
  let hint = "";
  let label = tr("select.decide");
  let onGo: (() => void) | null = null;
  if (st.phase === "order") {
    hint = tr("select.orderHint");
    label = tr("select.rolling");
  } else if (st.phase === "ban") {
    hint = myTurn ? tr("select.banHintYou") : tr("select.banHintWait", { who: cur });
    const ok = myTurn && picked && free(picked);
    label = myTurn ? (ok ? tr("select.stepBan") : tr("select.noBan")) : tr("select.banning");
    if (myTurn && !auto) onGo = () => void act({ act: "ban", character: ok ? picked : "" });
  } else if (st.phase === "pick") {
    hint = myTurn ? tr("select.pickHintYou") : tr("select.pickHintWait", { who: cur });
    if (myTurn && !auto && picked && free(picked)) onGo = () => void act({ act: "pick", character: picked });
  } else if (st.phase === "deck") {
    label = tr("select.stepDeck");
    hint = me.deckReady ? tr("select.deckDone") : tr("select.deckHint");
    if (!me.deckReady && !auto) onGo = () => showDeckPick(sess, me.character, act);
  }

  const [modeLabel, modeCls] = modeChip(st.mode);
  const stepIdx = { order: 0, ban: 1, pick: 2, deck: 3 }[st.phase] ?? 0;
  const step = (n: number, text: string, skip = false) => (
    <span className={cx(s.step, skip ? s.skip : n === stepIdx ? s.stepOn : n < stepIdx && s.stepDone)}>{n} {text}</span>
  );

  return (
    <>
      <TopBar
        section={tr("select.section")}
        title={st.phase === "deck" ? tr("select.deckTitle") : st.phase === "ban" ? tr("select.banTitle") : tr("select.pickTitle")}
        onBack={leave}
        right={
          <div className={s.steps}>
            <span className={cx(s.mode, s[modeCls])}>{modeLabel}</span>
            {step(1, tr("select.stepBan"), st.mode !== 2)}<i>›</i>{step(2, tr("select.stepPick"))}<i>›</i>{step(3, tr("select.stepDeck"))}
          </div>
        }
      />
      {/* Outside the TopBar so it can sit above modals: clickable at any time. */}
      <div className={cx(autoFloat, s.autoSlot)}><AutoToggle sess={sess} /></div>
      <AutoBanner sess={sess} />
      {sess instanceof SoloSession && st.phase !== "deck" && (
        <Btn size="small" icon="redo" className={s.quick} title={tr("select.quickHint")} disabled={auto} onClick={() => sess.quickStart()}>{tr("select.quickStart")}</Btn>
      )}
      <div className={s.body}>
        {preview && <Preview c={preview} />}

        <div className={s.right}>
          <div className={s.tabs}>
            <Chips items={tabLabels()} on={tab as (ReturnType<typeof tabLabels>)[number]} onPick={setTab} />
          </div>
          <div className={s.gridPanel}>
            <div className={s.grid}>
              {D.characters.filter((c) => inTab(c, tab)).map((c) => {
                const banned = st.bans.includes(c.name);
                const taken = takenBy.get(c.name);
                const mine = taken?.playerId === view.playerId;
                return (
                  <CharCard
                    key={c.name}
                    c={c}
                    dim={banned || (!!taken && !mine)}
                    chosen={c.name === chosen}
                    stamp={banned ? tr("select.stepBan") : taken ? (mine ? tr("common.you") : taken.name) : undefined}
                    stampKind={banned ? "gray" : "pink"}
                    check={c.name === chosen && (mine || myTurn)}
                    onClick={() => {
                      if (auto) return;
                      sfx("place");
                      if (!me.character) setPicked(c.name);
                    }}
                  />
                );
              })}
            </div>
          </div>

          <div className={s.order}>
            <PanelTab className={s.orderTab}>{tr("select.order")}</PanelTab>
            <div className={s.strip}>
              {st.players.map((x, i) => {
                const c = D.character(x.character);
                const turn = (st.phase === "ban" || st.phase === "pick") && st.turn === i;
                return (
                  <div key={i} className={cx(s.playerId, turn && s.turn)}>
                    <div className={s.av}>
                      <Avatar c={c} size={56} />
                      <span className={s.n}>{i + 1}</span>
                      {x.bot && <span className={s.bot}><Icon name="smart_toy" /></span>}
                      {st.phase === "deck" && x.deckReady && <span className={s.ok}><Icon name="check" /></span>}
                    </div>
                    <div className={s.name}>{i === view.playerId ? tr("common.youName", { name: x.player }) : x.player}{st.phase === "order" && x.roll > 0 && <b> {x.roll}</b>}</div>
                    {st.mode === 2 && x.ban && <div className={s.ban}>{tr("select.banMark", { name: D.character(x.ban)?.display ?? "" })}</div>}
                  </div>
                );
              })}
            </div>
          </div>

          <div className={s.hint}>{hint}{sess.kind !== "solo" && secs > 0 && tr("select.secsSuffix", { n: secs })}</div>
          <Btn kind="pink" className={s.go} disabled={!onGo} onClick={() => onGo?.()}>{label}</Btn>
        </div>
      </div>
    </>
  );
}

function Preview({ c }: { c: CharacterData }) {
  const [tab, setTab] = useState<"char" | "band">("char");
  const band = D.band(c.band);
  const char = tab === "char";
  return (
    <div className={s.left}>
      <div className={s.band}><BandMark band={c.band} /></div>
      {/* The eyes share one line across characters (the anchor ignores each
          model's height). `fit="top"` gives the head the room vertical
          centring wastes, and `headroom` clamps the tallest hairdos (004) so
          they drop a little rather than being sliced by the portrait rect. */}
      <Live2DStand key={D.artId(c)} id={D.artId(c)} className={s.stand} fit="top" zoom={1.22} headroom={0.03} eyeFrac={0.27} />
      <div className={s.plate}><NamePlate c={c} /></div>
      <div className={s.skill} style={{ ["--accent" as string]: c.color }}>
        <div className={s.skillHead}>
          <button type="button" className={cx(s.skillTab, char && s.skillTabOn)} onClick={() => setTab("char")}>{tr("select.skillChar")}</button>
          <button type="button" className={cx(s.skillTab, !char && s.skillTabOn)} onClick={() => setTab("band")}>{tr("select.skillBand")}</button>
          <b>{char ? c.skill || tr("select.skillTbd") : band?.skill || tr("select.bandSkillTbd")}</b>
          <span>{c.band}</span>
        </div>
        <div className={s.skillText}><SkillTextToggle className={s.skillSwitch} /><SkillBody text={skillText(char ? c : band)} /></div>
      </div>
    </div>
  );
}


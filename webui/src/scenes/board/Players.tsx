// Left column: player panels (PlayerPanelView) and the match log.

import { stateOf, stateMax } from "../../core/names";
import { useEffect, useRef } from "react";
import { sceneImg } from "../../core/assets";
import { cx } from "../../core/cx";
import { n0 } from "../../core/format";
import { Avatar } from "../../ui/Character";
import { PanelTab } from "../../ui/Chips";
import { Icon } from "../../ui/Icon";
import type { LogLine } from "./anim";
import type { Model } from "./model";
import s from "./Players.module.css";
import { showGraveyard, showPlayerInfo } from "./Popups";
import { t as tr } from "../../i18n/t";

export function Players({ m }: { m: Model }) {
  const S = m.S;
  const n = S.players.length;
  const gap = n > 6 ? 6 : 10;
  const height = Math.min(82, Math.floor((548 - gap * (n - 1)) / n));
  return (
    <div className={s.players} style={{ gap }}>
      {S.players.map((x, i) => {
        const c = m.charOf(i);
        const out = x.bankrupt || x.left;
        const status = [stateOf(x, "stay") ? tr("board.stayN", { n: stateOf(x, "stay") }) : "", stateOf(x, "stun") + stateOf(x, "stunStart") ? tr("board.stunned") : "", stateOf(x, "exile") ? tr("board.exiled") : "", x.ai && !x.bot && !out ? tr("board.afk") : ""].filter(Boolean);
        return (
          <button key={i} type="button" style={{ height }} className={cx(s.panel, i === S.turn && s.turn, out && s.out, height < 64 && s.compact)} onClick={() => showPlayerInfo(m, i)}>
            <span className={s.n}>{i + 1}</span>
            <div className={s.av}>
              <Avatar c={c} size={height < 64 ? 42 : 64} />
              {x.bot && <span className={s.bot}><Icon name="smart_toy" /></span>}
            </div>
            <div className={s.who}>
              <b>{c?.display ?? "—"}</b>
              <small>{i === m.playerId ? tr("common.youName", { name: x.player }) : x.player}</small>
            </div>
            <div className={s.money}><img src={sceneImg("icon_coin")} alt="" /><b>{n0(x.money)}</b></div>
            <div className={s.sub}>
              {status.map((t) => <span key={t} className={s.status}>{t}</span>)}
              <span className={s.hand}><Icon name="playing_cards" />{x.hand}</span>
              <span className={s.fire}><img src={sceneImg("icon_fire")} alt="" />{stateOf(x, "fire")}/{stateMax(x, "fire")}</span>
              {/* Per-player graveyard, at the right of the module (it used to be
                  one shared pile in the board centre). */}
              <span className={s.grave} title={tr("board.graveyardPile")} onClick={(e) => { e.stopPropagation(); showGraveyard(m, i); }}>
                <img src={sceneImg("card_back")} alt="" /><b>{x.discard.length}</b>
              </span>
            </div>
            {out && <div className={s.outMark}>{x.bankrupt ? tr("board.bankrupt") : tr("board.forfeit")}</div>}
          </button>
        );
      })}
    </div>
  );
}

export function Log({ lines }: { lines: LogLine[] }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [lines]);
  return (
    <div className={s.log}>
      <PanelTab><Icon name="history" />{tr("menu.history")}</PanelTab>
      <div className={s.lines} ref={ref}>
        {lines.map((l) => <div key={l.id} className={cx(l.turn && s.turnLine)}>{l.text}</div>)}
      </div>
    </div>
  );
}

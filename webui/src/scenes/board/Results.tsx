// End of match (ResultView): your rank and rewards, the ranking, and the way
// back. Rewards are recorded once per match (and survive a refresh).

import { sceneImg } from "../../core/assets";
import { cx } from "../../core/cx";
import { n0 } from "../../core/format";
import { applyMatch, hasProfile } from "../../core/store";
import type { MatchReward } from "../../core/types";
import { type GameSession, SoloSession } from "../../game/session";
import { Btn } from "../../ui/Button";
import { Avatar } from "../../ui/Character";
import { closeAllModals, openModal } from "../../ui/Modal";
import { modeName, type Model } from "./model";
import s from "./Results.module.css";
import { t as tr } from "../../i18n/t";

const reasonText = (end: string) => ({ last: tr("results.reasonLast"), vote: tr("board.voteEnd"), out: tr("results.reasonOut"), settle: tr("results.reasonSettle") } as Record<string, string>)[end] ?? "";

function Results({ m, reward, exit }: { m: Model; reward: MatchReward | null; exit: () => void }) {
  const S = m.S;
  const mine = S.players[m.playerId];
  const reason = reasonText(S.endReason);
  const ranked = S.players.map((x, i) => [x, i] as const).sort((a, b) => a[0].rank - b[0].rank);
  return (
    <div className={s.result}>
      <div className={s.me}>
        <div className={s.title}>{tr("results.yours")}</div>
        {mine?.rank === 1 && <img className={s.medal} src={sceneImg("icon_medal")} alt="" />}
        <div className={s.big}>{mine?.rank ?? "—"}<small>{tr("results.rankUnit")}</small></div>
        <div className={s.sub}>{tr("results.players", { n: S.players.length, mode: modeName(S.mode), reason: reason ? ` · ${reason}` : "" })}</div>
        {reward ? (
          <div className={s.rewards}>
            <div><small>{tr("common.exp")}</small><b>+{n0(reward.exp)}</b>{reward.fireUsed > 0 && <small>{tr("results.fireUsed", { n: reward.fireUsed, mult: reward.multiplier })}</small>}</div>
            <div><small>{tr("common.level")}</small><b>{reward.levelBefore} → {reward.levelAfter}</b>{reward.levelAfter > reward.levelBefore && <span className={s.lvup}>{tr("results.levelUp")}</span>}</div>
            <div><img src={sceneImg("icon_star")} alt="" /><b>+{reward.stars}</b></div>
            {reward.coins !== 0 && <div><img src={sceneImg("icon_coin")} alt="" /><b>{reward.coins > 0 ? "+" : ""}{n0(reward.coins)}</b></div>}
          </div>
        ) : <p className={s.sub}>{hasProfile() ? tr("results.recorded") : tr("results.noSave1")}</p>}
      </div>
      <div className={s.list}>
        {ranked.map(([x, i]) => (
          <div key={i} className={cx(s.row, i === m.playerId && s.mine)}>
            <div className={cx(s.rank, x.rank <= 3 && s[`r${x.rank}`])}>{x.rank}</div>
            <Avatar c={m.charOf(i)} size={52} />
            <div className={s.who}><b>{x.player}{i === m.playerId && <span className={s.you}>{tr("common.you")}</span>}</b><small>{m.charOf(i)?.display ?? ""}</small></div>
            <div className={s.num}><small>{x.bankrupt ? tr("board.bankrupt") : x.left ? tr("board.forfeit") : tr("common.score")}</small><b>{n0(x.score)}</b></div>
            <div className={s.num}><small>{tr("common.assets")}</small><b>{n0(x.assets)}</b></div>
          </div>
        ))}
      </div>
      <div className={s.foot}><Btn kind="pink" size="big" onClick={exit}>{m.S.mode === 0 ? tr("results.toHome") : tr("results.toRoom")}</Btn></div>
    </div>
  );
}

export function showResults(sess: GameSession, m: Model, exit: () => void): void {
  const mine = m.S.players[m.playerId];
  let reward: MatchReward | null = null;
  if (!sess.recorded && mine && hasProfile()) {
    reward = applyMatch(m.S.mode, mine.rank, m.S.players.length, mine.character);
    if (sess instanceof SoloSession) sess.markRecorded();
    else sess.recorded = true;
  }
  closeAllModals();
  openModal(tr("results.title"), (close) => <Results m={m} reward={reward} exit={() => { close(); exit(); }} />, { closable: false, size: "wide", key: "results" });
}

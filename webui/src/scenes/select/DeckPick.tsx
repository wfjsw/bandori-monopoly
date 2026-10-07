// tr("deckPick.title"): the preset or one of the 3 saved decks (complete ones only).

import { useEffect, useState } from "react";
import { cardArt, sceneImg } from "../../core/assets";
import { cx } from "../../core/cx";
import { D, rules } from "../../core/data";
import { useAutoplay, useMatchView } from "../../core/hooks";
import { hasProfile, profileJson, updateProfile } from "../../core/store";
import type { Command } from "../../core/types";
import type { GameSession } from "../../game/session";
import { Btn } from "../../ui/Button";
import { Icon } from "../../ui/Icon";
import { openModal } from "../../ui/Modal";
import s from "./DeckPick.module.css";
import { t as tr } from "../../i18n/t";

const slotName = (n: number) => tr("deck.slotNames").split("|")[n] ?? `#${n}`;

function DeckPick({ sess, character, act, close }: { sess: GameSession; character: string; act: (c: Command) => Promise<boolean>; close: () => void }) {
  const c = D.character(character)!;
  const auto = useAutoplay(sess); // 托管
  const { view } = useMatchView(sess);
  const slotCards = (n: number): string[] =>
    n === 0 ? JSON.parse(rules.deck_preset(c.name)) : hasProfile() ? JSON.parse(rules.deck_slot_cards(profileJson(), c.name, n)) : [];
  const [pick, setPick] = useState(() => {
    const n = hasProfile() ? rules.deck_chosen_slot(profileJson(), c.name) : 0;
    return slotCards(n).length === 10 ? n : 0;
  });
  // Submitted (here, by the timer, or after a refresh elsewhere): close.
  const ready = view?.state.players[view.playerId]?.deckReady || view?.state.phase !== "deck";
  useEffect(() => {
    if (ready) close();
  }, [ready, close]);

  return (
    <div className={s.pick}>
      <h3>{tr("deckPick.heading", { char: c.display })}</h3>
      <p>{tr("deckPick.desc")}</p>
      <div className={s.rows}>
        {[0, 1, 2, 3].map((n) => {
          const cards = slotCards(n);
          const ok = cards.length === 10;
          return (
            <button key={n} type="button" className={cx(s.row, n === pick && s.on, !ok && s.off)} disabled={!ok || auto} onClick={() => setPick(n)}>
              <div className={s.check}>{n === pick && <Icon name="check" />}</div>
              <div className={s.name}>
                <b>{slotName(n)}</b>
                <small>{n === 0 ? (c.preset?.length ? tr("deckPick.presetCustom") : tr("deckPick.presetAuto")) : cards.length ? tr("deckPick.filled", { n: cards.length }) + (ok ? "" : tr("deckPick.short", { n: 10 - cards.length })) : tr("common.empty")}</small>
              </div>
              <div className={s.cards}>
                {Array.from({ length: 10 }, (_, k) => (cards[k]
                  ? <img key={k} className={s.card} src={cardArt(cards[k])} alt="" loading="lazy" title={D.card(cards[k])?.name ?? ""} />
                  : <div key={k} className={cx(s.card, s.empty)}><img src={sceneImg("star5")} alt="" /></div>))}
              </div>
            </button>
          );
        })}
      </div>
      <div className={s.foot}>
        <Btn kind="pink" wide disabled={auto} onClick={async () => {
          if (await act({ act: "deck", cards: slotCards(pick) }) && hasProfile()) updateProfile(rules.deck_choose(profileJson(), c.name, pick));
        }}>{tr("common.confirm")}</Btn>
      </div>
    </div>
  );
}

export function showDeckPick(sess: GameSession, character: string, act: (c: Command) => Promise<boolean>): void {
  openModal(tr("deckPick.title"), (close) => <DeckPick sess={sess} character={character} act={act} close={close} />, { size: "xl", key: "deck" });
}

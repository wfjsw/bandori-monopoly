// tr("deckPick.title"): the preset or any of the character's named decks
// (complete ones only). The name is profile-only -- the match gets the card list.

import { useEffect, useState } from "react";
import { cardArt, sceneImg } from "../../core/assets";
import { cx } from "../../core/cx";
import { D, rules } from "../../core/data";
import { deckLabelWith, parseDeckList, type DeckEntry } from "../../core/deckNames";
import { useAutoplay, useMatchView } from "../../core/hooks";
import { hasProfile, profileJson, updateProfile } from "../../core/store";
import type { Command } from "../../core/types";
import type { GameSession } from "../../game/session";
import { Btn } from "../../ui/Button";
import { Icon } from "../../ui/Icon";
import { openModal } from "../../ui/Modal";
import s from "./DeckPick.module.css";
import { t as tr } from "../../i18n/t";

function DeckPick({ sess, character, act, close }: { sess: GameSession; character: string; act: (c: Command) => Promise<boolean>; close: () => void }) {
  const c = D.character(character)!;
  const auto = useAutoplay(sess); // 托管
  const { view } = useMatchView(sess);
  const list: DeckEntry[] = hasProfile() ? parseDeckList(rules.deck_list(profileJson(), c.name)) : [];
  const label = (d: { id: number; name: string }) => deckLabelWith(d, (n) => tr("deck.autoName", { n }));
  const slotCards = (n: number): string[] =>
    n === 0 ? JSON.parse(rules.deck_preset(c.name)) : JSON.parse(rules.deck_slot_cards(profileJson(), c.name, n));
  const rows: { id: number; name: string; cards: string[] }[] = [
    { id: 0, name: tr("deck.presetName"), cards: JSON.parse(rules.deck_preset(c.name)) },
    ...list.map((d) => ({ id: d.id, name: label(d), cards: d.cards })),
  ];
  const [pick, setPick] = useState(() => {
    const n = hasProfile() ? rules.deck_chosen_slot(profileJson(), c.name) : 0;
    return rows.find((r) => r.id === n && r.cards.length === 10)?.id ?? 0;
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
        {rows.map((r) => {
          const ok = r.cards.length === 10;
          return (
            <button key={r.id} type="button" className={cx(s.row, r.id === pick && s.on, !ok && s.off)} disabled={!ok || auto} onClick={() => setPick(r.id)}>
              <div className={s.check}>{r.id === pick && <Icon name="check" />}</div>
              <div className={s.name}>
                <b>{r.name}</b>
                <small>{r.id === 0
                  ? (c.preset?.length ? tr("deckPick.presetCustom") : tr("deckPick.presetAuto"))
                  : r.cards.length ? tr("deckPick.filled", { n: r.cards.length }) + (ok ? "" : tr("deckPick.short", { n: 10 - r.cards.length })) : tr("common.empty")}</small>
              </div>
              <div className={s.cards}>
                {Array.from({ length: 10 }, (_, k) => (r.cards[k]
                  ? <img key={k} className={s.card} src={cardArt(r.cards[k])} alt="" loading="lazy" title={D.card(r.cards[k])?.name ?? ""} />
                  : <div key={k} className={cx(s.card, s.empty)}><img src={sceneImg("star5")} alt="" /></div>))}
              </div>
            </button>
          );
        })}
      </div>
      <div className={s.foot}>
        <Btn kind="pink" wide disabled={auto} onClick={async () => {
          const r = rows.find((x) => x.id === pick);
          if (!r) return;
          if (await act({ act: "deck", cards: slotCards(r.id) }) && hasProfile()) updateProfile(rules.deck_choose(profileJson(), c.name, r.id));
        }}>{tr("common.confirm")}</Btn>
      </div>
    </div>
  );
}

export function showDeckPick(sess: GameSession, character: string, act: (c: Command) => Promise<boolean>): void {
  openModal(tr("deckPick.title"), (close) => <DeckPick sess={sess} character={character} act={act} close={close} />, { size: "xl", key: "deck" });
}
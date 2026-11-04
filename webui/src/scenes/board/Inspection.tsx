// The viewer's own open deck inspection (design G11): the cards a look has
// turned face up, top-first, while the look is running. A private peek is
// theirs alone; a public look's ids also ride the seats' `reveal` overlay.
// The strip sits over the board's upper edge like the field sheet and never
// takes the hand's place -- the cards are not in any pile until the look ends.

import { memo, useCallback } from "react";
import { cardArtUrl } from "../../core/data";
import { namesOf } from "../../core/names";
import { t as tr } from "../../i18n/t";
import { pickCard } from "../../ui/CardPreview";
import type { Model } from "./model";
import s from "./Inspection.module.css";

export const Inspection = memo(function Inspection({ m }: { m: Model }) {
  const look = m.v.inspection;
  const names = namesOf(m.S);
  const present = useCallback((id: string) => pickCard(id), []);
  if (!look || !look.cards.length) return null;
  return (
    <div className={s.strip} title={tr("board.inspection", { n: look.cards.length })}>
      <div className={s.head}>
        <span>{tr("board.inspection", { n: look.cards.length })}</span>
        {look.public && <b className={s.public}>{tr("ui.inspection.public")}</b>}
      </div>
      <div className={s.cards}>
        {look.cards.map((id, k) => (
          // The same card can sit in the look twice.
          <button key={`${id}#${k}`} type="button" className={s.card} title={names.card(id)} onClick={() => present(id)}>
            <img src={cardArtUrl(id)} alt="" />
            <span>{names.card(id)}</span>
          </button>
        ))}
      </div>
    </div>
  );
});

// The standing card panel in the right sidebar (Master Duel's card detail):
// always one card in full -- art, title, tags, effect text -- changing as the
// user hovers any card, clicks one, or a card flash plays. It keeps the last
// card until another replaces it. The match screen turns off the floating
// hover popups while this is up (CardPreview `useStandingPreviewOn`).

import { useEffect } from "react";
import { CardPreview, cardStandClass, stickCard, useStickyCard } from "../../ui/CardPreview";
import { t as tr } from "../../i18n/t";
import s from "./CardStand.module.css";

/** Follow a card id (the flash) into the standing panel. */
function useStickOn(id: string | undefined): void {
  useEffect(() => {
    if (id) stickCard(id);
  }, [id]);
}

export function CardStand({ flash }: { flash?: string }) {
  const card = useStickyCard();
  useStickOn(flash);
  return (
    <div className={s.stand} aria-live="polite">
      {card ? (
        <CardPreview id={card.id} note={card.note} className={cardStandClass} scrollable />
      ) : (
        <div className={s.empty}>{tr("prompt.cardHover")}</div>
      )}
    </div>
  );
}
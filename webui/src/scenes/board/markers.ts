// Card-face marker rules shared by the field sheet, the popups and the marker
// list.

/** Whether a face should render its `⌛N` countdown chip.
 *
 *  A countdown that is literally carried by another marker on the same face --
 *  the crystal count a decay removes, a phase counter -- is not repeated: that
 *  marker already shows the number, so the extra `⌛N` would just be a second
 *  copy of it. The countdown stays in the data either way; this only decides
 *  whether the face prints it again. */
export function showTurnsChip(turnsLeft: number, ...others: (number | undefined)[]): boolean {
  return turnsLeft > 0 && !others.some((n) => n === turnsLeft);
}

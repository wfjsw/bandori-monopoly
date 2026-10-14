// Text helpers shared by the scenes.

/** C# `ToString("N0")`. */
export function n0(v: number): string {
  return Math.round(v).toLocaleString("en-US");
}

/** Strip the line breaks tile names carry for the board. */
export const plain = (s: string) => s.replace(/\n/g, "");

function esc(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}

/** Unity rich text (`<b>`, `<color=#..>`, `<size=125%>`) to safe HTML. */
export function richText(s: string): string {
  return esc(s)
    .replace(/&lt;b&gt;/g, "<b>")
    .replace(/&lt;\/b&gt;/g, "</b>")
    .replace(/&lt;color=(#[0-9a-fA-F]{3,8})&gt;/g, '<span style="color:$1">')
    .replace(/&lt;\/color&gt;/g, "</span>")
    .replace(/&lt;size=(\d+)%&gt;/g, '<span style="font-size:$1%">')
    .replace(/&lt;\/size&gt;/g, "</span>")
    .replace(/\n/g, "<br>");
}

/** Pick readable text color for a background (BDTheme.ReadableOn). */
/** True when dark ink reads better than white on `hex` (higher WCAG contrast
 *  ratio wins; the plain luma cut-off leaves mid tones at ~3:1 with white). */
export function prefersDarkInk(hex: string): boolean {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex);
  if (!m) return false;
  const v = parseInt(m[1], 16);
  const lin = (c: number) => { const s = c / 255; return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4; };
  const L = 0.2126 * lin((v >> 16) & 255) + 0.7152 * lin((v >> 8) & 255) + 0.0722 * lin(v & 255);
  // Dark ink (#17221e, L≈0.015) beats white once (L+.05)² > 1.05 × 0.065.
  return (L + 0.05) ** 2 > 0.06825;
}

export function isLight(hex: string): boolean {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex);
  if (!m) return false;
  const v = parseInt(m[1], 16);
  return 0.299 * ((v >> 16) & 255) + 0.587 * ((v >> 8) & 255) + 0.114 * (v & 255) > 186;
}

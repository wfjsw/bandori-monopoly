// `?vdiag=1`: a live readout of every viewport the stage could size against,
// for checking the stage geometry on a real handset (the simulators cannot
// reproduce Safari's toolbar or the island). Screen space, over everything,
// click-through.

import { useEffect, useState } from "react";

function probe(css: string): string {
  const el = document.createElement("div");
  el.style.cssText = `position:fixed;visibility:hidden;pointer-events:none;${css}`;
  document.body.appendChild(el);
  const r = el.getBoundingClientRect();
  const cs = getComputedStyle(el);
  el.remove();
  if (css.includes("padding")) return [cs.paddingTop, cs.paddingRight, cs.paddingBottom, cs.paddingLeft].map((v) => parseFloat(v) || 0).join(" ");
  return `${r.left},${r.top} ${r.width}x${r.height}`;
}

function read(): string[] {
  const vv = window.visualViewport;
  const placer = document.querySelector("[data-stage]")?.parentElement;
  const p = placer?.getBoundingClientRect();
  const r = (n: number) => Math.round(n * 10) / 10;
  return [
    `ua ${/iPhone|iPod/.test(navigator.userAgent) ? "iPhone" : "other"}  standalone ${String((navigator as Navigator & { standalone?: boolean }).standalone ?? "-")}`,
    `screen ${window.screen.width}x${window.screen.height}`,
    `inner ${window.innerWidth}x${window.innerHeight}`,
    `visual ${vv ? `${r(vv.offsetLeft)},${r(vv.offsetTop)} ${r(vv.width)}x${r(vv.height)} @${r(vv.scale)}` : "-"}`,
    `fixed inset:0 ${probe("inset:0")}`,
    `100lvw/lvh ${probe("top:0;left:0;width:100lvw;height:100lvh")}`,
    `100svw/svh ${probe("top:0;left:0;width:100svw;height:100svh")}`,
    `safe t r b l ${probe("padding:env(safe-area-inset-top) env(safe-area-inset-right) env(safe-area-inset-bottom) env(safe-area-inset-left)")}`,
    `placer ${p ? `${r(p.left)},${r(p.top)} ${r(p.width)}x${r(p.height)}  ${placer?.style.transform ?? ""}` : "-"}`,
  ];
}

export function ViewportDiag() {
  const [lines, set] = useState(read);
  useEffect(() => {
    const id = window.setInterval(() => set(read()), 500);
    return () => clearInterval(id);
  }, []);
  return (
    <pre
      style={{
        position: "fixed",
        left: "50%",
        top: "50%",
        transform: "translate(-50%, -50%)",
        zIndex: 3000,
        margin: 0,
        padding: "6px 8px",
        background: "rgba(0, 0, 0, 0.75)",
        color: "#fff",
        font: "11px/1.35 ui-monospace, Menlo, monospace",
        pointerEvents: "none",
        whiteSpace: "pre",
        userSelect: "text",
      }}
    >
      {lines.join("\n")}
    </pre>
  );
}

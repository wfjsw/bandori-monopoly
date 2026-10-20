// A live readout of every viewport the stage could size against, for checking
// the stage geometry on a real handset (the simulators cannot reproduce
// Safari's toolbar, the island or the home-screen app's window). Screen space,
// over everything, click-through.
//
// On with `?vdiag=1`, or on every page of a dev server started with
// `BM_VDIAG=<file>` (rsbuild.config.ts) -- the home-screen app has no address
// bar to add a query to. That server's pages also post each changed reading
// to `/__vdiag`, which it logs, so the numbers can be read off the server
// rather than a screenshot. Nothing is persisted on the device.

import { useEffect, useState } from "react";
import { isStageRotated } from "./stageScale";
import stage from "./Stage.module.css";

function classProbe(cls: string): string {
  const el = document.createElement("div");
  el.className = cls;
  document.body.appendChild(el);
  const cs = getComputedStyle(el);
  const v = [cs.paddingTop, cs.paddingRight, cs.paddingBottom, cs.paddingLeft].map((p) => parseFloat(p) || 0).join(" ");
  el.remove();
  return v;
}

const REPORT = import.meta.env?.BM_VDIAG === "1";
/** Is the readout on for this page load? */
export const VDIAG = REPORT || new URLSearchParams(location.search).has("vdiag");

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

function rect(el: Element | null | undefined): string {
  if (!el) return "-";
  const r = el.getBoundingClientRect();
  const n = (v: number) => Math.round(v * 10) / 10;
  return `${n(r.left)},${n(r.top)} ${n(r.width)}x${n(r.height)}`;
}

function read(): string[] {
  const vv = window.visualViewport;
  const placer = document.querySelector("[data-stage]")?.parentElement;
  const n = (v: number) => Math.round(v * 10) / 10;
  const standalone = (navigator as Navigator & { standalone?: boolean }).standalone;
  return [
    `ua ${/iPhone|iPod/.test(navigator.userAgent) ? "iPhone" : "other"}  standalone ${String(standalone ?? "-")}  display-mode ${window.matchMedia("(display-mode: standalone)").matches ? "standalone" : "browser"}`,
    `orientation ${screen.orientation?.type ?? "-"}  dpr ${window.devicePixelRatio}`,
    `screen ${screen.width}x${screen.height}  avail ${screen.availWidth}x${screen.availHeight}`,
    `inner ${window.innerWidth}x${window.innerHeight}  client ${document.documentElement.clientWidth}x${document.documentElement.clientHeight}`,
    `visual ${vv ? `${n(vv.offsetLeft)},${n(vv.offsetTop)} ${n(vv.width)}x${n(vv.height)} @${n(vv.scale)}` : "-"}`,
    `fixed inset:0 ${probe("inset:0")}`,
    `100lvw/lvh ${probe("top:0;left:0;width:100lvw;height:100lvh")}`,
    `100svw/svh ${probe("top:0;left:0;width:100svw;height:100svh")}`,
    `100vw/vh ${probe("top:0;left:0;width:100vw;height:100vh")}`,
    `html ${rect(document.documentElement)}  body ${rect(document.body)}`,
    `safe t r b l (stylesheet, used) ${classProbe(stage.safeProbe)}`,
    `safe t r b l (cssText, old) ${probe("padding:env(safe-area-inset-top) env(safe-area-inset-right) env(safe-area-inset-bottom) env(safe-area-inset-left)")}`,
    `--safe t r b l ${(() => {
      const st = document.querySelector<HTMLElement>("[data-stage]");
      return st ? ["top", "right", "bottom", "left"].map((k) => st.style.getPropertyValue(`--safe-${k}`).replace("px", "").slice(0, 5)).join(" ") : "-";
    })()}`,
    `placer ${rect(placer)}  ${placer?.style.transform ?? ""}`,
    `placer at ${placer ? `${placer.style.left || "-"} ${placer.style.top || "-"}` : "-"}  forced ${isStageRotated()}`,
  ];
}

let lastSent = "";
function report(lines: string[]): void {
  const key = lines.join("\n");
  if (!REPORT || key === lastSent) return;
  lastSent = key;
  void fetch("/__vdiag", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ path: location.pathname, ua: navigator.userAgent, lines }),
    keepalive: true,
  }).catch(() => {});
}

export function ViewportDiag() {
  const [lines, set] = useState(read);
  useEffect(() => {
    const id = window.setInterval(() => {
      const next = read();
      report(next);
      set(next);
    }, 500);
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
      }}
    >
      {lines.join("\n")}
    </pre>
  );
}

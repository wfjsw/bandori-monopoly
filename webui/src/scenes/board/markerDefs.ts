// The card-face marker rows, as exported by the rule code.
//
// `generated/display.json` `markers` is `bandori_std::marker::MARKERS` verbatim
// (`rules/bandori-std/src/marker.rs`, `docs/DISPLAY.md`), in print order.
// Adding a marker kind is a rules edit; nothing here names one.

import displayJson from "../../generated/display.json";
import type { DisplayFile } from "../../core/types";
import type { MarkerDef } from "./markerKinds";

const display = displayJson as unknown as DisplayFile;

/** One row per marker kind, in print order. */
export const MARKER_DEFS = (display.markers ?? []) as MarkerDef[];

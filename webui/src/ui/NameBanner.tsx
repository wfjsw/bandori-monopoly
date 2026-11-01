// The vector name plate, built from the roster row: pill, dotted end zone,
// stars and the name on top, all as SVG elements — no markup strings, so the
// name is escaped by React. Geometry lives in core/nameBanner.ts. Text is live,
// so the page's `BDRounded` face applies and any name fits.

import { useId, type CSSProperties } from "react";
import {
  BANNER_HEIGHT,
  RIM,
  SUBTITLE,
  TITLE,
  bannerWidth,
  dotZone,
  pill,
  starCentres,
  starPath,
  subtitleSize,
  titleBand,
  titleSize,
} from "../core/nameBanner";
import s from "./NameBanner.module.css";

/** Rounded faces, the app face first. */
const FONT =
  "BDRounded,'M PLUS Rounded 1c','Rounded Mplus 1c','Hiragino Maru Gothic ProN','Kosugi Maru','Arial Rounded MT Bold','Noto Sans JP',sans-serif";

const STAR = "#fdee5b";

export interface NameBannerProps {
  /** The character name. */
  title: string;
  /** Line under the name; omitted when empty. */
  subtitle?: string;
  /** Pill and title-outline colour. */
  color: string;
  /** Design length in px; defaults to the fitted width. Display size is CSS. */
  width?: number;
  className?: string;
  style?: CSSProperties;
}

export function NameBanner({ title, subtitle: rawSubtitle = "", color, width, className, style }: NameBannerProps) {
  // The artwork sets its latin subtitle in caps; callers store it in natural case.
  const subtitle = rawSubtitle.toUpperCase();
  const W = width ?? bannerWidth(title, subtitle);
  const size = titleSize(title, W);
  const subSize = subtitleSize(subtitle, W);
  const outline = (TITLE.outline * size) / TITLE.size;
  const cx = W / 2;
  const p = pill(W);
  const dots = dotZone(W, p);
  const stars = starCentres(W, p);
  const band = titleBand(title, size);
  const id = useId().replace(/:/g, "");

  // The title is stroked three times over the same string: the white rim that
  // joins the pill's rim, then the colour, then the letters, each layer wider
  // than the one over it so only its edge shows.
  const text = (props: { fill: string; stroke?: string; strokeWidth?: number }) => (
    <text
      x={cx + TITLE.spacing / 2}
      y={TITLE.base}
      textAnchor="middle"
      fontFamily={FONT}
      fontSize={size}
      fontWeight={500}
      letterSpacing={TITLE.spacing}
      strokeLinejoin="round"
      strokeLinecap="round"
      {...props}
    >
      {title}
    </text>
  );

  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width={W}
      height={BANNER_HEIGHT}
      viewBox={`0 0 ${W} ${BANNER_HEIGHT}`}
      role="img"
      aria-label={[title, subtitle].filter(Boolean).join(" ")}
      className={className ? `${s.banner} ${className}` : s.banner}
      style={style}
    >
      <defs>
        <clipPath id={`${id}-dots`}>
          <polygon points={`${dots.cutTop},${p.innerY} ${W},${p.innerY} ${W},${p.innerY + p.innerHeight} ${dots.cutBottom},${p.innerY + p.innerHeight}`} />
        </clipPath>
        <clipPath id={`${id}-pill`}>
          <rect x={p.innerX} y={p.innerY} width={p.innerWidth} height={p.innerHeight} rx={p.innerRadius} />
        </clipPath>
        <pattern id={`${id}-lattice`} x={dots.offsetX} y={dots.offsetY} width={dots.cellX} height={dots.cellY} patternUnits="userSpaceOnUse">
          <g fill="#ffffff" fillOpacity={0.24}>
            <circle cx={0} cy={0} r={dots.radius} />
            <circle cx={dots.cellX} cy={0} r={dots.radius} />
            <circle cx={0} cy={dots.cellY} r={dots.radius} />
            <circle cx={dots.cellX} cy={dots.cellY} r={dots.radius} />
            <circle cx={dots.cellX / 2} cy={dots.cellY / 2} r={dots.radius} />
          </g>
        </pattern>
      </defs>

      {/* 1. rim: pill outline and title outline merge into one sticker shape */}
      <rect x={0} y={p.y} width={W} height={p.height} rx={p.radius} fill="#ffffff" />
      {text({ fill: "#ffffff", stroke: "#ffffff", strokeWidth: outline + 2 * RIM })}

      {/* 2. pill body and its dots */}
      <rect x={p.innerX} y={p.innerY} width={p.innerWidth} height={p.innerHeight} rx={p.innerRadius} fill={color} />
      <g clipPath={`url(#${id}-pill)`}>
        <g clipPath={`url(#${id}-dots)`}>
          <rect x={0} y={p.innerY} width={W} height={p.innerHeight} fill={`url(#${id}-lattice)`} />
        </g>
      </g>

      {/* 3. stars */}
      <g fill={STAR}>
        <path d={starPath(stars.left, stars.y, stars.radius)} />
        <path d={starPath(stars.right, stars.y, stars.radius)} />
      </g>

      {/* 4. subtitle */}
      {subtitle && (
        <text
          x={cx + SUBTITLE.spacing / 2}
          y={p.mid + subSize * 0.357}
          textAnchor="middle"
          fontFamily={FONT}
          fontSize={subSize}
          fontWeight={500}
          letterSpacing={SUBTITLE.spacing}
          fill="#ffffff"
        >
          {subtitle}
        </text>
      )}

      {/* 5. title: colour, then the letters — the strip hides the pill rim between them */}
      <rect x={cx - band / 2} y={p.y} width={band} height={RIM + 2} fill={color} />
      {text({ fill: color, stroke: color, strokeWidth: outline })}
      {text({ fill: "#ffffff" })}
    </svg>
  );
}

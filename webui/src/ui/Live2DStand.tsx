// A character stand: the compiled Live2D model (docs/LIVE2D.md) over the static
// stand art, which stays as the fallback until the model is on screen and
// whenever a model is missing. Model ids are the character art ids
// (`D.artId(c)`), see core/data.ts and docs/LIVE2D.md.
//
// Fit mirrors the original `BandoriMonopoly.UI.Live2DPortrait`: the model draws
// into a 3:4 window (the Unity portrait's 768x1024 render target, AspectRatio-
// Fitter FitInParent) centred in the stand box, with `zoom` / `focusTop` /
// `headroom` and the per-id `framing.json` measuring sheet shifting and scaling
// the canvas exactly as Live2DPortrait.Draw does. All of that chain lives in
// `useLive2DStand`; this component is the box, the art and the click.

import { useLive2DStand } from "../hooks/live2d";
import { useRestartAnimation } from "../hooks/timers";
import { Live2DModel } from "../live2d";
import { charArt } from "../core/assets";
import { cx } from "../core/cx";
import { t as tr } from "../i18n/t";
import s from "./Live2DStand.module.css";

export interface Live2DStandProps {
  /** Live2D model id = the character art id (`D.artId(c)`): "001", "036c", … */
  id: string;
  /** Scene class that positions the stand box (stand art is 582 × 910). */
  className?: string;
  /** Restart the stand bounce when this changes (the Menu dialogue lines). */
  pulse?: number;
  alt?: string;
  onClick?: () => void;
  /** Parameter values for the model, e.g. `{ PARAM_ANGLE_X: 5 }`. */
  params?: Record<string, number>;
  /** Display zoom (Live2DPortrait.Show); 1 fits the canvas to the window. */
  zoom?: number;
  /** Cap for the framing shift (0 = never push the focus further down). */
  focusTop?: number;
  /** Minimum gap above the head as a fraction of the window height. */
  headroom?: number;
  /** Window height fraction the eye line sits on (characters align here). */
  eyeFrac?: number;
  /**
   * Where the 3:4 portrait window sits inside the stand box. `center` matches
   * Unity's AspectRatioFitter FitInParent; `top` pulls it up flush with the
   * stand's top edge, buying the head the ~7% of the window height that
   * vertical centring wastes below the top bar.
   */
  fit?: "center" | "top";
}

export function Live2DStand({ id, className, pulse, alt, onClick, params, zoom, focusTop, headroom, eyeFrac, fit }: Live2DStandProps) {
  const stand = useLive2DStand({ id, zoom, focusTop, headroom, eyeFrac, fit });
  // Restart the CSS bounce without remounting (which would reload the model).
  useRestartAnimation(pulse, stand.animRef);
  const art = charArt(id, "stand");

  // Clicking the stand plays a random reaction (Live2DPortrait.TryPlayReaction;
  // reactions never stack) and still runs the scene's own click handler.
  const handleClick = () => {
    stand.modelHandle.current?.playReaction();
    onClick?.();
  };

  return (
    <div ref={stand.boxRef} className={cx(s.stand, className)} onClick={handleClick}>
      <div ref={stand.animRef} className={cx(s.anim, !!pulse && s.bounce)}>
        {art ? (
          !stand.gone && <img className={cx(s.art, stand.shown && s.hide)} src={art} alt={alt ?? ""} draggable={false} />
        ) : (
          !stand.gone && <div className={s.missing}>{stand.failed ? tr("live2d.missing") : tr("live2d.loading")}</div>
        )}
        {stand.win && stand.model && (
          <div className={s.window} style={stand.win}>
            <div className={s.model} style={stand.model}>
              <Live2DModel ref={stand.modelHandle} id={id} params={params} className={s.fill} />
            </div>
          </div>
        )}
      </div>
      {art && !stand.shown && !stand.failed && <div className={s.loading}>{tr("live2d.loading")}</div>}
    </div>
  );
}
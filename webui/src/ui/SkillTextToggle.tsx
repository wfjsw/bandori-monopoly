// C# `SkillTextToggle` -- flips SkillText.Simple and shows the action it would
// perform ("show simple" / "show full").
import { useState } from "react";

import { skillTextSwitch } from "../core/data";
import { settings, saveSettings } from "../core/store";
import { t as tr } from "../i18n/t";
import s from "./SkillTextToggle.module.css";

export function SkillTextToggle({ className }: { className?: string }) {
  const [, setOn] = useState(settings().skillTextSimple);
  return (
    <button
      type="button"
      className={`${s.toggle}${className ? ` ${className}` : ""}`}
      onClick={() => {
        const next = !settings().skillTextSimple;
        saveSettings({ ...settings(), skillTextSimple: next });
        setOn(next);
      }}
    >
      {tr(skillTextSwitch())}
    </button>
  );
}

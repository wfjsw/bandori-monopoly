// The server picker: which game server this page plays on, and which of the
// profile server's advertised servers are available. Connecting switches the
// live rules / tables in place (no page reload; the profile-server address is
// edited here too, for the directory + uploads).

import { useEffect, useState } from "react";
import { switchSource } from "../../core/data";
import { normalizeServer, probeGameServer, useServer } from "../../net/server";
import { health, listServers, profileBase, setProfileBase, type ProfileGameServer } from "../../net/profile";
import { Btn } from "../../ui/Button";
import { TextInput } from "../../ui/Form";
import { Icon } from "../../ui/Icon";
import { openModal } from "../../ui/Modal";
import { toast } from "../../ui/Toast";
import s from "./Servers.module.css";
import { t as tr } from "../../i18n/t";

/** "host:port" of a base URL, or the local label for this origin. */
function serverLabel(base: string): string {
  if (!base) return tr("server.local");
  try {
    return new URL(base).host;
  } catch {
    return base;
  }
}

function buildLabel(build: string, name: string): string {
  if (name) return name;
  if (build) return build;
  return tr("server.defaultBuild");
}

/** The menu's current-server pill; clicking opens the picker. */
export function ServerChip() {
  const { base, build, buildName } = useServer();
  return (
    <button type="button" className={s.chip} onClick={showServers} title={tr("server.title")}>
      <Icon name="swap_horiz" />
      <span className={s.chipServer}>{serverLabel(base)}</span>
      <span className={s.chipBuild}>{buildLabel(build, buildName)}</span>
    </button>
  );
}

function Servers({ close }: { close: () => void }) {
  const current = useServer();
  const [profile, setProfile] = useState(profileBase());
  const [directory, setDirectory] = useState<ProfileGameServer[] | null>(null);
  const [profileOnline, setProfileOnline] = useState<boolean | null>(null);
  const [url, setUrl] = useState(current.base);
  const [ping, setPing] = useState<{ ok: boolean; version?: string; rooms?: number; builds?: number } | null>(null);
  const [pinging, setPinging] = useState(false);

  const refreshDirectory = () => {
    void listServers().then(setDirectory);
    void health().then((h) => setProfileOnline(h.ok));
  };

  useEffect(refreshDirectory, []);

  const test = async (target: string) => {
    setPinging(true);
    setPing(await probeGameServer(target));
    setPinging(false);
  };

  // Ping whatever this page is on right now, so the panel opens with a status.
  useEffect(() => {
    void test(current.base);
    // Mount-only: the picker closes on connect (the scene then remounts on the
    // new data).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const connect = (target: string | null) => {
    if (target === null) {
      close();
      void switchSource({ server: null });
      return;
    }
    const raw = target.trim();
    const next = normalizeServer(raw);
    if (!next) return toast(tr("server.invalid"), "error");
    close();
    // Loads the new server's rules and tables live; the scenes remount on them.
    void switchSource({ server: next });
  };

  /** A directory URL may be absolute or relative to the profile server. */
  const resolveUrl = (u: string): string => {
    try {
      return new URL(u, profileBase() || location.origin).href;
    } catch {
      return u;
    }
  };

  const saveProfile = () => {
    const next = profile.trim();
    if (next && !normalizeServer(next)) return toast(tr("server.invalid"), "error");
    setProfileBase(next);
    refreshDirectory();
  };

  return (
    <div className={s.wrap}>
      <div className={s.current}>
        <span className={s.currentLabel}>{tr("server.current")}</span>
        <b>{serverLabel(current.base)}</b>
        <span className={s.currentBuild}>{buildLabel(current.build, current.buildName)}</span>
        <span className={ping ? (ping.ok ? s.ok : s.bad) : s.hint}>
          {pinging ? tr("server.loading") : ping ? (ping.ok ? tr("server.healthOk", { version: ping.version ?? "?", rooms: ping.rooms ?? 0, builds: ping.builds ?? 0 }) : tr("server.healthBad")) : ""}
        </span>
        <Btn size="small" onClick={() => void test(current.base)} disabled={pinging}>{tr("server.test")}</Btn>
      </div>

      <div className={s.section}>
        <div className={s.sectionTitle}>{tr("server.profileBase")}</div>
        <div className={s.field}>
          <TextInput className={s.grow} value={profile} placeholder={tr("server.profilePlaceholder")} onChange={(e) => setProfile(e.target.value)} />
          <Btn size="small" onClick={saveProfile}>{tr("server.save")}</Btn>
        </div>
        <div className={profileOnline === false ? s.bad : s.hint}>
          {profileOnline === null ? tr("server.loading") : profileOnline ? tr("server.profileOnline") : tr("server.directoryOffline")}
        </div>
      </div>

      <div className={s.section}>
        <div className={s.sectionTitle}>{tr("server.directory")}</div>
        {directory === null ? (
          <div className={s.hint}>{tr("server.loading")}</div>
        ) : directory.length === 0 ? (
          <div className={s.hint}>{tr("server.directoryEmpty")}</div>
        ) : (
          <div className={s.list}>
            {directory.map((d) => (
              <div key={d.id} className={s.server}>
                <div className={s.serverMain}>
                  <b>{d.name || d.url}</b>
                  <small>{d.url}</small>
                  {d.note && <span className={s.note}>{d.note}</span>}
                </div>
                <Btn size="small" kind="pink" disabled={!d.url.trim()} onClick={() => connect(resolveUrl(d.url))}>{tr("server.connect")}</Btn>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className={s.section}>
        <div className={s.sectionTitle}>{tr("server.customUrl")}</div>
        <div className={s.field}>
          <TextInput className={s.grow} value={url} placeholder={tr("server.customPlaceholder")} onChange={(e) => setUrl(e.target.value)} />
          <Btn size="small" onClick={() => void test(url)} disabled={pinging}>{tr("server.test")}</Btn>
          <Btn size="small" kind="pink" onClick={() => connect(url)} disabled={!url.trim()}>{tr("server.connect")}</Btn>
        </div>
        <div className={s.foot}>
          <Btn size="small" onClick={() => connect(null)}>{tr("server.useLocal")}</Btn>
          <span className={s.hint}>{tr("server.localHint")}</span>
        </div>
      </div>
    </div>
  );
}

export function showServers(): void {
  openModal(tr("server.title"), (close) => <Servers close={close} />, { size: "mid" });
}

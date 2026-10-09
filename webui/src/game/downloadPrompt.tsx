// Download a `.bdrec`, with the optional **portable** form (`docs/REPLAY.md`
// §10): "include engine" embeds the exact engine bundle the record was written
// with, so the file replays offline and on a deployment that never hosted that
// engine. Default off -- a plain record is a few kB and plays everywhere the
// archive has the bundle; the portable form is ~2 MiB and plays anywhere.
//
// One dialog, used by the Results screen, the replay list and the room's
// "last replay" button.

import { useState } from "react";
import { rules } from "../core/data";
import { useAsync } from "../hooks/async";
import { Btn } from "../ui/Button";
import { closeAllModals, openModal } from "../ui/Modal";
import { toast } from "../ui/Toast";
import { t as tr } from "../i18n/t";
import { downloadRecord, type RecordHeader } from "./record";
import { exportPortable } from "./portableEngine";
import { PortableError } from "./portable";

const fmtBytes = (n: number): string =>
  n >= 1024 * 1024 ? `${(n / 1024 / 1024).toFixed(2)} MiB` : `${Math.max(1, Math.round(n / 1024))} KiB`;

/** Open the download dialog for `bytes` (a plain or portable `.bdrec`). */
export function downloadRecordPrompt(bytes: Uint8Array, filename: string): void {
  let header: RecordHeader | null = null;
  try {
    header = JSON.parse(rules.record_header_bytes(bytes)) as RecordHeader;
  } catch (e) {
    console.warn("download: cannot read the record header:", e);
  }
  const name = filename.endsWith(".bdrec") ? filename : `${filename}.bdrec`;
  openModal(
    tr("results.downloadReplay"),
    (close) => <DownloadDialog bytes={bytes} name={name} header={header} close={close} />,
    { size: "mid", key: "download-replay" },
  );
}

/**
 * Build the portable (engine-embedded) form of the record -- lazily, only when
 * the box is ticked. It hashes and compresses the engine bundle, which is
 * ~7 MiB of work the plain path never needs. Cancellable: unticking the box
 * (or closing the dialog) abandons the in-flight build.
 */
function usePortable(bytes: Uint8Array, header: RecordHeader | null, withEngine: boolean): {
  busy: boolean;
  portable: { bytes: Uint8Array; portableLen: number } | undefined;
  whyNot: string | null;
} {
  const state = useAsync(async () => {
    const out = await exportPortable(bytes, header!);
    return { bytes: out.bytes, portableLen: out.portableLen };
  }, [withEngine, bytes, header], withEngine && !!header);
  return {
    busy: state.loading,
    portable: state.value,
    whyNot: state.error == null
      ? null
      : state.error instanceof PortableError
        ? state.error.message
        : String(state.error instanceof Error ? state.error.message : state.error),
  };
}

function DownloadDialog({
  bytes,
  name,
  header,
  close,
}: {
  bytes: Uint8Array;
  name: string;
  header: RecordHeader | null;
  close: () => void;
}) {
  const [withEngine, setWithEngine] = useState(false);
  const { busy, portable, whyNot } = usePortable(bytes, header, withEngine);

  const canEmbed = !!header?.engine?.bundle || !!header;
  const doDownload = () => {
    const out = withEngine && portable ? portable.bytes : bytes;
    downloadRecord(out, name);
    close();
    if (withEngine && portable) toast(tr("results.portableSaved"), "info");
  };

  return (
    <div>
      <p>{name}</p>
      <p>
        {tr("results.plainSize", { size: fmtBytes(bytes.length) })}
        {portable ? ` · ${tr("results.portableSize", { size: fmtBytes(portable.portableLen) })}` : ""}
      </p>
      <label style={{ display: "flex", gap: 8, alignItems: "flex-start", margin: "12px 0" }}>
        <input
          type="checkbox"
          checked={withEngine}
          disabled={!canEmbed || busy}
          onChange={(e) => setWithEngine(e.target.checked)}
        />
        <span>
          {tr("results.includeEngine")}
          <br />
          <small>{tr("results.includeEngineHint")}</small>
        </span>
      </label>
      {busy && <p>{tr("results.includeEngineBusy")}</p>}
      {whyNot && <p role="alert">{whyNot}</p>}
      <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
        <Btn onClick={close}>{tr("common.cancel")}</Btn>
        <Btn
          kind="pink"
          onClick={doDownload}
          disabled={withEngine && (!portable || busy)}
        >
          {tr("results.downloadReplay")}
        </Btn>
      </div>
    </div>
  );
}

/** Close whatever the dialog left open (tests / scene changes). */
export function closeDownloadPrompt(): void {
  closeAllModals();
}
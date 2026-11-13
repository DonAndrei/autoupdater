import { useEffect, useMemo, useState } from "react";
import { scan, subscribe, updateApps } from "./api";
import type { OutdatedApp, RowStatus, ScanResult } from "./types";

const SOURCE_LABEL: Record<string, string> = {
  winget: "winget",
  msstore: "MS Store",
  brew: "Homebrew",
  cask: "Homebrew Cask",
  mas: "App Store",
};

const MAX_LOG = 200;

export default function App() {
  const [apps, setApps] = useState<OutdatedApp[]>([]);
  const [warnings, setWarnings] = useState<string[]>([]);
  const [checkedAt, setCheckedAt] = useState<number | null>(null);
  const [scanning, setScanning] = useState(false);
  const [updating, setUpdating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [status, setStatus] = useState<Record<string, RowStatus>>({});
  const [expanded, setExpanded] = useState<string | null>(null);
  const [filter, setFilter] = useState("");
  const [summary, setSummary] = useState<string | null>(null);

  const applyScan = (r: ScanResult) => {
    setApps(r.apps);
    setWarnings(r.warnings);
    setCheckedAt(r.checkedAt);
    setScanning(false);
    setStatus({});
    setSummary(null);
    setSelected((prev) => new Set(r.apps.filter((a) => prev.has(a.key)).map((a) => a.key)));
  };

  useEffect(() => {
    let off: (() => void) | undefined;
    let cancelled = false;
    subscribe({
      onScanStarted: () => setScanning(true),
      onScanResult: applyScan,
      onProgress: (p) =>
        setStatus((s) => {
          const prev = s[p.key] ?? { state: "queued", log: [] };
          const log = p.line ? [...prev.log, p.line].slice(-MAX_LOG) : prev.log;
          return { ...s, [p.key]: { state: p.status, log, error: p.error ?? prev.error } };
        }),
      onFinished: (f) => {
        setUpdating(false);
        setSummary(f.failed ? `Updated ${f.ok}, ${f.failed} failed.` : `Updated ${f.ok} app(s).`);
      },
    }).then((u) => (cancelled ? u() : (off = u)));
    runScan();
    return () => {
      cancelled = true;
      off?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function runScan() {
    setError(null);
    setScanning(true);
    try {
      applyScan(await scan());
    } catch (e) {
      setScanning(false);
      // A scan started from the tray is already running; its result arrives as an event.
      if (!String(e).includes("already running")) setError(String(e));
      else setScanning(true);
    }
  }

  async function runUpdate() {
    const keys = apps.filter((a) => selected.has(a.key)).map((a) => a.key);
    if (!keys.length) return;
    setError(null);
    setSummary(null);
    setStatus((s) => {
      const next = { ...s };
      keys.forEach((k) => (next[k] = { state: "queued", log: [] }));
      return next;
    });
    setUpdating(true);
    try {
      await updateApps(keys);
      setSelected(new Set());
    } catch (e) {
      setUpdating(false);
      setError(String(e));
    }
  }

  const visible = useMemo(() => {
    const q = filter.trim().toLowerCase();
    return q ? apps.filter((a) => a.name.toLowerCase().includes(q) || a.id.toLowerCase().includes(q)) : apps;
  }, [apps, filter]);

  const selectable = visible.filter((a) => status[a.key]?.state !== "done");
  const allSelected = selectable.length > 0 && selectable.every((a) => selected.has(a.key));
  const busy = scanning || updating;

  const toggle = (key: string) =>
    setSelected((s) => {
      const n = new Set(s);
      n.has(key) ? n.delete(key) : n.add(key);
      return n;
    });

  const toggleAll = () =>
    setSelected((s) => {
      const n = new Set(s);
      selectable.forEach((a) => (allSelected ? n.delete(a.key) : n.add(a.key)));
      return n;
    });

  const pending = apps.filter((a) => status[a.key]?.state !== "done").length;

  return (
    <div className="app">
      <header className="top">
        <div>
          <h1>AutoUpdater</h1>
          <p className="muted">
            {scanning
              ? "Checking for updates…"
              : checkedAt
                ? `${pending} update${pending === 1 ? "" : "s"} available · checked ${new Date(checkedAt).toLocaleTimeString()}`
                : "Not checked yet"}
          </p>
        </div>
        <div className="actions">
          <button className="btn" onClick={runScan} disabled={busy}>
            {scanning ? <span className="spinner" /> : null}
            {scanning ? "Scanning" : "Scan again"}
          </button>
          <button className="btn primary" onClick={runUpdate} disabled={busy || selected.size === 0}>
            {updating ? <span className="spinner light" /> : null}
            {updating ? "Updating…" : `Update selected (${selected.size})`}
          </button>
        </div>
      </header>

      {error && <div className="banner error">{error}</div>}
      {summary && <div className="banner ok">{summary}</div>}
      {warnings.map((w) => (
        <div key={w} className="banner warn">
          {w}
        </div>
      ))}

      {apps.length > 0 && (
        <div className="toolbar">
          <label className="check">
            <input type="checkbox" checked={allSelected} onChange={toggleAll} disabled={busy || !selectable.length} />
            Select all
          </label>
          <input
            className="search"
            placeholder="Filter apps"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
          />
        </div>
      )}

      <main className="list">
        {!scanning && checkedAt && apps.length === 0 && (
          <div className="empty">
            <div className="empty-icon">✓</div>
            <h2>All up to date</h2>
            <p className="muted">No updates found.</p>
          </div>
        )}
        {scanning && apps.length === 0 && (
          <div className="empty">
            <span className="spinner big" />
            <p className="muted">Asking your package manager what's outdated. This can take a minute.</p>
          </div>
        )}
        {visible.map((a) => {
          const st = status[a.key];
          const done = st?.state === "done";
          const open = expanded === a.key;
          return (
            <div key={a.key} className={`row ${done ? "done" : ""}`}>
              <div className="row-main">
                <input
                  type="checkbox"
                  aria-label={`Select ${a.name}`}
                  checked={selected.has(a.key)}
                  onChange={() => toggle(a.key)}
                  disabled={busy || done}
                />
                <div className="name">
                  <div className="title">{a.name}</div>
                  <div className="sub">{a.id}</div>
                </div>
                <div className="versions">
                  <span className="old">{a.current}</span>
                  <span className="arrow">→</span>
                  <span className="new">{a.available}</span>
                </div>
                <span className={`badge src-${a.source}`}>{SOURCE_LABEL[a.source] ?? a.source}</span>
                <div className="state">
                  <StateCell st={st} onLog={() => setExpanded(open ? null : a.key)} />
                </div>
              </div>
              {st?.state === "running" && st.log.length > 0 && <div className="live">{st.log[st.log.length - 1]}</div>}
              {open && st && (
                <pre className="log">{[...st.log, st.error ? `Error: ${st.error}` : ""].filter(Boolean).join("\n")}</pre>
              )}
            </div>
          );
        })}
      </main>
    </div>
  );
}

function StateCell({ st, onLog }: { st?: RowStatus; onLog: () => void }) {
  if (!st) return null;
  switch (st.state) {
    case "queued":
      return <span className="muted">Queued</span>;
    case "running":
      return (
        <span className="pill running">
          <span className="spinner" /> Updating
        </span>
      );
    case "done":
      return (
        <button className="pill done linklike" onClick={onLog}>
          ✓ Updated
        </button>
      );
    case "failed":
      return (
        <button className="pill failed linklike" onClick={onLog} title={st.error}>
          ✗ Failed · log
        </button>
      );
  }
}

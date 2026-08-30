import { useCallback, useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useWorkspaceActions } from "../../hooks/useWorkspace";
import { formatError } from "../../lib/formatError";

function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

// Retourne null si annulation ou hors Tauri, throw si vraie erreur sous Tauri
async function pickFolderNative(title: string): Promise<string | null> {
  const selected = await open({
    directory: true,
    multiple: false,
    title,
  });
  if (typeof selected === "string") return selected;
  const arr = selected as unknown as string[] | null;
  if (Array.isArray(arr) && arr.length > 0) return arr[0];
  return null;
}

function parentDir(p: string): string {
  const sep = p.includes("\\") ? "\\" : "/";
  const idx = p.lastIndexOf(sep);
  return idx > 0 ? p.slice(0, idx) : p;
}

function slugifyName(n: string): string {
  return n.trim().toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "mon-workspace";
}

export default function WorkspaceLauncher() {
  const { createWorkspace, openWorkspace } = useWorkspaceActions();
  const [name, setName] = useState("Mon Workspace");
  const [path, setPath] = useState("");
  const [defaultDir, setDefaultDir] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [dragOver, setDragOver] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  // 1a — dossier par défaut (Documents/Sebtable/<slug>) pour création 1-clic
  useEffect(() => {
    let cancelled = false;
    (async () => {
      if (!isTauri()) return;
      try {
        const p = await import("@tauri-apps/api/path");
        const base = await (p as unknown as { documentDir: () => Promise<string> }).documentDir();
        if (!cancelled && base) setDefaultDir(base.replace(/\/$/, "") + "/Sebtable");
        else throw new Error("no base");
      } catch {
        try {
          const h = await import("@tauri-apps/api/path");
          const home = await (h as unknown as { homeDir: () => Promise<string> }).homeDir();
          if (!cancelled && home) setDefaultDir(home.replace(/\/$/, "") + "/Documents/Sebtable");
        } catch {
          if (!cancelled) setDefaultDir(null);
        }
      }
    })();
    return () => { cancelled = true; };
  }, []);

  const suggestedPath = defaultDir ? `${defaultDir}/${slugifyName(name)}` : "";

  // Drag & drop natif Tauri (WebView) — HTML5 DnD est intercepté quand dragDropEnabled=true
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    (async () => {
      if (!isTauri()) return;
      try {
        const { getCurrentWebview } = await import("@tauri-apps/api/webview");
        const wv = getCurrentWebview();
        unlisten = await wv.onDragDropEvent((event) => {
          if (cancelled) return;
          if (event.payload.type === "over") {
            setDragOver(true);
          } else if (event.payload.type === "drop") {
            setDragOver(false);
            const p = event.payload.paths[0];
            if (p) {
              const low = p.toLowerCase();
              // Si l'utilisateur dépose workspace.json, remonter au dossier parent
              const isFile = low.endsWith(".json");
              setPath(isFile ? parentDir(p) : p);
            }
          } else {
            setDragOver(false);
          }
        });
      } catch {
        // hors Tauri ou webview indisponible — fallback HTML5 ci-dessous
      }
    })();
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const handleCreate = async () => {
    setBusy(true);
    setError("");
    try {
      await createWorkspace(path.trim(), name.trim());
    } catch (e) {
      setError(formatError(e));
    } finally {
      setBusy(false);
    }
  };

  const handleOpen = async () => {
    setBusy(true);
    setError("");
    try {
      await openWorkspace(path.trim());
    } catch (e) {
      setError(formatError(e));
    } finally {
      setBusy(false);
    }
  };

  const handleBrowse = useCallback(async (mode: "create" | "open") => {
    setError("");
    if (isTauri()) {
      try {
        const picked = await pickFolderNative(
          mode === "create" ? "Choisir le dossier du nouveau workspace" : "Ouvrir un workspace existant"
        );
        if (picked) setPath(picked);
        // null = annulation utilisateur, pas d'erreur
        return;
      } catch (e) {
        setError(`Sélecteur système indisponible : ${formatError(e)}`);
        return;
      }
    }
    // Fallback web : webkitdirectory
    fileInputRef.current?.click();
  }, []);

  const handleFileInput = (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files;
    if (!files || files.length === 0) return;
    const first = files[0] as File & { webkitRelativePath?: string; path?: string };
    const raw = (first as unknown as { path?: string }).path;
    if (raw) {
      const dir = parentDir(raw);
      setPath(dir);
    } else if (first.webkitRelativePath) {
      const topFolder = first.webkitRelativePath.split("/")[0];
      setPath(topFolder);
      setError(
        "Navigateur web : saisis le chemin complet manuellement (ex: C:\\Users\\...\\" +
          topFolder +
          "). Le selecteur natif est dispo dans l'app Tauri."
      );
    }
    e.target.value = "";
  };

  // Fallback HTML5 DnD uniquement hors Tauri (sous Tauri, onDragDropEvent gère tout)
  const onDragOver = (e: React.DragEvent) => {
    if (isTauri()) return;
    e.preventDefault();
    setDragOver(true);
  };
  const onDragLeave = (e: React.DragEvent) => {
    if (isTauri()) return;
    e.preventDefault();
    // éviter le flicker quand on survole les enfants
    const rel = e.relatedTarget as Node | null;
    if (rel && e.currentTarget.contains(rel)) return;
    setDragOver(false);
  };
  const onDrop = (e: React.DragEvent) => {
    if (isTauri()) return;
    e.preventDefault();
    setDragOver(false);
    const text = e.dataTransfer.getData("text/plain");
    if (text) {
      const p = text.trim();
      const low = p.toLowerCase();
      const isFile = low.endsWith(".json");
      setPath(isFile ? parentDir(p) : p);
    }
  };

  const canCreate = !busy && path.trim().length > 0 && name.trim().length > 0;
  const canCreateDefault = !busy && !!suggestedPath && name.trim().length > 0;
  const canOpen = !busy && path.trim().length > 0;

  const handleCreateDefault = async () => {
    if (!suggestedPath) return;
    setBusy(true);
    setError("");
    try {
      await createWorkspace(suggestedPath, name.trim());
    } catch (e) {
      setError(formatError(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="launcher">
      <div
        className={`launcher-card${dragOver ? " launcher-drop-active" : ""}`}
        onDragOver={onDragOver}
        onDragLeave={onDragLeave}
        onDrop={onDrop}
        role="region"
        aria-label="Sélecteur de workspace"
      >
        <h1>Sebtable</h1>
        <p style={{ color: "var(--text-muted)", margin: 0 }}>
          Base de données locale type Airtable. Crée un workspace ou ouvre un dossier existant contenant{" "}
          <code>workspace.json</code>.
        </p>

        <div className="modal-field">
          <label htmlFor="ws-path">Chemin du dossier</label>
          <div className="launcher-file-row">
            <input
              id="ws-path"
              value={path}
              placeholder="C:\Users\sebas\Documents\sebtable-workspaces\mon-projet"
              onChange={(e) => setPath(e.target.value)}
              aria-describedby="ws-path-hint"
              onKeyDown={(e) => {
                if (e.key === "Enter" && canOpen) handleOpen();
              }}
            />
            <button type="button" onClick={() => handleBrowse("open")} disabled={busy} aria-label="Parcourir les dossiers">
              📁 Parcourir
            </button>
          </div>
          <span id="ws-path-hint" className="hint">
            Glisse un dossier ici, colle un chemin, ou clique sur Parcourir (sélecteur système natif).
          </span>
          <input
            ref={fileInputRef}
            type="file"
            // @ts-expect-error webkitdirectory non typé
            webkitdirectory=""
            directory=""
            multiple
            style={{ display: "none" }}
            onChange={handleFileInput}
            aria-hidden
            tabIndex={-1}
          />
        </div>

        <div className="modal-field">
          <label htmlFor="ws-name">Nom du workspace (création uniquement)</label>
          <input id="ws-name" value={name} onChange={(e) => setName(e.target.value)} placeholder="Mon Workspace" />
        </div>

        {suggestedPath && (
          <div className="launcher-row" style={{ background: "rgba(109,123,255,0.08)", border: "1px solid var(--border)", borderRadius: 8, padding: 8 }}>
            <div style={{ flex: 1, minWidth: 0 }}>
              <div className="hint" style={{ marginBottom: 4 }}>Création 1-clic dans :</div>
              <code style={{ fontSize: 11, wordBreak: "break-all" }}>{suggestedPath}</code>
            </div>
            <button className="primary" disabled={!canCreateDefault} onClick={handleCreateDefault} title={suggestedPath} aria-label="Créer workspace au dossier par défaut">
              {busy ? "…" : "✨ Créer ici"}
            </button>
          </div>
        )}

        <div className="launcher-row">
          <button className="primary" disabled={!canCreate} onClick={handleCreate} title={canCreate ? "" : "Renseigne chemin + nom"}>
            {busy ? "Création…" : "Créer (chemin custom)"}
          </button>
          <button disabled={!canOpen} onClick={handleOpen}>
            {busy ? "Ouverture…" : "Ouvrir"}
          </button>
          <button type="button" disabled={busy} onClick={() => handleBrowse("create")} title="Choisir où créer le dossier">
            📁 Parcourir…
          </button>
        </div>

        {dragOver && <div className="launcher-drop-hint">Dépose le dossier ici</div>}
        <div className="launcher-error" role="alert">
          {error}
        </div>
        <p className="hint" style={{ margin: 0 }}>
          Astuce : le sélecteur natif (Parcourir) fonctionne dans l'app Tauri. En mode web, saisis le chemin manuellement.
        </p>
      </div>
    </div>
  );
}

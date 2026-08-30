import { useEffect, useState } from "react";
import Modal from "../common/Modal";
import { useUiStore } from "../../stores/uiStore";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import * as api from "../../lib/api";
import { formatError } from "../../lib/formatError";

export default function SettingsModal() {
  const close = useUiStore((s) => s.closeModal);
  const config = useWorkspaceStore((s) => s.config);
  const setConfig = useWorkspaceStore((s) => s.setConfig);
  const [form, setForm] = useState({ llm_provider: "hybrid", lmstudio_url: "http://localhost:1234/v1", lmstudio_model: "auto", openai_api_key: "", openai_model: "gpt-4o-mini" });
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [ok, setOk] = useState(false);

  const hasStoredKey = config?.settings.openai_api_key === "***" || false;

  useEffect(() => {
    if (config) setForm({
      llm_provider: config.settings.llm_provider,
      lmstudio_url: config.settings.lmstudio_url,
      lmstudio_model: config.settings.lmstudio_model,
      openai_api_key: "",
      openai_model: config.settings.openai_model,
    });
  }, [config]);

  const save = async () => {
    setBusy(true); setErr(null); setOk(false);
    try {
      const res = await api.updateWorkspaceSettings(form as any);
      if (config) setConfig({ ...config, settings: res as any });
      setOk(true);
    } catch (e) { setErr(formatError(e)); }
    setBusy(false);
  };

  const removeKey = async () => {
    setBusy(true); setErr(null); setOk(false);
    try {
      const res = await api.updateWorkspaceSettings({ ...form, openai_api_key: "__delete__" } as any);
      if (config) setConfig({ ...config, settings: res as any });
      setForm({ ...form, openai_api_key: "" });
      setOk(true);
    } catch (e) { setErr(formatError(e)); }
    setBusy(false);
  };

  return (
    <Modal title="Paramètres · IA & workspace">
      <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
        <label>Provider IA
          <select value={form.llm_provider} onChange={(e) => setForm({ ...form, llm_provider: e.target.value })}>
            <option value="hybrid">Hybrid (LM Studio → OpenAI)</option>
            <option value="lmstudio">LM Studio seul</option>
            <option value="openai">OpenAI seul</option>
            <option value="off">Désactivé (heuristiques)</option>
          </select>
        </label>
        <label>LM Studio URL
          <input value={form.lmstudio_url} onChange={(e) => setForm({ ...form, lmstudio_url: e.target.value })} placeholder="http://localhost:1234/v1" />
        </label>
        <label>Modèle LM Studio
          <input value={form.lmstudio_model} onChange={(e) => setForm({ ...form, lmstudio_model: e.target.value })} placeholder="auto" />
        </label>
        <label>Modèle OpenAI
          <input value={form.openai_model} onChange={(e) => setForm({ ...form, openai_model: e.target.value })} />
        </label>
        <label>Clé OpenAI
          <input type="password" value={form.openai_api_key} onChange={(e) => setForm({ ...form, openai_api_key: e.target.value })} placeholder={hasStoredKey ? "Clé stockée (laisser vide pour conserver)" : "sk-..."} />
        </label>
        {hasStoredKey && (
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <span className="hint">Une clé est stockée en sécurité dans le keychain. Laisser vide pour la conserver.</span>
            <button onClick={removeKey} disabled={busy}>Supprimer la clé</button>
          </div>
        )}
        {err && <div role="alert" style={{ color: "var(--danger)" }}>{err}</div>}
        {ok && <div style={{ color: "var(--accent)" }}>Enregistré ✅</div>}
        <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
          <button onClick={close} disabled={busy}>Fermer</button>
          <button className="primary" onClick={save} disabled={busy}>{busy ? "…" : "Enregistrer"}</button>
        </div>
        <hr />
        <div className="hint">
          Raccourcis : <kbd>Ctrl+K</kbd> recherche · <kbd>Ctrl+N</kbd> nouvelle table · <kbd>Ctrl+,</kbd> paramètres · <kbd>Ctrl+Shift+I</kbd> IA
        </div>
      </div>
    </Modal>
  );
}

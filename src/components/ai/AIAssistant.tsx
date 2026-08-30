import { useState } from "react";
import FormulaGenerator from "./FormulaGenerator";
import AnalysisPanel from "./AnalysisPanel";
import CleaningWizard from "./CleaningWizard";
import RelationSuggest from "./RelationSuggest";
import { useAIStatus } from "../../hooks/useAI";

type Tab = "formula" | "analyze" | "clean" | "relations";

export default function AIAssistant({ onClose }: { onClose: () => void }) {
  const [tab, setTab] = useState<Tab>("formula");
  const statusQ = useAIStatus();

  return (
    <div style={{ width: 420, borderLeft: "1px solid var(--border)", background: "var(--bg-panel)", display: "flex", flexDirection: "column", minHeight: 0 }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", padding: "8px 12px", borderBottom: "1px solid var(--border)" }}>
        <strong>Assistant IA</strong>
        <button onClick={onClose} aria-label="Fermer assistant">×</button>
      </div>
      <div className="hint" style={{ padding: "6px 12px", borderBottom: "1px solid var(--border)" }}>
        {statusQ.data ? <>Provider: <code>{statusQ.data.active_provider}</code> — LM Studio: {statusQ.data.lmstudio_available ? "✅" : "—"} — OpenAI: {statusQ.data.openai_configured ? "✅" : "—"}</> : "Statut…"}
        <div>Hybride : heuristiques offline + LLM si configuré dans workspace.json.</div>
      </div>
      <div style={{ display: "flex", gap: 4, padding: 8, borderBottom: "1px solid var(--border)", flexWrap: "wrap" }}>
        {([ ["formula","Formule"] as const, ["analyze","Analyse"] as const, ["clean","Nettoyage"] as const, ["relations","Relations"] as const]).map(([k,l])=> (
          <button key={k} className={tab===k ? "primary" : ""} onClick={()=>setTab(k)}>{l}</button>
        ))}
      </div>
      <div style={{ padding: 12, overflowY: "auto", flex: 1 }}>
        {tab==="formula" && <FormulaGenerator />}
        {tab==="analyze" && <AnalysisPanel />}
        {tab==="clean" && <CleaningWizard />}
        {tab==="relations" && <RelationSuggest />}
      </div>
    </div>
  );
}

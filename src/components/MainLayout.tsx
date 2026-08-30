import { useTableStore } from "../stores/tableStore";
import { useUiStore } from "../stores/uiStore";
import { useTableList, useFieldList, useViewList } from "../hooks/useTable";
import { useShortcuts } from "../hooks/useShortcuts";
import DatabaseSwitcher from "./workspace/DatabaseSwitcher";
import WorkspaceSidebar from "./workspace/WorkspaceSidebar";
import DataGrid from "./grid/DataGrid";
import DatabaseHome from "./workspace/DatabaseHome";
import CreateTableModal from "./fields/CreateTableModal";
import CreateFieldModal from "./fields/CreateFieldModal";
import CreateViewModal from "./fields/CreateViewModal";
import CreateDatabaseModal from "./fields/CreateDatabaseModal";
import AIAssistant from "./ai/AIAssistant";
import Modal from "./common/Modal";
import SettingsModal from "./settings/SettingsModal";
import ImportExportPanel from "./import/ImportExportPanel";
import GlobalSearchPalette from "./search/GlobalSearchPalette";
import GalleryView from "./grid/GalleryView";
import KanbanView from "./grid/KanbanView";
import FormView from "./grid/FormView";

export default function MainLayout() {
  useTableList();
  useFieldList();
  useViewList();
  useShortcuts();

  const activeTableId = useTableStore((s) => s.activeTableId);
  const modal = useUiStore((s) => s.modal);
  const aiOpen = useUiStore((s) => s.aiOpen);
  const toggleAi = useUiStore((s) => s.toggleAi);
  const setAiOpen = useUiStore((s) => s.setAiOpen);
  const openModal = useUiStore((s) => s.openModal);
  const viewMode = useUiStore((s) => s.viewMode);
  const setViewMode = useUiStore((s) => s.setViewMode);
  const setSearchOpen = useUiStore((s) => s.setSearchOpen);

  return (
    <div className="app">
      <div className="topbar">
        <span className="title">Sebtable</span>
        <span className="build-tag" title="Version du frontend chargé">v0.1.17</span>
        <DatabaseSwitcher />
        <button onClick={() => setSearchOpen(true)} aria-label="Recherche globale" title="Recherche globale (Ctrl+K)">🔍</button>
        <span style={{ flex: 1 }} />
        <button onClick={() => openModal("importExport")} title="Import / Export (CSV/JSON/XLSX)">⇅ Import</button>
        <button onClick={() => openModal("settings")} aria-label="Paramètres" title="Paramètres (Ctrl+,)">⚙</button>
        <button onClick={toggleAi} aria-label="Assistant IA" title="Assistant IA (Ctrl+Shift+I)">
          {aiOpen ? "✕ IA" : "✦ IA"}
        </button>
      </div>
      <div className="body">
        <WorkspaceSidebar />
        <main className="main">
          {activeTableId ? (
            <>
              <div style={{ display: "flex", gap: 6, padding: "6px 10px", borderBottom: "1px solid var(--border)", background: "var(--bg-panel)", flexWrap: "wrap" }}>
                <button className={viewMode === "grid" ? "primary" : ""} onClick={() => setViewMode("grid")} aria-pressed={viewMode === "grid"}>⊞ Grille</button>
                <button className={viewMode === "gallery" ? "primary" : ""} onClick={() => setViewMode("gallery")} aria-pressed={viewMode === "gallery"}>🖼 Galerie</button>
                <button className={viewMode === "kanban" ? "primary" : ""} onClick={() => setViewMode("kanban")} aria-pressed={viewMode === "kanban"}>📋 Kanban</button>
                <button className={viewMode === "form" ? "primary" : ""} onClick={() => setViewMode("form")} aria-pressed={viewMode === "form"}>📝 Formulaire</button>
                <span className="hint" style={{ marginLeft: 8 }}>
                  {viewMode === "gallery" ? "Cartes · couverture = 1re pièce jointe" : viewMode === "kanban" ? "Kanban · glisser carte pour changer statut" : viewMode === "form" ? "Formulaire · création rapide" : "Tableau · flèches/Shift, Ctrl+C/V, glisser bord/colonne"}
                </span>
              </div>
              {viewMode === "gallery" ? <GalleryView /> : viewMode === "kanban" ? <KanbanView /> : viewMode === "form" ? <FormView /> : <DataGrid />}
            </>
          ) : (
            <DatabaseHome />
          )}
        </main>
        {aiOpen && <AIAssistant onClose={() => setAiOpen(false)} />}
      </div>

      {modal === "createTable" && <CreateTableModal />}
      {modal === "createField" && <CreateFieldModal />}
      {modal === "createView" && <CreateViewModal />}
      {modal === "settings" && <SettingsModal />}
      {modal === "importExport" && (
        <Modal title="Import / Export">
          <ImportExportPanel />
        </Modal>
      )}
      <CreateDatabaseModal />
      <GlobalSearchPalette />
    </div>
  );
}

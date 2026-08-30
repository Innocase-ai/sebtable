import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useWorkspaceActions } from "../../hooks/useWorkspace";
import { useTableStore } from "../../stores/tableStore";
import { useUiStore } from "../../stores/uiStore";
import { formatError } from "../../lib/formatError";
import * as api from "../../lib/api";
import TemplatePicker from "./TemplatePicker";

function initials(name: string) {
  const parts = name.trim().split(/\s+/).filter(Boolean);
  if (parts.length >= 2) return (parts[0][0] + parts[1][0]).toUpperCase();
  return name.slice(0, 2).toUpperCase() || "?";
}

function colorFor(id: string) {
  const hues = ["#6d7bff", "#ff6b9d", "#4ecdc4", "#feca57", "#a55eea", "#26de81", "#fd9644"];
  let h = 0;
  for (let i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) >>> 0;
  return hues[h % hues.length];
}

export default function DatabaseHome() {
  const config = useWorkspaceStore((s) => s.config);
  const { switchDatabase } = useWorkspaceActions();
  const setActiveTable = useTableStore((s) => s.setActiveTable);
  const openModal = useUiStore((s) => s.openModal);
  const queryClient = useQueryClient();
  const [err, setErr] = useState<string | null>(null);

  const dbs = config?.databases ?? [];
  const isEmpty = dbs.length === 0;

  // F1 : ouvrir une base doit « entrer » dedans -> sélection auto de la 1re table
  const openFirstTable = async (dbId: string) => {
    const tables = await queryClient.fetchQuery({
      queryKey: ["tables", dbId],
      queryFn: () => api.listTables(dbId),
      staleTime: 5_000,
    });
    setActiveTable(tables[0]?.id ?? null);
  };

  const handleOpen = async (dbId: string) => {
    setErr(null);
    try {
      if (dbId !== config?.active_database_id) {
        await switchDatabase(dbId);
      }
      await openFirstTable(dbId);
    } catch (e) {
      setErr(formatError(e));
    }
  };

  return (
    <div className="db-home" aria-label="Accueil bases de données">
      <div className="db-home-header">
        <h1 className="db-home-title">Vos bases Sebtable</h1>
        <p className="db-home-subtitle">
          {isEmpty ? "Aucune base pour l'instant — créez la première." : "Sélectionnez une base ou créez-en une nouvelle."}
        </p>
      </div>

      {/* Cadre principal : liste des bases déjà créées, comme demandé */}
      <section className="db-home-frame" aria-labelledby="db-home-frame-title">
        <div className="db-home-frame-head">
          <h2 id="db-home-frame-title" className="db-home-frame-title">
            Ouvert(es) n&apos;importe quand <span aria-hidden>▾</span>
          </h2>
          <span className="db-home-count">{dbs.length} base{dbs.length !== 1 ? "s" : ""}</span>
        </div>

        {isEmpty ? (
          <div className="db-home-empty">
            <p>Aucune base existante dans ce workspace.</p>
            <button className="primary db-home-create-big" onClick={() => openModal("createDatabase")} aria-label="Créer une base de données">
              <span aria-hidden>+</span> Créer une base
            </button>
          </div>
        ) : (
          <>
            <p className="db-home-section-label">Aujourd&apos;hui</p>
            <div className="db-home-grid">
              {dbs.map((db) => {
                const active = db.id === config?.active_database_id;
                return (
                  <button
                    key={db.id}
                    className={`db-card ${active ? "db-card--active" : ""}`}
                    onClick={() => void handleOpen(db.id)}
                    aria-label={`Ouvrir la base ${db.name}${active ? " (active)" : ""}`}
                    title={active ? "Base active — cliquer pour ouvrir sa première table" : `Ouvrir ${db.name}`}
                  >
                    <span className="db-card-icon" style={{ background: colorFor(db.id) }} aria-hidden>
                      {initials(db.name)}
                    </span>
                    <span className="db-card-body">
                      <span className="db-card-name">{db.name}</span>
                      <span className="db-card-meta">
                        {db.role === "reference" ? "Référentiel" : "Projet"}{active ? " · Active" : ""}
                      </span>
                    </span>
                    {active && <span className="db-card-badge" aria-hidden>●</span>}
                  </button>
                );
              })}
              {/* Carte + toujours visible quand il y a déjà des bases */}
              <button className="db-card db-card--new" onClick={() => openModal("createDatabase")} aria-label="Créer une nouvelle base">
                <span className="db-card-icon db-card-icon--new" aria-hidden>+</span>
                <span className="db-card-body">
                  <span className="db-card-name">Nouvelle base</span>
                  <span className="db-card-meta">Créer à partir de zéro</span>
                </span>
              </button>
            </div>
          </>
        )}
        {err && (
          <p role="alert" className="db-home-error">
            {err} <button onClick={() => setErr(null)} style={{ marginLeft: 8 }}>×</button>
          </p>
        )}
      </section>

      <TemplatePicker />

      <p className="db-home-hint">
        Astuce : vous pouvez aussi créer/ouvrir une base via la barre latérale « Bases » → <strong>+ Nouvelle base</strong>.
      </p>
    </div>
  );
}

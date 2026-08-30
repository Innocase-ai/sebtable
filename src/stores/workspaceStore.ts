import { create } from "zustand";
import type { Database, WorkspaceConfig } from "../types/workspace";

interface WorkspaceState {
  config: WorkspaceConfig | null;
  setConfig: (c: WorkspaceConfig | null) => void;
  addDatabase: (db: Database) => void;
  setActiveDatabase: (id: string) => void;
}

export const useWorkspaceStore = create<WorkspaceState>((set) => ({
  config: null,
  setConfig: (c) => set({ config: c }),
  addDatabase: (db) =>
    set((s) =>
      s.config
        ? { config: { ...s.config, databases: [...s.config.databases, db] } }
        : {}
    ),
  setActiveDatabase: (id) =>
    set((s) =>
      s.config ? { config: { ...s.config, active_database_id: id } } : {}
    ),
}));

import { create } from "zustand";
import type { Table } from "../types/database";
import type { Field, View, ViewConfig } from "../types/field";

const EMPTY_VIEW_CONFIG: ViewConfig = {
  filters: [],
  filter_conjunction: "and",
  sorts: [],
  groups: [],
};

interface TableState {
  tables: Table[];
  fields: Field[];
  views: View[];
  activeTableId: string | null;
  activeViewId: string | null;
  viewConfig: ViewConfig;
  setTables: (t: Table[]) => void;
  setFields: (f: Field[]) => void;
  setViews: (v: View[]) => void;
  setActiveTable: (id: string | null) => void;
  setActiveView: (id: string | null) => void;
  setViewConfig: (c: ViewConfig) => void;
  reset: () => void;
}

export const useTableStore = create<TableState>((set, get) => ({
  tables: [],
  fields: [],
  views: [],
  activeTableId: null,
  activeViewId: null,
  viewConfig: EMPTY_VIEW_CONFIG,
  setTables: (tables) => set({ tables }),
  setFields: (fields) => set({ fields }),
  setViews: (views) => {
    // Si la vue active a disparu, réinitialiser le filtre
    const { activeViewId } = get();
    if (activeViewId && !views.some((v) => v.id === activeViewId)) {
      set({ views, activeViewId: null, viewConfig: EMPTY_VIEW_CONFIG });
    } else {
      set({ views });
    }
  },
  setActiveTable: (activeTableId) =>
    set({
      activeTableId,
      activeViewId: null,
      viewConfig: EMPTY_VIEW_CONFIG,
    }),
  setActiveView: (activeViewId) => {
    if (!activeViewId) {
      set({ activeViewId: null, viewConfig: EMPTY_VIEW_CONFIG });
      return;
    }
    const view = get().views.find((v) => v.id === activeViewId);
    if (view) {
      // Restaurer la config de la vue (sans conserver la pagination stockée)
      const cfg: ViewConfig = {
        filters: view.config.filters ?? [],
        filter_conjunction: (view.config.filter_conjunction as "and" | "or") ?? "and",
        sorts: view.config.sorts ?? [],
        groups: view.config.groups ?? [],
        visible_field_ids: view.config.visible_field_ids ?? null,
        page: null,
      };
      set({ activeViewId, viewConfig: cfg });
    } else {
      set({ activeViewId });
    }
  },
  setViewConfig: (viewConfig) => set({ viewConfig }),
  reset: () =>
    set({
      tables: [],
      fields: [],
      views: [],
      activeTableId: null,
      activeViewId: null,
      viewConfig: EMPTY_VIEW_CONFIG,
    }),
}));

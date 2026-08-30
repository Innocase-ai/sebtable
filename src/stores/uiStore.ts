import { create } from "zustand";

type ModalName = "createTable" | "createField" | "createView" | "createDatabase" | "settings" | "importExport" | null;

interface UiState {
  modal: ModalName;
  openModal: (m: ModalName) => void;
  closeModal: () => void;
  aiOpen: boolean;
  toggleAi: () => void;
  setAiOpen: (v: boolean) => void;
  searchOpen: boolean;
  toggleSearch: () => void;
  setSearchOpen: (v: boolean) => void;
  viewMode: "grid" | "gallery" | "kanban" | "form";
  setViewMode: (v: "grid" | "gallery" | "kanban" | "form") => void;
}

export const useUiStore = create<UiState>((set) => ({
  modal: null,
  openModal: (modal) => set({ modal }),
  closeModal: () => set({ modal: null }),
  aiOpen: false,
  toggleAi: () => set((s) => ({ aiOpen: !s.aiOpen })),
  setAiOpen: (aiOpen) => set({ aiOpen }),
  searchOpen: false,
  toggleSearch: () => set((s) => ({ searchOpen: !s.searchOpen })),
  setSearchOpen: (searchOpen) => set({ searchOpen }),
  viewMode: "grid",
  setViewMode: (viewMode) => set({ viewMode }),
}));

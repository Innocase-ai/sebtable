import { useCallback } from "react";
import * as api from "../lib/api";
import { useWorkspaceStore } from "../stores/workspaceStore";
import { useTableStore } from "../stores/tableStore";
import type { DbRole } from "../types/workspace";

export function useWorkspaceActions() {
  const setConfig = useWorkspaceStore((s) => s.setConfig);
  const addDatabase = useWorkspaceStore((s) => s.addDatabase);
  const resetTable = useTableStore((s) => s.reset);

  const createWorkspace = useCallback(
    async (dir: string, name: string) => {
      const config = await api.createWorkspace(dir, name);
      setConfig(config);
      return config;
    },
    [setConfig]
  );

  const openWorkspace = useCallback(
    async (path: string) => {
      const config = await api.openWorkspace(path);
      resetTable();
      setConfig(config);
      return config;
    },
    [setConfig, resetTable]
  );

  const createDatabase = useCallback(
    async (name: string, role: DbRole) => {
      const db = await api.createDatabase(name, role);
      addDatabase(db);
      return db;
    },
    [addDatabase]
  );

  const switchDatabase = useCallback(
    async (dbId: string) => {
      const config = await api.switchDatabase(dbId);
      resetTable();
      setConfig(config);
      return config;
    },
    [setConfig, resetTable]
  );

  const deleteDatabase = useCallback(
    async (dbId: string) => {
      const config = await api.deleteDatabase(dbId);
      resetTable();
      setConfig(config);
      return config;
    },
    [setConfig, resetTable]
  );

  return { createWorkspace, openWorkspace, createDatabase, switchDatabase, deleteDatabase };
}

export function useActiveDbId(): string | undefined {
  return useWorkspaceStore((s) => s.config?.active_database_id);
}

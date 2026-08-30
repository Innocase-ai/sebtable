import { useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import * as api from "../lib/api";
import { useWorkspaceStore } from "../stores/workspaceStore";
import { useTableStore } from "../stores/tableStore";

// R1 : paradigme unifié — React Query partout (auparavant useEffect manuel).
// Les stores Zustand restent la source de vérité UI pour tables/fields/views,
// mais les données sont désormais fetched/cachées via React Query (même
// mécanisme que useTableData).

export function useTableList() {
  const dbId = useWorkspaceStore((s) => s.config?.active_database_id);
  const setTables = useTableStore((s) => s.setTables);

  const q = useQuery({
    queryKey: ["tables", dbId],
    queryFn: () => api.listTables(dbId!),
    enabled: !!dbId,
  });

  useEffect(() => {
    if (q.data) setTables(q.data);
  }, [q.data, setTables]);
  useEffect(() => {
    if (!dbId) setTables([]);
  }, [dbId, setTables]);

  return q;
}

export function useFieldList() {
  const dbId = useWorkspaceStore((s) => s.config?.active_database_id);
  const tableId = useTableStore((s) => s.activeTableId);
  const setFields = useTableStore((s) => s.setFields);

  const q = useQuery({
    queryKey: ["fields", dbId, tableId],
    queryFn: () => api.listFields(dbId!, tableId!),
    enabled: !!dbId && !!tableId,
  });

  useEffect(() => {
    if (q.data) setFields(q.data);
  }, [q.data, setFields]);
  useEffect(() => {
    if (!dbId || !tableId) setFields([]);
  }, [dbId, tableId, setFields]);

  return q;
}

export function useViewList() {
  const dbId = useWorkspaceStore((s) => s.config?.active_database_id);
  const tableId = useTableStore((s) => s.activeTableId);
  const setViews = useTableStore((s) => s.setViews);

  const q = useQuery({
    queryKey: ["views", dbId, tableId],
    queryFn: () => api.listViews(dbId!, tableId!),
    enabled: !!dbId && !!tableId,
  });

  useEffect(() => {
    if (q.data) setViews(q.data);
  }, [q.data, setViews]);
  useEffect(() => {
    if (!dbId || !tableId) setViews([]);
  }, [dbId, tableId, setViews]);

  return q;
}

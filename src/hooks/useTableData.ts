import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import * as api from "../lib/api";
import { useWorkspaceStore } from "../stores/workspaceStore";
import { useTableStore } from "../stores/tableStore";
import type { PaginatedRecords, Record as TableRecord } from "../types/record";

const KEY = "table-data";

export function useTableData() {
  const dbId = useWorkspaceStore((s) => s.config?.active_database_id);
  const tableId = useTableStore((s) => s.activeTableId);
  const viewConfig = useTableStore((s) => s.viewConfig);

  return useQuery({
    queryKey: [KEY, dbId, tableId, viewConfig],
    queryFn: () => api.getTableData(dbId!, tableId!, viewConfig),
    enabled: !!dbId && !!tableId,
  });
}

type Rollback = [readonly unknown[], unknown][];

function applyUpsert(
  old: PaginatedRecords,
  records: TableRecord[]
): PaginatedRecords {
  const byId = new Map<string, Record<string, unknown>>();
  for (const r of old.records) byId.set((r as { _id: string })._id, r as Record<string, unknown>);
  let added = 0;
  for (const rec of records as { _id: string; [k: string]: unknown }[]) {
    const existing = byId.get(rec._id);
    if (existing) byId.set(rec._id, { ...existing, ...rec });
    else {
      byId.set(rec._id, { ...rec });
      added++;
    }
  }
  return {
    ...old,
    records: Array.from(byId.values()) as PaginatedRecords["records"],
    total: old.total + added,
  };
}

function applyDelete(old: PaginatedRecords, ids: string[]): PaginatedRecords {
  const idSet = new Set(ids);
  const kept = old.records.filter((r) => !idSet.has((r as { _id: string })._id));
  const removed = old.records.length - kept.length;
  return { ...old, records: kept, total: Math.max(0, old.total - removed) };
}

// Mutations optimistes : l'UI réagit immédiatement, le backend suit.
// `cancelQueries` annule les refetch en vol pour éviter qu'une réponse
// obsolète n'écrase la mise à jour optimiste (cause du « rien ne s'affiche »).
export function useUpsertRecords() {
  const dbId = useWorkspaceStore((s) => s.config?.active_database_id);
  const tableId = useTableStore((s) => s.activeTableId);
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (records: TableRecord[]) => {
      if (!dbId || !tableId) return Promise.resolve([] as TableRecord[]);
      return api.upsertRecords(dbId, tableId, records);
    },
    onMutate: async (records): Promise<Rollback> => {
      if (!dbId || !tableId) return [];
      const filter = { queryKey: [KEY, dbId, tableId] };
      await queryClient.cancelQueries(filter);
      const prev = queryClient.getQueriesData<PaginatedRecords>(filter);
      queryClient.setQueriesData<PaginatedRecords>(filter, (old) => {
        if (!old) return old as PaginatedRecords | undefined;
        return applyUpsert(old, records);
      });
      return prev;
    },
    onError: (_err, _vars, ctx) => {
      for (const [key, data] of ctx ?? []) queryClient.setQueryData(key, data);
    },
    onSettled: () => {
      if (!dbId || !tableId) return;
      queryClient.invalidateQueries({ queryKey: [KEY, dbId, tableId] });
    },
  }).mutateAsync;
}

export function useDeleteRecords() {
  const dbId = useWorkspaceStore((s) => s.config?.active_database_id);
  const tableId = useTableStore((s) => s.activeTableId);
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (ids: string[]) => {
      if (!dbId || !tableId) return Promise.resolve();
      return api.deleteRecords(dbId, tableId, ids);
    },
    onMutate: async (ids): Promise<Rollback> => {
      if (!dbId || !tableId) return [];
      const filter = { queryKey: [KEY, dbId, tableId] };
      await queryClient.cancelQueries(filter);
      const prev = queryClient.getQueriesData<PaginatedRecords>(filter);
      queryClient.setQueriesData<PaginatedRecords>(filter, (old) => {
        if (!old) return old as PaginatedRecords | undefined;
        return applyDelete(old, ids);
      });
      return prev;
    },
    onError: (_err, _vars, ctx) => {
      for (const [key, data] of ctx ?? []) queryClient.setQueryData(key, data);
    },
    onSettled: () => {
      if (!dbId || !tableId) return;
      queryClient.invalidateQueries({ queryKey: [KEY, dbId, tableId] });
    },
  }).mutateAsync;
}

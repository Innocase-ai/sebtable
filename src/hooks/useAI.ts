import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import * as api from "../lib/api";
import type { TransformPlan } from "../types/ai";

export function useAIGenerateFormula() {
  return useMutation({
    mutationFn: ({ dbId, tableId, prompt }: { dbId: string; tableId: string; prompt: string }) =>
      api.aiGenerateFormula(dbId, tableId, prompt),
  });
}

export function useAIAnalyze() {
  return useMutation({
    mutationFn: ({ dbId, tableId, question }: { dbId: string; tableId: string; question?: string }) =>
      api.aiAnalyze(dbId, tableId, question ?? null),
  });
}

export function useAICleanPreview() {
  return useMutation({
    mutationFn: ({ dbId, tableId, instruction }: { dbId: string; tableId: string; instruction: string }) =>
      api.aiCleanPreview(dbId, tableId, instruction),
  });
}

export function useAIApply() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ dbId, tableId, plan }: { dbId: string; tableId: string; plan: TransformPlan }) =>
      api.aiApplyTransform(dbId, tableId, plan),
    onSuccess: (_data, vars) => {
      qc.invalidateQueries({ queryKey: ["table-data", vars.dbId, vars.tableId] });
      qc.invalidateQueries({ queryKey: ["table-data", vars.dbId] });
    },
  });
}

export function useAIStatus() {
  return useQuery({
    queryKey: ["ai-status"],
    queryFn: () => api.aiCheckStatus(),
  });
}

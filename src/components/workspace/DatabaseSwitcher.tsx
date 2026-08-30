import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useWorkspaceActions } from "../../hooks/useWorkspace";

export default function DatabaseSwitcher() {
  const config = useWorkspaceStore((s) => s.config);
  const { switchDatabase } = useWorkspaceActions();
  if (!config) return null;

  return (
    <select
      value={config.active_database_id}
      onChange={(e) => void switchDatabase(e.target.value)}
    >
      {config.databases.map((db) => (
        <option key={db.id} value={db.id}>
          {db.name}
          {db.role === "reference" ? " (référentiel)" : ""}
        </option>
      ))}
    </select>
  );
}

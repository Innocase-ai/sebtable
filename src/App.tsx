import { useWorkspaceStore } from "./stores/workspaceStore";
import WorkspaceLauncher from "./components/workspace/WorkspaceLauncher";
import MainLayout from "./components/MainLayout";

export default function App() {
  const config = useWorkspaceStore((s) => s.config);
  if (!config) return <WorkspaceLauncher />;
  return <MainLayout />;
}

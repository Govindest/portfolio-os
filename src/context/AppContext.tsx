import * as React from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type AppState, type ConnectionDto } from "@/lib/api";

interface AppContextValue {
  state: AppState | undefined;
  loading: boolean;
  activeConnection: ConnectionDto | null;
  activeAccountId: string | null;
  setActiveConnectionId: (id: string) => void;
  refresh: () => void;
}

const AppContext = React.createContext<AppContextValue | null>(null);

export function AppProvider({ children }: { children: React.ReactNode }) {
  const qc = useQueryClient();
  const { data: state, isLoading } = useQuery({
    queryKey: ["app-state"],
    queryFn: api.getAppState,
  });
  const [selectedId, setSelectedId] = React.useState<string | null>(null);

  const activeConnection = React.useMemo(() => {
    const conns = state?.connections ?? [];
    if (conns.length === 0) return null;
    return conns.find((c) => c.id === selectedId) ?? conns[0];
  }, [state, selectedId]);

  const activeAccountId =
    activeConnection?.default_account_id ?? activeConnection?.accounts[0]?.id ?? null;

  // Apply theme.
  React.useEffect(() => {
    const theme = state?.settings.theme ?? "system";
    const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
    const dark = theme === "dark" || (theme === "system" && prefersDark);
    document.documentElement.classList.toggle("dark", dark);
  }, [state?.settings.theme]);

  const value: AppContextValue = {
    state,
    loading: isLoading,
    activeConnection,
    activeAccountId,
    setActiveConnectionId: setSelectedId,
    refresh: () => {
      void qc.invalidateQueries();
    },
  };

  return <AppContext.Provider value={value}>{children}</AppContext.Provider>;
}

export function useApp(): AppContextValue {
  const ctx = React.useContext(AppContext);
  if (!ctx) throw new Error("useApp must be used inside AppProvider");
  return ctx;
}

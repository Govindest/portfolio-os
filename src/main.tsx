import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { HashRouter } from "react-router";
import { TooltipProvider } from "@/components/ui/tooltip";
import { AppProvider } from "@/context/AppContext";
import App from "./App";
import "./index.css";

// Plain-browser development (npm run dev without Tauri): simulate the backend
// so the UI is fully explorable. Stripped from production desktop builds.
if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
  const { installDevMock } = await import("./lib/devMock");
  installDevMock();
}

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: 1, staleTime: 5_000, refetchOnWindowFocus: false },
  },
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <HashRouter>
        <AppProvider>
          <TooltipProvider delayDuration={200}>
            <App />
          </TooltipProvider>
        </AppProvider>
      </HashRouter>
    </QueryClientProvider>
  </React.StrictMode>,
);

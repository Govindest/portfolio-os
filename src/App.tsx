import { Suspense, lazy } from "react";
import { Route, Routes } from "react-router";
import { Layout } from "@/components/Layout";
import { useApp } from "@/context/AppContext";
import { Skeleton } from "@/components/ui/skeleton";
import Onboarding from "@/pages/Onboarding";

// Overview is the landing route, so it stays in the main chunk. The rest —
// including the Recharts-heavy pages — load on demand to keep startup light.
const Overview = lazy(() => import("@/pages/Overview"));
const Portfolio = lazy(() => import("@/pages/Portfolio"));
const Strategies = lazy(() => import("@/pages/Strategies"));
const Rebalance = lazy(() => import("@/pages/Rebalance"));
const ActivityPage = lazy(() => import("@/pages/Activity"));
const Brokerages = lazy(() => import("@/pages/Brokerages"));
const SettingsPage = lazy(() => import("@/pages/Settings"));

function RouteFallback() {
  return (
    <div className="space-y-4" aria-busy="true">
      <Skeleton className="h-8 w-48" />
      <Skeleton className="h-24 w-full" />
      <Skeleton className="h-64 w-full" />
    </div>
  );
}

export default function App() {
  const { state, loading } = useApp();

  if (loading || !state) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="w-72 space-y-3">
          <Skeleton className="h-8 w-40" />
          <Skeleton className="h-4 w-full" />
          <Skeleton className="h-4 w-2/3" />
        </div>
      </div>
    );
  }

  if (!state.onboarding_completed) {
    return <Onboarding />;
  }

  return (
    <Routes>
      <Route
        element={
          <Suspense fallback={<RouteFallback />}>
            <Layout />
          </Suspense>
        }
      >
        <Route index element={<Overview />} />
        <Route path="portfolio" element={<Portfolio />} />
        <Route path="strategies" element={<Strategies />} />
        <Route path="rebalance" element={<Rebalance />} />
        <Route path="activity" element={<ActivityPage />} />
        <Route path="brokerages" element={<Brokerages />} />
        <Route path="settings" element={<SettingsPage />} />
      </Route>
    </Routes>
  );
}

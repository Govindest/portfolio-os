import { Route, Routes } from "react-router";
import { Layout } from "@/components/Layout";
import { useApp } from "@/context/AppContext";
import { Skeleton } from "@/components/ui/skeleton";
import Onboarding from "@/pages/Onboarding";
import Overview from "@/pages/Overview";
import Portfolio from "@/pages/Portfolio";
import Strategies from "@/pages/Strategies";
import Rebalance from "@/pages/Rebalance";
import ActivityPage from "@/pages/Activity";
import Brokerages from "@/pages/Brokerages";
import SettingsPage from "@/pages/Settings";

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
      <Route element={<Layout />}>
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

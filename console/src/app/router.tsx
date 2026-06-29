import { createBrowserRouter, Navigate } from "react-router-dom";
import { AppShell } from "./AppShell";
import { RequireAdmin, RequireAuth, useAuth } from "@/lib/auth";
import { Login } from "@/routes/Login";
import { Reset } from "@/routes/Reset";
import { Overview } from "@/routes/admin/Overview";
import { Workers } from "@/routes/admin/Workers";
import { Sessions } from "@/routes/admin/Sessions";
import { Logs } from "@/routes/admin/Logs";
import { Metrics } from "@/routes/admin/Metrics";
import { UsersPage } from "@/routes/admin/Users";
import { Billing } from "@/routes/admin/Billing";
import { Audit } from "@/routes/admin/Audit";
import { Dashboard } from "@/routes/user/Dashboard";
import { ApiKeys } from "@/routes/user/ApiKeys";
import { Recharge } from "@/routes/user/Recharge";
import { Security } from "@/routes/user/Security";

function Home() {
  const { isAdmin } = useAuth();
  return <Navigate to={isAdmin ? "/overview" : "/dashboard"} replace />;
}

const admin = (el: React.ReactNode) => <RequireAdmin>{el}</RequireAdmin>;

export const router = createBrowserRouter([
  { path: "/login", element: <Login /> },
  { path: "/reset", element: <Reset /> },
  {
    path: "/",
    element: (
      <RequireAuth>
        <AppShell />
      </RequireAuth>
    ),
    children: [
      { index: true, element: <Home /> },
      { path: "overview", element: admin(<Overview />) },
      { path: "workers", element: admin(<Workers />) },
      { path: "sessions", element: admin(<Sessions />) },
      { path: "logs", element: admin(<Logs />) },
      { path: "metrics", element: admin(<Metrics />) },
      { path: "users", element: admin(<UsersPage />) },
      { path: "billing", element: admin(<Billing />) },
      { path: "audit", element: admin(<Audit />) },
      { path: "dashboard", element: <Dashboard /> },
      { path: "keys", element: <ApiKeys /> },
      { path: "recharge", element: <Recharge /> },
      { path: "security", element: <Security /> },
    ],
  },
  { path: "*", element: <Navigate to="/" replace /> },
]);

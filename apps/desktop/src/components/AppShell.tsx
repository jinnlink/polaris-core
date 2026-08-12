import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Navigate, NavLink, Outlet, useLocation } from "react-router-dom";

import { routeDefinitions } from "../app/routes";
import {
  commandKeys,
  getLifecycleStatus,
  getStatus,
  normalizeCommandError,
  pollBackgroundEvents,
  setWindowMode,
} from "../lib/commands";

export function AppShell() {
  const client = useQueryClient();
  const location = useLocation();
  const isMapRoute = location.pathname === "/map";
  const [windowMode, setLocalWindowMode] = useState<"compact" | "workspace">("compact");
  const lifecycle = useQuery({ queryKey: commandKeys.lifecycle, queryFn: getLifecycleStatus, retry: 1 });
  const ready = lifecycle.data?.startup_status === "ready";
  const hasPendingBackgroundJobs = (lifecycle.data?.pending_background_jobs.length ?? 0) > 0;
  const status = useQuery({
    queryKey: commandKeys.status,
    queryFn: getStatus,
    staleTime: 30_000,
    retry: 1,
    enabled: ready,
  });
  const error = status.error ? normalizeCommandError(status.error) : null;
  useEffect(() => {
    if (!ready || !hasPendingBackgroundJobs) return;
    const interval = window.setInterval(() => {
      void pollBackgroundEvents().then(async (events) => {
        if (events.length === 0) return;
        await client.invalidateQueries({ queryKey: commandKeys.lifecycle });
        if (events.some((event) => event.status === "finished")) {
          await client.invalidateQueries({ queryKey: ["core"] });
        }
      });
    }, 1000);
    return () => { window.clearInterval(interval); };
  }, [client, hasPendingBackgroundJobs, ready]);

  if (lifecycle.isPending) return <div className="route-loading" role="status">正在检查本地数据库与恢复状态…</div>;
  if (lifecycle.error) {
    const lifecycleError = normalizeCommandError(lifecycle.error);
    return <div className="route-error" role="alert"><strong>无法读取本地恢复状态</strong><span>{lifecycleError.message}</span><button type="button" onClick={() => { void lifecycle.refetch(); }}>重试</button></div>;
  }
  if (!ready && location.pathname !== "/settings") return <Navigate to="/settings" replace />;

  return (
    <div className="app-frame" data-window-mode={windowMode} data-route={isMapRoute ? "map" : "default"}>
      <a className="skip-link" href="#main-content">跳到主要内容</a>
      <aside className="sidebar" aria-label="主导航" aria-hidden={isMapRoute || undefined}>
        <header className="brand">
          <span className="brand__mark" aria-hidden="true">P</span>
          <div>
            <strong>Polaris</strong>
            <span>Porcelain Intelligence</span>
          </div>
          <button
            className="window-mode-button"
            type="button"
            onClick={() => {
              const nextMode = windowMode === "compact" ? "workspace" : "compact";
              void setWindowMode(nextMode).then(() => {
                setLocalWindowMode(nextMode);
              });
            }}
          >
            {windowMode === "compact" ? "展开" : "小窗"}
          </button>
        </header>
        <nav className="navigation">
          {routeDefinitions.map((route) => (
            <NavLink
              key={route.key}
              to={route.path}
              end={route.path === "/"}
              className={({ isActive }) => isActive ? "nav-link nav-link--active" : "nav-link"}
            >
              {route.label}
            </NavLink>
          ))}
        </nav>
        <div className="engine-state" aria-live="polite">
          <span className={status.isSuccess ? "state-dot state-dot--ready" : "state-dot"} />
          {status.isPending && "正在连接本地 Core…"}
          {status.isSuccess && `${status.data.current_pack ?? "尚未选择 Pack"} · 本地`}
          {error && error.message}
        </div>
      </aside>
      <main id="main-content" className="workspace" tabIndex={-1}>
        <Outlet />
      </main>
    </div>
  );
}

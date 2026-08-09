import { useQuery } from "@tanstack/react-query";
import { NavLink, Outlet } from "react-router-dom";

import { routeDefinitions } from "../app/routes";
import {
  commandKeys,
  getStatus,
  normalizeCommandError,
} from "../lib/commands";

export function AppShell() {
  const status = useQuery({
    queryKey: commandKeys.status,
    queryFn: getStatus,
    staleTime: 30_000,
    retry: 1,
  });
  const error = status.error ? normalizeCommandError(status.error) : null;

  return (
    <div className="app-frame">
      <a className="skip-link" href="#main-content">跳到主要内容</a>
      <aside className="sidebar" aria-label="主导航">
        <header className="brand">
          <span className="brand__mark" aria-hidden="true">P</span>
          <div>
            <strong>Polaris</strong>
            <span>Porcelain Intelligence</span>
          </div>
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

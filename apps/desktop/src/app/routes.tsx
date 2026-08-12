import { lazy, Suspense } from "react";
import { createHashRouter } from "react-router-dom";

import { AppShell } from "../components/AppShell";
import { TodayPage } from "../components/TodayPage";

const MapPage = lazy(async () => {
  const module = await import("../components/MapPage");
  return { default: module.MapPage };
});
const PracticePage = lazy(async () => {
  const module = await import("../components/PracticePage");
  return { default: module.PracticePage };
});
const InboxPage = lazy(async () => {
  const module = await import("../components/InboxPage");
  return { default: module.InboxPage };
});
const ProfilePage = lazy(async () => {
  const module = await import("../components/ProfilePage");
  return { default: module.ProfilePage };
});
const GoalsPage = lazy(async () => {
  const module = await import("../components/GoalsPage");
  return { default: module.GoalsPage };
});
const ReportsPage = lazy(async () => {
  const module = await import("../components/ReportsPage");
  return { default: module.ReportsPage };
});
const TrustPage = lazy(async () => {
  const module = await import("../components/TrustPage");
  return { default: module.TrustPage };
});
const SettingsPage = lazy(async () => {
  const module = await import("../components/SettingsPage");
  return { default: module.SettingsPage };
});

export const routeDefinitions = [
  { path: "/", key: "today", label: "Today", description: "今天最值得完成的 2–3 个行动。" },
  { path: "/map", key: "map", label: "Map", description: "查看当前、预测与全局知识地图。" },
  { path: "/practice", key: "practice", label: "Practice", description: "完成证据绑定的练习回合。" },
  { path: "/inbox", key: "inbox", label: "Inbox", description: "处理已记录的学习线索。" },
  { path: "/profile", key: "profile", label: "Profile", description: "理解画像、边界和验证状态。" },
  { path: "/goals", key: "goals", label: "Goals", description: "选择目标并查看范围内行动。" },
  { path: "/reports", key: "reports", label: "Reports", description: "查看有证据来源的学习报告。" },
  { path: "/trust", key: "trust", label: "Trust", description: "检查实验门、隐私与数据主权。" },
  { path: "/settings", key: "settings", label: "Settings", description: "管理本地体验和应用设置。" },
] as const;

export const router = createHashRouter([
  {
    element: <AppShell />,
    children: routeDefinitions.map((route) => ({
      path: route.path,
      element: route.key === "today" ? <TodayPage /> : route.key === "map" ? (
        <Suspense fallback={<section className="map-loading" role="status">正在加载知识地图…</section>}>
          <MapPage />
        </Suspense>
      ) : route.key === "practice" ? (
        <Suspense fallback={<div className="route-loading" role="status">正在恢复学习现场…</div>}>
          <PracticePage />
        </Suspense>
      ) : route.key === "inbox" ? (
        <Suspense fallback={<div className="route-loading" role="status">正在读取收件箱…</div>}>
          <InboxPage />
        </Suspense>
      ) : route.key === "profile" ? (
        <Suspense fallback={<div className="route-loading" role="status">正在整理画像证据…</div>}>
          <ProfilePage />
        </Suspense>
      ) : route.key === "goals" ? (
        <Suspense fallback={<div className="route-loading" role="status">正在计算目标路径…</div>}>
          <GoalsPage />
        </Suspense>
      ) : route.key === "reports" ? (
        <Suspense fallback={<div className="route-loading" role="status">正在整理有出处的学习信号…</div>}>
          <ReportsPage />
        </Suspense>
      ) : route.key === "trust" ? (
        <Suspense fallback={<div className="route-loading" role="status">正在核对实验门与后台活动…</div>}>
          <TrustPage />
        </Suspense>
      ) : (
        <Suspense fallback={<div className="route-loading" role="status">正在读取本地设置…</div>}>
          <SettingsPage />
        </Suspense>
      ),
    })),
  },
]);

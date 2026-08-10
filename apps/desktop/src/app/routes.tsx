import { lazy, Suspense } from "react";
import { createHashRouter } from "react-router-dom";

import { AppShell } from "../components/AppShell";
import { RoutePlaceholder } from "../components/RoutePlaceholder";
import { TodayPage } from "../components/TodayPage";

const MapPage = lazy(async () => {
  const module = await import("../components/MapPage");
  return { default: module.MapPage };
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
      ) : (
        <RoutePlaceholder
          eyebrow={route.key}
          title={route.label}
          description={route.description}
        />
      ),
    })),
  },
]);

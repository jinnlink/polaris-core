import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "react-router-dom";

import type { TodayAction, TodaySnapshot } from "../contracts/core";
import {
  commandKeys,
  getToday,
  hideToTray,
  normalizeCommandError,
  switchPack,
} from "../lib/commands";

type TodayViewProps = {
  snapshot: TodaySnapshot;
  switching: boolean;
  onSwitchPack: (packId: string) => void;
  onRest: () => void;
};

function ActionCard({ action, onRest }: { action: TodayAction; onRest: () => void }) {
  const content = (
    <>
      <span className="action-card__kind">{action.kind}</span>
      <strong>{action.title}</strong>
      <span>{action.detail}</span>
      {action.expected_success !== null && (
        <small>预计成功率 {Math.round(action.expected_success * 100)}%</small>
      )}
    </>
  );
  if (action.route) {
    return <Link className="action-card" to={action.route}>{content}</Link>;
  }
  return (
    <button className="action-card action-card--button" type="button" onClick={onRest}>
      {content}
    </button>
  );
}

export function TodayView({ snapshot, switching, onSwitchPack, onRest }: TodayViewProps) {
  return (
    <section className="today" aria-labelledby="today-title">
      <div className="today__heading">
        <div>
          <p className="eyebrow">today</p>
          <h1 id="today-title">今天，选一件。</h1>
        </div>
        <label className="pack-picker">
          <span>学习空间</span>
          <select
            value={snapshot.current_pack ?? ""}
            disabled={switching || snapshot.packs.length === 0}
            onChange={(event) => {
              onSwitchPack(event.target.value);
            }}
          >
            {snapshot.packs.length === 0 && <option value="">尚未安装 Pack</option>}
            {snapshot.packs.map((pack) => (
              <option key={pack.id} value={pack.id}>
                {pack.title} · {pack.theta_mode}
              </option>
            ))}
          </select>
        </label>
      </div>

      {snapshot.top_signal && (
        <aside className="top-signal" aria-label="如果只看一句">
          <span>如果只看一句</span>
          <strong>{snapshot.top_signal.claim}</strong>
          {snapshot.top_signal.suggested_action && <p>{snapshot.top_signal.suggested_action}</p>}
        </aside>
      )}

      <div className="today-actions" aria-label="今天的可选行动">
        {snapshot.actions.map((action) => (
          <ActionCard key={action.id} action={action} onRest={onRest} />
        ))}
      </div>

      <footer className="today__footer">
        <span>{snapshot.theta_mode ?? "shared"} 模式</span>
        {snapshot.notification_policy.suppress_non_error && (
          <span className="flow-badge">心流保护中 · 已静音</span>
        )}
      </footer>
    </section>
  );
}

export function TodayPage() {
  const queryClient = useQueryClient();
  const today = useQuery({
    queryKey: commandKeys.today,
    queryFn: getToday,
    staleTime: 15_000,
    retry: 1,
  });
  const packSwitch = useMutation({
    mutationFn: switchPack,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["core"] });
    },
  });

  if (today.isPending) {
    return <div className="route-loading" role="status">正在读取本地学习状态…</div>;
  }
  if (today.error) {
    const error = normalizeCommandError(today.error);
    return (
      <div className="route-error" role="alert">
        <strong>暂时读不到 Today</strong>
        <span>{error.message}</span>
        <button type="button" onClick={() => void today.refetch()}>重试</button>
      </div>
    );
  }

  return (
    <TodayView
      snapshot={today.data}
      switching={packSwitch.isPending}
      onSwitchPack={(packId) => {
        packSwitch.mutate(packId);
      }}
      onRest={() => void hideToTray()}
    />
  );
}

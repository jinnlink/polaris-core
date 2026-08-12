import { ArrowClockwiseIcon, CheckCircleIcon, DownloadSimpleIcon, ShieldCheckIcon } from "@phosphor-icons/react";
import { useEffect, useRef, useState } from "react";

import { normalizeCommandError } from "../lib/commands";
import { productionUpdater, type UpdateCheckResult, type UpdaterAdapter, type UpdateProgress } from "../lib/updater";

export function UpdatePanel({ adapter = productionUpdater }: { adapter?: UpdaterAdapter }) {
  const started = useRef(false);
  const [result, setResult] = useState<UpdateCheckResult | null>(null);
  const [progress, setProgress] = useState<UpdateProgress | null>(null);
  const [checking, setChecking] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState("");

  const runCheck = async () => {
    setChecking(true);
    setError("");
    try {
      setResult(await adapter.check());
    } catch (reason) {
      setError(normalizeCommandError(reason).message);
    } finally {
      setChecking(false);
    }
  };

  useEffect(() => {
    if (started.current) return;
    started.current = true;
    void runCheck();
  }, []);

  const install = async () => {
    setInstalling(true);
    setError("");
    try {
      await adapter.install(setProgress);
    } catch (reason) {
      setError(normalizeCommandError(reason).message);
      setInstalling(false);
    }
  };

  const percent = progress?.total && progress.total > 0
    ? Math.min(100, Math.round((progress.downloaded / progress.total) * 100))
    : null;

  return <section className="settings-panel update-panel" aria-labelledby="update-title">
    <header><ShieldCheckIcon size={23} /><div><h2 id="update-title">应用更新</h2><p>正式更新先验证签名，展示版本与变更，再由你确认安装。</p></div></header>
    {checking && <p role="status">正在检查签名更新通道…</p>}
    {result?.status === "disabled" && <div className="update-state"><ShieldCheckIcon size={20} /><p><strong>自动更新已禁用</strong><span>{result.reason}</span></p></div>}
    {result?.status === "current" && <div className="update-state"><CheckCircleIcon size={20} /><p><strong>已经是最新版本</strong><span>没有下载或改动任何本地学习数据。</span></p></div>}
    {result?.status === "available" && <div className="update-available">
      <div><p className="eyebrow">signed update</p><h3>Polaris {result.version}</h3>{result.date && <time dateTime={result.date}>{result.date}</time>}</div>
      <p className="update-notes">{result.notes || "此版本没有附加变更说明。"}</p>
      {installing && <div className="update-progress" role="status" aria-live="polite"><progress max={100} value={percent ?? undefined} /> <span>{progress?.finished ? "验证完成，正在交给 Windows 安装…" : percent === null ? "正在下载并验证…" : `已下载 ${String(percent)}%`}</span></div>}
      <button className="primary-action" type="button" disabled={installing} onClick={() => { void install(); }}><DownloadSimpleIcon />{installing ? "正在准备安装…" : `确认下载并安装 ${result.version}`}</button>
      <small>安装时 Windows 会关闭 Polaris；数据库与学习记录保留，失败时仍可从当前版本重新启动。</small>
    </div>}
    {error && <p className="inline-error" role="alert">更新未执行：{error}。当前版本仍可继续使用。</p>}
    {!checking && result?.status !== "available" && <button className="secondary-action" type="button" onClick={() => { void runCheck(); }}><ArrowClockwiseIcon />重新检查</button>}
  </section>;
}

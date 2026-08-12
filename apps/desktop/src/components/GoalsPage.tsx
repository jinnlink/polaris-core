import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArchiveIcon, ArrowRightIcon, CheckCircleIcon, PlusIcon, TrashIcon } from "@phosphor-icons/react";
import { Link } from "react-router-dom";

import type { GoalEditorInput, GoalView } from "../contracts/core";
import { archiveGoal, commandKeys, deleteGoal, getGoalsWorkspace, normalizeCommandError, refreshGoal, saveGoal } from "../lib/commands";

const emptyGoal = (): GoalEditorInput => ({ id: `goal-${crypto.randomUUID()}`, title: "", description: null, status: "active", deadline: null, pace: "steady", priority: 50, scope: { pack_ids: [], dimension_keys: [], concept_ids: [] }, dimensions: [{ id: `dimension-${crypto.randomUUID()}`, dimension_key: "mastery", display_name: "平均掌握度", metric_type: "mastery_mean", target_value: 0.8, weight: 1 }], milestones: [] });
const editGoal = (goal: GoalView): GoalEditorInput => ({
  id: goal.id,
  title: goal.title,
  description: goal.description,
  status: goal.status,
  deadline: goal.deadline,
  pace: goal.pace,
  priority: goal.priority,
  scope: goal.scope,
  dimensions: goal.dimensions.map((dimension) => ({
    id: dimension.id,
    dimension_key: dimension.dimension_key,
    display_name: dimension.display_name,
    metric_type: dimension.metric_type,
    target_value: dimension.target_value,
    weight: dimension.weight,
  })),
  milestones: goal.milestones.map((milestone) => ({
    id: milestone.id,
    title: milestone.title,
    dimension_key: milestone.dimension_key,
    threshold: milestone.threshold,
    manual: milestone.manual,
  })),
});
const csv = (value: string) => value.split(",").map((item) => item.trim()).filter(Boolean);

export function GoalsPage() {
  const queryClient = useQueryClient();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [editor, setEditor] = useState<GoalEditorInput | null>(null);
  const [deleteConfirm, setDeleteConfirm] = useState("");
  const goals = useQuery({ queryKey: commandKeys.goals(selectedId), queryFn: () => getGoalsWorkspace(selectedId) });
  const selected = useMemo(() => goals.data?.goals.find((goal) => goal.id === (selectedId ?? goals.data.selected_goal_id)) ?? null, [goals.data, selectedId]);
  const refresh = () => queryClient.invalidateQueries({ queryKey: ["core", "goals"] });
  const save = useMutation({ mutationFn: saveGoal, onSuccess: async (receipt) => { setEditor(null); setSelectedId(receipt.goal_id); await refresh(); } });
  const recalc = useMutation({ mutationFn: refreshGoal, onSuccess: refresh });
  const archive = useMutation({ mutationFn: archiveGoal, onSuccess: refresh });
  const remove = useMutation({ mutationFn: deleteGoal, onSuccess: async () => { setDeleteConfirm(""); setSelectedId(null); await refresh(); } });
  if (goals.isPending) return <div className="route-loading" role="status">正在计算目标路径…</div>;
  if (goals.error) return <GoalError error={goals.error} onRetry={() => void goals.refetch()} />;
  const error = save.error ?? recalc.error ?? archive.error ?? remove.error;
  return <section className="goals-workspace" aria-labelledby="goals-title">
    <header className="goals-hero"><div><p className="eyebrow">goals · bounded ambition</p><h1 id="goals-title">把愿望变成可验证的路径。</h1><p>目标只筛选候选范围；评分、先修门和证据标准不会为截止日期让路。</p></div><button className="primary-action" type="button" onClick={() => { setEditor(emptyGoal()); }}><PlusIcon size={18} /> 新建目标</button></header>
    {error && <p className="inline-error" role="alert">{normalizeCommandError(error).message}。表单仍保留，可修正后重试。</p>}
    <div className="goals-layout">
      <nav className="goal-index" aria-label="目标列表">{goals.data.goals.length === 0 ? <div className="goal-index__empty"><strong>还没有目标</strong><p>创建一个具体目标，系统会给出范围内 2–3 个行动。</p></div> : goals.data.goals.map((goal) => <button type="button" key={goal.id} className={selected?.id === goal.id ? "is-selected" : ""} onClick={() => { setSelectedId(goal.id); setEditor(null); }}><span>{goal.status}</span><strong>{goal.title}</strong><i><b style={{ width: `${String(goal.overall_progress * 100)}%` }} /></i><small>{Math.round(goal.overall_progress * 100)}%</small></button>)}</nav>
      <div className="goal-stage">{editor ? <GoalEditor input={editor} pending={save.isPending} onChange={setEditor} onCancel={() => { setEditor(null); }} onSave={() => { save.mutate(editor); }} /> : selected ? <GoalDetail goal={selected} actions={goals.data.actions} deleteConfirm={deleteConfirm} onDeleteConfirm={setDeleteConfirm} onEdit={() => { setEditor(editGoal(selected)); }} onRefresh={() => { recalc.mutate(selected.id); }} onArchive={() => { archive.mutate(selected.id); }} onDelete={() => { remove.mutate(selected.id); }} /> : <div className="goal-stage__empty"><strong>选择一个目标，或创建新的目标。</strong><p>目标不会覆盖当前 Pack，也不会绕过 prerequisite。</p></div>}</div>
    </div>
  </section>;
}

function GoalDetail({ goal, actions, deleteConfirm, onDeleteConfirm, onEdit, onRefresh, onArchive, onDelete }: { goal: GoalView; actions: Array<{ id: string; title: string; detail: string; route: string | null; expected_success: number | null }>; deleteConfirm: string; onDeleteConfirm: (value: string) => void; onEdit: () => void; onRefresh: () => void; onArchive: () => void; onDelete: () => void }) {
  const schedulable = goal.status === "active";
  return <><header className="goal-stage__header"><div><span className="eyebrow">{goal.status} · priority {goal.priority}</span><h2>{goal.title}</h2><p>{goal.description ?? "没有附加说明。"}</p></div><div><button type="button" onClick={onEdit}>编辑</button><button type="button" onClick={onRefresh} disabled={!schedulable}>刷新进度</button></div></header>
    <div className="goal-progress-hero"><strong>{Math.round(goal.overall_progress * 100)}%</strong><div><span style={{ width: `${String(goal.overall_progress * 100)}%` }} /></div><p>{goal.deadline ? `截止 ${goal.deadline}` : "无硬性截止日期"} · {goal.scope.pack_ids.length ? goal.scope.pack_ids.join(" / ") : "当前 Pack"}</p></div>
    <section className="goal-section"><h3>进度维度</h3>{goal.dimensions.map((dimension) => <article className="goal-dimension-row" key={dimension.id}><div><strong>{dimension.display_name}</strong><span>{dimension.metric_type} · 权重 {dimension.weight}</span></div><p>{dimension.current_value.toFixed(2)} / {dimension.target_value.toFixed(2)}</p><i><b style={{ width: `${String(dimension.progress * 100)}%` }} /></i></article>)}</section>
    <section className="goal-section"><h3>里程碑</h3>{goal.milestones.length === 0 ? <p>尚未设置里程碑。</p> : goal.milestones.map((milestone) => <article className="goal-milestone" key={milestone.id}><CheckCircleIcon size={19} weight={milestone.status === "reached" ? "fill" : "regular"} /><div><strong>{milestone.title}</strong><p>{milestone.manual ? "手动确认" : `${milestone.dimension_key ?? "维度"} ≥ ${String(milestone.threshold ?? "")}`} · {milestone.status}</p></div></article>)}</section>
    <section className="goal-section"><h3>接下来最值得做</h3><div className="goal-action-list">{actions.length === 0 ? <p>{schedulable ? "当前范围内没有合法行动。先检查 Pack、概念范围或先修条件。" : "历史目标只读展示，不参与当前调度。"}</p> : actions.map((action) => action.route ? <Link key={action.id} to={action.route}><span>{action.title}</span><small>{action.detail}</small><ArrowRightIcon size={18} /></Link> : null)}</div></section>
    <details className="goal-danger"><summary>归档或删除</summary><p>归档会保留历史；删除会移除目标、维度和里程碑，但不会删除学习证据。</p><div><button type="button" onClick={onArchive}><ArchiveIcon size={16} /> 归档</button><label>输入目标 ID 以删除<input value={deleteConfirm} onChange={(event) => { onDeleteConfirm(event.target.value); }} /></label><button type="button" disabled={deleteConfirm !== goal.id} onClick={onDelete}><TrashIcon size={16} /> 删除目标</button></div></details>
  </>;
}

function GoalEditor({ input, pending, onChange, onCancel, onSave }: { input: GoalEditorInput; pending: boolean; onChange: (value: GoalEditorInput) => void; onCancel: () => void; onSave: () => void }) {
  return <form className="goal-editor" onSubmit={(event) => { event.preventDefault(); onSave(); }}><header><p className="eyebrow">goal editor</p><h2>{input.title || "新目标"}</h2></header><div className="goal-editor__grid"><label>标题<input value={input.title} onChange={(event) => { onChange({ ...input, title: event.target.value }); }} /></label><label>状态<select value={input.status} onChange={(event) => { onChange({ ...input, status: event.target.value }); }}><option value="active">进行中</option><option value="paused">暂停</option><option value="completed">已完成</option><option value="abandoned">已放弃</option></select></label><label className="span-2">说明<textarea rows={3} value={input.description ?? ""} onChange={(event) => { onChange({ ...input, description: event.target.value || null }); }} /></label><label>截止日期<input type="date" value={input.deadline ?? ""} onChange={(event) => { onChange({ ...input, deadline: event.target.value || null }); }} /></label><label>优先级<input type="number" min="0" max="100" value={input.priority} onChange={(event) => { onChange({ ...input, priority: Number(event.target.value) }); }} /></label><label>Pack 范围<input value={input.scope.pack_ids.join(", ")} placeholder="algorithms, rust" onChange={(event) => { onChange({ ...input, scope: { ...input.scope, pack_ids: csv(event.target.value) } }); }} /></label><label>概念范围<input value={input.scope.concept_ids.join(", ")} placeholder="留空表示整个 Pack" onChange={(event) => { onChange({ ...input, scope: { ...input.scope, concept_ids: csv(event.target.value) } }); }} /></label></div>
    <section className="goal-editor__children"><header><h3>进度维度</h3><button type="button" onClick={() => { const id = crypto.randomUUID(); onChange({ ...input, dimensions: [...input.dimensions, { id: `dimension-${id}`, dimension_key: `metric-${id}`, display_name: "已评分回答", metric_type: "graded_attempts", target_value: 10, weight: 1 }] }); }}>添加维度</button></header>{input.dimensions.map((dimension, index) => <div className="goal-editor-row" key={dimension.id}><input aria-label={`维度 ${String(index + 1)} 名称`} value={dimension.display_name} onChange={(event) => { onChange({ ...input, dimensions: input.dimensions.map((item, itemIndex) => itemIndex === index ? { ...item, display_name: event.target.value } : item) }); }} /><select aria-label={`维度 ${String(index + 1)} 指标`} value={dimension.metric_type} onChange={(event) => { onChange({ ...input, dimensions: input.dimensions.map((item, itemIndex) => itemIndex === index ? { ...item, metric_type: event.target.value } : item) }); }}><option value="mastery_mean">平均掌握度</option><option value="mastered_count">已掌握概念数</option><option value="graded_attempts">已评分回答数</option><option value="evidence_count">证据数</option></select><input aria-label={`维度 ${String(index + 1)} 目标值`} type="number" step="0.01" value={dimension.target_value} onChange={(event) => { onChange({ ...input, dimensions: input.dimensions.map((item, itemIndex) => itemIndex === index ? { ...item, target_value: Number(event.target.value) } : item) }); }} /><button type="button" aria-label={`删除维度 ${String(index + 1)}`} onClick={() => { onChange({ ...input, dimensions: input.dimensions.filter((_, itemIndex) => itemIndex !== index) }); }}><TrashIcon /></button></div>)}</section>
    <section className="goal-editor__children"><header><h3>里程碑</h3><button type="button" onClick={() => { onChange({ ...input, milestones: [...input.milestones, { id: `milestone-${crypto.randomUUID()}`, title: "新里程碑", dimension_key: input.dimensions[0]?.dimension_key ?? null, threshold: input.dimensions[0]?.target_value ?? null, manual: input.dimensions.length === 0 }] }); }}>添加里程碑</button></header>{input.milestones.map((milestone, index) => <div className="goal-editor-row goal-editor-row--milestone" key={milestone.id}><input aria-label={`里程碑 ${String(index + 1)} 标题`} value={milestone.title} onChange={(event) => { onChange({ ...input, milestones: input.milestones.map((item, itemIndex) => itemIndex === index ? { ...item, title: event.target.value } : item) }); }} /><select aria-label={`里程碑 ${String(index + 1)} 类型`} value={milestone.manual ? "manual" : "threshold"} onChange={(event) => { onChange({ ...input, milestones: input.milestones.map((item, itemIndex) => itemIndex === index ? { ...item, manual: event.target.value === "manual", dimension_key: event.target.value === "manual" ? null : (input.dimensions[0]?.dimension_key ?? null) } : item) }); }}><option value="threshold">达到维度阈值</option><option value="manual">手动确认</option></select><select aria-label={`里程碑 ${String(index + 1)} 维度`} disabled={milestone.manual} value={milestone.dimension_key ?? ""} onChange={(event) => { onChange({ ...input, milestones: input.milestones.map((item, itemIndex) => itemIndex === index ? { ...item, dimension_key: event.target.value || null } : item) }); }}><option value="">选择维度</option>{input.dimensions.map((dimension) => <option key={dimension.id} value={dimension.dimension_key}>{dimension.display_name}</option>)}</select><input aria-label={`里程碑 ${String(index + 1)} 阈值`} type="number" step="0.01" disabled={milestone.manual} value={milestone.threshold ?? ""} onChange={(event) => { onChange({ ...input, milestones: input.milestones.map((item, itemIndex) => itemIndex === index ? { ...item, threshold: Number(event.target.value) } : item) }); }} /><button type="button" aria-label={`删除里程碑 ${String(index + 1)}`} onClick={() => { onChange({ ...input, milestones: input.milestones.filter((_, itemIndex) => itemIndex !== index) }); }}><TrashIcon /></button></div>)}</section>
    <div className="workbench-actions"><button className="primary-action" type="submit" disabled={pending || !input.title.trim() || input.dimensions.length === 0}>保存目标</button><button className="quiet-action" type="button" onClick={onCancel}>取消</button></div></form>;
}

function GoalError({ error, onRetry }: { error: unknown; onRetry: () => void }) { return <div className="route-error" role="alert"><strong>暂时读不到目标</strong><span>{normalizeCommandError(error).message}</span><button type="button" onClick={onRetry}>重试</button></div>; }

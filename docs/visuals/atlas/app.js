const savedLanguage = (() => {
  try {
    return localStorage.getItem("polaris-atlas-language");
  } catch {
    return null;
  }
})();

const state = {
  data: null,
  selected: { type: "module", id: "engine" },
  query: "",
  view: "overview",
  lang: savedLanguage === "en" ? "en" : "zh",
  reducedMotion: window.matchMedia("(prefers-reduced-motion: reduce)").matches,
};

const refs = {
  brandTitle: document.querySelector("#brand-title"),
  brandSubtitle: document.querySelector("#brand-subtitle"),
  searchLabel: document.querySelector("#search-label"),
  search: document.querySelector("#atlas-search"),
  results: document.querySelector("#search-results"),
  tree: document.querySelector("#module-tree"),
  moduleHeading: document.querySelector("#module-heading"),
  moduleCount: document.querySelector("#module-count"),
  view: document.querySelector("#atlas-view"),
  inspectorHeading: document.querySelector("#inspector-heading"),
  inspector: document.querySelector("#inspector"),
  inspectorState: document.querySelector("#inspector-state"),
  tabs: Array.from(document.querySelectorAll("[data-view]")),
  langButtons: Array.from(document.querySelectorAll("[data-lang]")),
  canvas: document.querySelector(".ambient-field"),
};

const GROUP_ORDER = [
  "Entrypoints",
  "Foundation",
  "Core Loop",
  "Data Ledger",
  "Knowledge Space",
  "Evidence",
  "Cognitive Dynamics",
  "Teaching",
  "Pack",
];

const GRAPH_GROUPS = {
  Entrypoints: { x: 64, y: 64 },
  Foundation: { x: 64, y: 288 },
  Teaching: { x: 64, y: 548 },
  "Core Loop": { x: 358, y: 178 },
  "Data Ledger": { x: 358, y: 520 },
  "Knowledge Space": { x: 650, y: 64 },
  Evidence: { x: 650, y: 336 },
  "Cognitive Dynamics": { x: 928, y: 64 },
  Pack: { x: 928, y: 584 },
};

const NODE = {
  width: 224,
  height: 56,
  gap: 68,
};

const COPY = {
  zh: {
    brandTitle: "Polaris Core 图谱",
    brandSubtitle: "Porcelain Intelligence",
    searchLabel: "检索 Search",
    searchPlaceholder: "搜索模块、票据、数据表、护栏...",
    moduleHeading: "系统索引",
    inspectorHeading: "详情面板",
    moduleCount: (count) => `${count} 个模块`,
    noSelection: "未选择",
    selectPrompt: "选择一个模块、数据表、票据或护栏，右侧会展开它的职责、依赖和证据来源。",
    results: "搜索结果",
    noResults: "没有结果",
    loadingErrorKicker: "载入失败",
    loadingErrorTitle: "Atlas 数据不可用",
    graphAria: "Polaris 架构依赖图",
    tabs: {
      overview: ["总览", "Overview"],
      graph: ["图谱", "Graph"],
      timeline: ["时间线", "Timeline"],
      data: ["数据模型", "Data"],
      guardrails: ["护栏", "Guardrails"],
    },
    types: {
      module: "模块",
      ticket: "票据",
      table: "数据表",
      guardrail: "护栏",
    },
    inspectorBlocks: {
      files: "源文件",
      dependencies: "依赖模块",
      tables: "关联数据表",
      tickets: "关联票据",
      sourceRefs: "依据来源",
    },
    hero: {
      kicker: "本地证据驱动的学习引擎",
      kickerAlt: "Evidence-bound learning system",
      title: "验证真懂，定位模糊，针对性补缺。",
      titleAlt: "Verify understanding. Locate uncertainty. Repair precisely.",
      body:
        "Polaris Core 把学习行为、证据、掌握度和教学动作收束到一个可回放的本地闭环里：快路径确定、慢路径审计、外部 AI 只能建议不能写入掌握度。",
      bodyAlt:
        "A local, replayable learning loop where deterministic Tier 0 state owns mastery and slower AI paths remain audited helpers.",
      statusLabel: "当前工程态",
      statusAlt: "Project state",
      metrics: {
        completed: "已完成票据",
        queued: "待推进票据",
        tables: "SQLite 数据表",
      },
      actions: [
        {
          view: "graph",
          title: "看架构图谱",
          alt: "Architecture graph",
          body: "模块依赖、源文件、Tier 边界和证据路径在同一张图里联动。",
        },
        {
          view: "data",
          title: "看本地账本",
          alt: "Local ledger",
          body: "事实源、派生状态、队列与审计记录分层呈现。",
        },
        {
          view: "timeline",
          title: "看阶段进度",
          alt: "Phase line",
          body: "已完成、排队中、未来规划清楚分离，避免把未建能力画成已完成。",
        },
      ],
      stage: {
        core: "确定性核心",
        coreAlt: "Tier 0 local loop",
        ledger: "证据账本",
        diagnosis: "图谱诊断",
        audit: "慢路径审计",
        teaching: "教学动作",
      },
    },
    sections: {
      graphKicker: "可点击架构",
      graphTitle: "模块依赖图谱",
      graphBody: "点选节点后，相关依赖会被点亮，右侧会展开源文件、数据表、票据和护栏。",
      timelineKicker: "票据阶段线",
      timelineTitle: "完成、进行、待推进分层",
      timelineBody: "时间线直接读取 QUEUE；已完成、唯一进行中票和后续正式票不会被静态快照混淆。",
      dataKicker: "SQLite 本地账本",
      dataTitle: "事实源与派生状态",
      dataBody: "数据模型把原始证据、行为事件、作答、掌握度、队列和审计记录分开，保证可回放和可追责。",
      guardrailKicker: "系统护栏",
      guardrailTitle: "让闭环保持诚实的规则",
      guardrailBody: "这些约束保护本地持久化、证据所有权、引用纪律和单票制边界。",
    },
    tableTags: {
      indexed: "已索引",
      table: "数据表",
    },
  },
  en: {
    brandTitle: "Polaris Core Atlas",
    brandSubtitle: "瓷白智能图谱",
    searchLabel: "Search 检索",
    searchPlaceholder: "Search modules, tickets, tables, guardrails...",
    moduleHeading: "System Index",
    inspectorHeading: "Inspector",
    moduleCount: (count) => `${count} modules`,
    noSelection: "No selection",
    selectPrompt: "Select a module, table, ticket, or guardrail to inspect responsibilities, dependencies, and evidence sources.",
    results: "Results",
    noResults: "No results",
    loadingErrorKicker: "Load failed",
    loadingErrorTitle: "Atlas data is unavailable",
    graphAria: "Polaris architecture dependency graph",
    tabs: {
      overview: ["Overview", "总览"],
      graph: ["Graph", "图谱"],
      timeline: ["Timeline", "时间线"],
      data: ["Data", "数据模型"],
      guardrails: ["Guardrails", "护栏"],
    },
    types: {
      module: "module",
      ticket: "ticket",
      table: "table",
      guardrail: "guardrail",
    },
    inspectorBlocks: {
      files: "Files",
      dependencies: "Depends on",
      tables: "Tables",
      tickets: "Tickets",
      sourceRefs: "Source refs",
    },
    hero: {
      kicker: "Evidence-bound learning system",
      kickerAlt: "本地证据驱动的学习引擎",
      title: "Verify understanding. Locate uncertainty. Repair precisely.",
      titleAlt: "验证真懂，定位模糊，针对性补缺。",
      body:
        "Polaris Core binds learning behavior, evidence, mastery, and teaching moves into a replayable local loop: deterministic fast path, audited slow path, and external AI that can advise but never write mastery.",
      bodyAlt: "本地可回放学习闭环；Tier 0 掌握度归引擎所有，慢路径 AI 只做审计助手。",
      statusLabel: "Project state",
      statusAlt: "当前工程态",
      metrics: {
        completed: "completed tickets",
        queued: "queued tickets",
        tables: "SQLite tables",
      },
      actions: [
        {
          view: "graph",
          title: "Architecture graph",
          alt: "看架构图谱",
          body: "Module dependencies, source files, Tier boundaries, and evidence paths stay linked in one graph.",
        },
        {
          view: "data",
          title: "Local ledger",
          alt: "看本地账本",
          body: "Facts, derived state, queues, and audits are separated for replayable ownership.",
        },
        {
          view: "timeline",
          title: "Phase line",
          alt: "看阶段进度",
          body: "Completed, queued, and planned work remain visually distinct.",
        },
      ],
      stage: {
        core: "Deterministic core",
        coreAlt: "Tier 0 本地闭环",
        ledger: "Evidence ledger",
        diagnosis: "Graph diagnosis",
        audit: "Slow-path audit",
        teaching: "Teaching moves",
      },
    },
    sections: {
      graphKicker: "Clickable architecture",
      graphTitle: "Module graph",
      graphBody: "Select a node to light its dependency path and inspect source files, tables, tickets, and guardrails.",
      timelineKicker: "Ticket phase line",
      timelineTitle: "Completed, active, remaining",
      timelineBody: "The timeline reads QUEUE directly so completed work, the single active ticket, and remaining formal tickets stay distinct.",
      dataKicker: "SQLite ledger",
      dataTitle: "Facts and derived state",
      dataBody: "Raw evidence, behavior, attempts, mastery, queues, and audit records are separated for replay and accountability.",
      guardrailKicker: "System guardrails",
      guardrailTitle: "Rules that keep the loop honest",
      guardrailBody: "These constraints preserve local persistence, evidence ownership, citation discipline, and ticket scope.",
    },
    tableTags: {
      indexed: "indexed",
      table: "table",
    },
  },
};

const GROUP_LABELS = {
  Entrypoints: { zh: "入口层", en: "Entrypoints" },
  Foundation: { zh: "基础约束", en: "Foundation" },
  "Core Loop": { zh: "学习闭环", en: "Core Loop" },
  "Data Ledger": { zh: "本地账本", en: "Data Ledger" },
  "Knowledge Space": { zh: "知识结构", en: "Knowledge Space" },
  Evidence: { zh: "证据与报告", en: "Evidence" },
  "Cognitive Dynamics": { zh: "心智动力学", en: "Cognitive Dynamics" },
  Teaching: { zh: "教学策略", en: "Teaching" },
  Pack: { zh: "领域包", en: "Pack" },
  "Core Module": { zh: "核心模块", en: "Core Module" },
};

const MODULE_ZH = {
  citation: {
    title: "引用校验",
    summary: "校验 strict-citation 边界，确保报告里的每一句判断都能回到原始证据。",
  },
  config: {
    title: "参数登记",
    summary: "集中登记带类型、边界和调优路径的参数，让行为旋钮保持显式、可审计。",
  },
  consolidation: {
    title: "夜间巩固",
    summary: "在慢路径运行巩固、残差聚合和反事实回放，提炼不阻塞前台的学习信号。",
  },
  db: {
    title: "SQLite 本地账本",
    summary: "负责 schema、迁移、热路径索引和本地持久化，是可回放系统的事实底座。",
  },
  diagnosis: {
    title: "图谱感知诊断",
    summary: "把知识图谱和作答证据转成概念级诊断，定位模糊、误解和修补入口。",
  },
  engine: {
    title: "引擎调度核心",
    summary: "串联 pack 初始化、选题、证据提交、评分、报告和维护任务，维持学习闭环的主节拍。",
  },
  error: {
    title: "错误边界",
    summary: "集中定义可恢复的 Polaris Core 错误类型，让降级路径清晰可测。",
  },
  fsrs: {
    title: "FSRS 记忆状态",
    summary: "实现 FSRS 风格的复习状态和可提取度估计，支持本地调度。",
  },
  geometry: {
    title: "几何候选层",
    summary: "用结构相似度、潜因子和残差信号构造候选集，服务针对性补缺。",
  },
  grader: {
    title: "评分队列",
    summary: "生成引擎所有的 provisional score，并承接异步评分队列的最终结果。",
  },
  graph: {
    title: "类型化超图",
    summary: "表达 prerequisite、confusion、component 等 typed edges，形成可诊断知识拓扑。",
  },
  gu: {
    title: "G_u 误解语法",
    summary: "从重复证据模式中归纳并审计 G_u 规则，让误解归因有生命周期。",
  },
  mastery: {
    title: "掌握度折叠",
    summary: "更新 p_known、校准差、知识相位和复习状态，是“验证真懂”的确定性核心。",
  },
  mental_fit: {
    title: "心智动力拟合",
    summary: "拟合 hazard、HMM gate 和 EM 心智动力模型；未过门时只审计、不调度。",
  },
  mental_state: {
    title: "学习者状态",
    summary: "从行为事件与作答残差中估计心流、挫败、焦虑等 HMM 风格状态。",
  },
  mirt: {
    title: "MIRT 潜因子",
    summary: "维护潜在能力向量，支持诊断、夜间巩固和掌握度融合。",
  },
  moves: {
    title: "教学动作库",
    summary: "定义 recall、explain、apply、analyze、evaluate、create、transfer 等 move taxonomy。",
  },
  pack: {
    title: "领域包装载",
    summary: "加载并校验 domain pack 的概念、先修边、moves 和误解声明。",
  },
  phase: {
    title: "知识相位判定",
    summary: "判断幻影、脆弱、惰性、僵硬、活跃、自动化等相位，指导下一步取证。",
  },
  report: {
    title: "镜像报告",
    summary: "生成带证据 id、置信度和假设边界的 mirror report，拒绝无证据断言。",
  },
  scheduler: {
    title: "交错调度器",
    summary: "在记忆、相位、几何候选和掌握度约束下选择下一题与交错 batch。",
  },
  status: {
    title: "状态快照",
    summary: "为 CLI 与 MCP 消费方构造只读状态视图，避免外部路径写入核心状态。",
  },
  teaching: {
    title: "教学策略输出",
    summary: "生成 Tier 2 教学指令，但不允许外部 AI 直接改写掌握度。",
  },
  tuning: {
    title: "参数自调优",
    summary: "通过审计记录和反事实回放评估参数候选，写入接受或拒绝结果。",
  },
  cli: {
    title: "CLI 入口",
    summary: "暴露 init、next、submit、hint、status、report、mcp、pack validate 等命令。",
  },
  mcp: {
    title: "MCP 接口",
    summary: "向 agent 暴露 next task、interleaved batch、submit evidence、teaching instruction 和只读资源。",
  },
};

const TABLE_ZH = {
  meta: "配置、迁移和参数登记元数据。",
  op_log: "追加式操作日志，用于回放和审计边界。",
  evidence_items: "学习者或系统产生的本地原始证据。",
  attempts: "作答、置信度、耗时、评分和证据链接。",
  concepts: "领域包概念与初始掌握度先验。",
  edges: "先修、混淆、迁移等 typed relationships。",
  mastery_states: "派生掌握度、相位、FSRS、到期时间和校准信号。",
  sessions: "绑定行为事件的学习会话。",
  behavior_events: "next、hint、abandon 等细粒度学习行为。",
  grade_queue: "Tier 1 异步评分队列与结果。",
  theta: "当前潜在能力估计。",
  theta_history: "用于回放和趋势分析的潜因子历史快照。",
  residual_stats: "模型拟合和漂移审计的残差统计。",
  consolidation_runs: "夜间巩固运行记录。",
  moves_effects: "教学动作效果估计。",
  mrt_log: "微随机试验审计记录。",
  param_tuning_runs: "参数调优提案及接受/拒绝结果。",
  gu_rules: "带生命周期审计的 G_u 归纳规则。",
  mirror_reports: "证据约束下的镜像报告和过滤后断言。",
  hazard_models: "心智动力学 hazard 模型拟合记录。",
  state_gate_evals: "HMM/state gate 参与决策评估。",
};

const TICKET_ZH = {
  P01: ["最小闭环", "验证真懂"],
  P02A: ["类型化超图", "定位模糊知识"],
  P02B: ["图谱感知诊断", "定位并修补模糊知识"],
  P02C: ["MCP Server", "向 agent 暴露学习闭环"],
  P03A: ["MIRT 潜因子层", "潜在能力诊断"],
  P03B: ["夜间巩固", "慢路径证据回放"],
  P03C: ["几何候选层", "针对性补缺"],
  P03D: ["心智状态 HMM", "行为感知诊断"],
  P03E: ["知识相位图", "相位感知验证"],
  P03F: ["Moves Bloom 扩展", "教学动作覆盖"],
  P03G: ["交错调度", "混合练习选择"],
  P03H: ["G_u 自动归纳", "误解规则审计"],
  P03I: ["镜像报告", "证据约束反思"],
  P03J: ["参数自调优", "审计驱动调参"],
  P03K: ["心智动力拟合", "心智动力层激活"],
  P03L: ["索引审计", "热路径性能护栏"],
  P04A: ["桌面状态镜子契约", "Tauri/HTTP 共用 Tier 0 DTO；不是桌面应用"],
  P04B: ["HTTP API", "本地外部 API"],
  P04C: ["MRT 引擎", "教学策略试验"],
  P04D: ["Goals 迁移", "目标维度迁移"],
  P04E: ["学习模拟端到端测试", "全闭环验证"],
  P05A0: ["课程接入协议", "课程作者 API"],
  P05A1: ["算法 Domain Pack", "第二领域验证"],
};

const GUARDRAIL_ZH = {
  "sync-no-llm": {
    title: "同步路径零 LLM",
    summary: "Tier 0 学习动作必须保持本地、确定性和快速。",
  },
  "engine-owned-mastery": {
    title: "掌握度归引擎所有",
    summary: "LLM 可以教学或解释，但掌握状态只能由引擎证据和评分写入。",
  },
  "strict-citation": {
    title: "镜像报告必须严格引用",
    summary: "断言必须带 evidence id 和置信度；说不出证据就不得进入报告。",
  },
  "local-persistent": {
    title: "本地持久化账本",
    summary: "事实存于本地 SQLite，并可通过证据与操作记录回放。",
  },
  "single-ticket": {
    title: "单票制纪律",
    summary: "同一时刻只有一张实现票推进；QUEUE 是事实来源。",
  },
};

const STATUS_LABELS = {
  completed: { zh: "已完成", en: "completed" },
  in_progress: { zh: "进行中", en: "in progress" },
  queued: { zh: "排队中", en: "queued" },
  planned: { zh: "规划中", en: "planned" },
  implemented: { zh: "已实现", en: "implemented" },
  support: { zh: "支撑", en: "support" },
};

const AMBIENT_TERMS = [
  "验证真懂",
  "证据链",
  "掌握度",
  "知识相位",
  "FSRS",
  "MCP",
  "Tier 0",
  "回放",
  "Strict Citation",
  "Local Ledger",
];

function t() {
  return COPY[state.lang];
}

function alternateLang() {
  return state.lang === "zh" ? "en" : "zh";
}

function pick(primary, fallback) {
  return primary || fallback || "";
}

function pair(zh, en) {
  return state.lang === "zh"
    ? { primary: pick(zh, en), secondary: pick(en, zh) }
    : { primary: pick(en, zh), secondary: pick(zh, en) };
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

function normalize(value) {
  return String(value || "").toLowerCase();
}

function truncate(value, maxLength) {
  const text = String(value || "");
  return text.length > maxLength ? `${text.slice(0, maxLength - 1)}…` : text;
}

function toList(value) {
  if (!value) return [];
  return Array.isArray(value) ? value : [value];
}

function getModules() {
  return state.data?.modules || [];
}

function statusLabel(status) {
  const label = STATUS_LABELS[status] || { zh: status, en: status };
  return state.lang === "zh" ? label.zh : label.en;
}

function groupPair(group) {
  const label = GROUP_LABELS[group] || { zh: group, en: group };
  return pair(label.zh, label.en);
}

function localizeModule(module) {
  const zh = MODULE_ZH[module.id] || {};
  const group = groupPair(module.group || "Core Module");
  return {
    ...module,
    displayTitle: state.lang === "zh" ? pick(zh.title, module.title) : module.title,
    displayTitleAlt: state.lang === "zh" ? module.title : pick(zh.title, module.id),
    displaySummary: state.lang === "zh" ? pick(zh.summary, module.summary) : module.summary,
    displaySummaryAlt: state.lang === "zh" ? module.summary : pick(zh.summary, ""),
    displayGroup: group.primary,
    displayGroupAlt: group.secondary,
  };
}

function localizeTicket(ticket) {
  const zh = TICKET_ZH[ticket.id] || [];
  const displayLoop = state.lang === "zh" ? pick(zh[1], ticket.service_loop) : ticket.service_loop;
  const displayLoopAlt = state.lang === "zh" ? ticket.service_loop : pick(zh[1], "");
  return {
    ...ticket,
    displayTitle: state.lang === "zh" ? pick(zh[0], ticket.title) : ticket.title,
    displayTitleAlt: state.lang === "zh" ? ticket.title : pick(zh[0], ""),
    displayLoop,
    displayLoopAlt,
    displaySummary: displayLoop,
    displaySummaryAlt: displayLoopAlt,
  };
}

function localizeTable(table) {
  const zh = TABLE_ZH[table.id];
  return {
    ...table,
    displayTitle: table.id,
    displayTitleAlt: state.lang === "zh" ? "SQLite table" : "SQLite 数据表",
    displaySummary: state.lang === "zh" ? pick(zh, table.summary) : table.summary,
    displaySummaryAlt: state.lang === "zh" ? table.summary : pick(zh, ""),
  };
}

function localizeGuardrail(guardrail) {
  const zh = GUARDRAIL_ZH[guardrail.id] || {};
  return {
    ...guardrail,
    displayTitle: state.lang === "zh" ? pick(zh.title, guardrail.title) : guardrail.title,
    displayTitleAlt: state.lang === "zh" ? guardrail.title : pick(zh.title, ""),
    displaySummary: state.lang === "zh" ? pick(zh.summary, guardrail.summary) : guardrail.summary,
    displaySummaryAlt: state.lang === "zh" ? guardrail.summary : pick(zh.summary, ""),
  };
}

function getSelectedEntity() {
  if (!state.data) return null;
  const { type, id } = state.selected;
  if (type === "module") {
    const module = state.data.modules.find((item) => item.id === id);
    return module ? localizeModule(module) : null;
  }
  if (type === "ticket") {
    const ticket = state.data.tickets.find((item) => item.id === id);
    return ticket ? localizeTicket(ticket) : null;
  }
  if (type === "table") {
    const table = state.data.database.find((item) => item.id === id);
    return table ? localizeTable(table) : null;
  }
  if (type === "guardrail") {
    const guardrail = state.data.guardrails.find((item) => item.id === id);
    return guardrail ? localizeGuardrail(guardrail) : null;
  }
  return null;
}

async function loadAtlasData() {
  const response = await fetch("./data/atlas-data.json");
  if (!response.ok) {
    throw new Error(`Failed to load atlas-data.json: ${response.status}`);
  }
  state.data = await response.json();
}

function startViewTransition(update) {
  if (document.startViewTransition && !state.reducedMotion) {
    document.startViewTransition(update);
  } else {
    update();
  }
}

function selectEntity(type, id, view = state.view) {
  state.selected = { type, id };
  if (view !== state.view) state.view = view;
  startViewTransition(() => {
    render();
  });
}

function setView(view) {
  state.view = view;
  window.location.hash = view;
  startViewTransition(() => {
    render();
  });
}

function setLanguage(lang) {
  state.lang = lang === "en" ? "en" : "zh";
  document.documentElement.lang = state.lang === "zh" ? "zh-CN" : "en";
  try {
    localStorage.setItem("polaris-atlas-language", state.lang);
  } catch {
    // Ignore storage restrictions in static preview contexts.
  }
  startViewTransition(() => {
    render();
  });
}

function groupModules() {
  const groups = new Map();
  for (const module of getModules()) {
    const group = module.group || "Core Module";
    if (!groups.has(group)) groups.set(group, []);
    groups.get(group).push(module);
  }
  return [...groups.entries()].sort((a, b) => {
    const aIndex = GROUP_ORDER.indexOf(a[0]);
    const bIndex = GROUP_ORDER.indexOf(b[0]);
    return (aIndex === -1 ? 99 : aIndex) - (bIndex === -1 ? 99 : bIndex);
  });
}

function renderChrome() {
  const copy = t();
  document.documentElement.lang = state.lang === "zh" ? "zh-CN" : "en";
  refs.brandTitle.textContent = copy.brandTitle;
  refs.brandSubtitle.textContent = copy.brandSubtitle;
  refs.searchLabel.textContent = copy.searchLabel;
  refs.search.placeholder = copy.searchPlaceholder;
  refs.moduleHeading.textContent = copy.moduleHeading;
  refs.inspectorHeading.textContent = copy.inspectorHeading;
  refs.langButtons.forEach((button) => {
    const active = button.dataset.lang === state.lang;
    button.setAttribute("aria-pressed", String(active));
  });
}

function renderTabs() {
  refs.tabs.forEach((button) => {
    const active = button.dataset.view === state.view;
    const [primary, secondary] = t().tabs[button.dataset.view];
    button.setAttribute("aria-pressed", String(active));
    button.innerHTML = `<span>${escapeHtml(primary)}</span><small>${escapeHtml(secondary)}</small>`;
  });
}

function renderModuleTree() {
  refs.moduleCount.textContent = t().moduleCount(getModules().length);
  refs.tree.innerHTML = groupModules()
    .map(([group, modules]) => {
      const groupLabel = groupPair(group);
      return `
        <section class="module-group">
          <h2><span>${escapeHtml(groupLabel.primary)}</span><small>${escapeHtml(groupLabel.secondary)}</small></h2>
          <div class="module-list">
            ${modules
              .map((module) => {
                const item = localizeModule(module);
                const selected =
                  state.selected.type === "module" && state.selected.id === item.id;
                return `
                  <button class="module-button${selected ? " is-selected" : ""}" type="button" data-module="${escapeHtml(item.id)}">
                    <span>${escapeHtml(item.displayTitle)}</span>
                    <small>${escapeHtml(item.displayTitleAlt || item.id)} · ${escapeHtml(item.tier || "support")}</small>
                  </button>
                `;
              })
              .join("")}
          </div>
        </section>
      `;
    })
    .join("");

  refs.tree.querySelectorAll("[data-module]").forEach((button) => {
    button.addEventListener("click", () => selectEntity("module", button.dataset.module, "graph"));
  });
}

function searchItems(query) {
  const q = normalize(query);
  if (!q || !state.data) return [];
  const results = [];
  for (const module of state.data.modules) {
    const zh = MODULE_ZH[module.id] || {};
    const group = groupPair(module.group || "Core Module");
    const haystack = normalize([
      module.title,
      zh.title,
      module.summary,
      zh.summary,
      module.group,
      group.primary,
      group.secondary,
      module.files?.join(" "),
      module.related_tables?.join(" "),
      module.related_tickets?.join(" "),
    ].join(" "));
    if (haystack.includes(q)) {
      const item = localizeModule(module);
      results.push({
        type: "module",
        id: item.id,
        title: item.displayTitle,
        meta: item.displayTitleAlt || item.displayGroup,
      });
    }
  }
  for (const ticket of state.data.tickets) {
    const zh = TICKET_ZH[ticket.id] || [];
    const haystack = normalize([ticket.id, ticket.title, zh[0], ticket.status, ticket.phase, ticket.service_loop, zh[1]].join(" "));
    if (haystack.includes(q)) {
      const item = localizeTicket(ticket);
      results.push({ type: "ticket", id: item.id, title: item.id, meta: item.displayTitle });
    }
  }
  for (const table of state.data.database) {
    const haystack = normalize([table.id, table.summary, TABLE_ZH[table.id]].join(" "));
    if (haystack.includes(q)) {
      const item = localizeTable(table);
      results.push({ type: "table", id: item.id, title: item.id, meta: t().types.table });
    }
  }
  for (const guardrail of state.data.guardrails) {
    const zh = GUARDRAIL_ZH[guardrail.id] || {};
    const haystack = normalize([guardrail.id, guardrail.title, zh.title, guardrail.summary, zh.summary].join(" "));
    if (haystack.includes(q)) {
      const item = localizeGuardrail(guardrail);
      results.push({ type: "guardrail", id: item.id, title: item.displayTitle, meta: t().types.guardrail });
    }
  }
  return results.slice(0, 14);
}

function renderSearchResults() {
  const results = searchItems(state.query);
  refs.results.hidden = !state.query;
  if (!state.query) {
    refs.results.innerHTML = "";
    return;
  }
  refs.results.innerHTML = `
    <h2>${escapeHtml(results.length ? t().results : t().noResults)}</h2>
    <div class="module-list">
      ${results
        .map((result) => {
          const selected =
            state.selected.type === result.type && state.selected.id === result.id;
          return `
            <button class="result-button${selected ? " is-selected" : ""}" type="button" data-result-type="${escapeHtml(result.type)}" data-result-id="${escapeHtml(result.id)}">
              <span>${escapeHtml(result.title)}</span>
              <small>${escapeHtml(result.meta)}</small>
            </button>
          `;
        })
        .join("")}
    </div>
  `;
  refs.results.querySelectorAll("[data-result-type]").forEach((button) => {
    button.addEventListener("click", () => {
      const type = button.dataset.resultType;
      const view =
        type === "ticket" ? "timeline" : type === "table" ? "data" : type === "guardrail" ? "guardrails" : "graph";
      selectEntity(type, button.dataset.resultId, view);
    });
  });
}

function renderCurrentView() {
  if (!state.data) return;
  if (state.view === "graph") renderGraphView();
  else if (state.view === "timeline") renderTimelineView();
  else if (state.view === "data") renderDataView();
  else if (state.view === "guardrails") renderGuardrailsView();
  else renderOverviewView();
}

function renderOverviewView() {
  const completed = state.data.tickets.filter((ticket) => ticket.status === "completed").length;
  const queued = state.data.tickets.filter((ticket) => ticket.status !== "completed").length;
  const tables = state.data.database.length;
  const copy = t().hero;

  refs.view.innerHTML = `
    <section class="hero" aria-labelledby="hero-title">
      <div class="hero-layout">
        <div class="hero-copy">
          <div class="hero-kicker">
            <span>${escapeHtml(copy.kicker)}</span>
            <small>${escapeHtml(copy.kickerAlt)}</small>
          </div>
          <h1 id="hero-title">
            <span>${escapeHtml(copy.title)}</span>
            <small>${escapeHtml(copy.titleAlt)}</small>
          </h1>
          <p class="hero-body">
            <span>${escapeHtml(copy.body)}</span>
            <small>${escapeHtml(copy.bodyAlt)}</small>
          </p>
          <div class="metric-row" aria-label="${escapeHtml(copy.statusLabel)}">
            <div class="metric"><strong>${completed}</strong><span>${escapeHtml(copy.metrics.completed)}</span></div>
            <div class="metric"><strong>${queued}</strong><span>${escapeHtml(copy.metrics.queued)}</span></div>
            <div class="metric"><strong>${tables}</strong><span>${escapeHtml(copy.metrics.tables)}</span></div>
          </div>
        </div>

        <div class="hero-stage" aria-hidden="true">
          <div class="stage-grid"></div>
          <div class="porcelain-core">
            <span class="core-ring ring-a"></span>
            <span class="core-ring ring-b"></span>
            <span class="core-ring ring-c"></span>
            <span class="core-cross cross-a"></span>
            <span class="core-cross cross-b"></span>
            <div class="core-label">
              <strong>${escapeHtml(copy.stage.core)}</strong>
              <span>${escapeHtml(copy.stage.coreAlt)}</span>
            </div>
            <div class="core-panel panel-ledger">
              <span>01</span>
              <strong>${escapeHtml(copy.stage.ledger)}</strong>
            </div>
            <div class="core-panel panel-diagnosis">
              <span>02</span>
              <strong>${escapeHtml(copy.stage.diagnosis)}</strong>
            </div>
            <div class="core-panel panel-audit">
              <span>03</span>
              <strong>${escapeHtml(copy.stage.audit)}</strong>
            </div>
            <div class="core-panel panel-teaching">
              <span>04</span>
              <strong>${escapeHtml(copy.stage.teaching)}</strong>
            </div>
          </div>
        </div>
      </div>

      <div class="hero-actions">
        ${copy.actions
          .map(
            (action) => `
              <button class="hero-action" type="button" data-jump="${escapeHtml(action.view)}">
                <strong>${escapeHtml(action.title)}</strong>
                <small>${escapeHtml(action.alt)}</small>
                <span>${escapeHtml(action.body)}</span>
              </button>
            `,
          )
          .join("")}
      </div>
    </section>
  `;
  refs.view.querySelectorAll("[data-jump]").forEach((button) => {
    button.addEventListener("click", () => setView(button.dataset.jump));
  });
}

function graphPositions() {
  const counts = {};
  const positions = new Map();
  for (const module of getModules()) {
    const base = GRAPH_GROUPS[module.group] || { x: 64, y: 64 };
    const index = counts[module.group] || 0;
    counts[module.group] = index + 1;
    positions.set(module.id, {
      x: base.x,
      y: base.y + index * NODE.gap,
    });
  }
  return positions;
}

function edgePath(from, to) {
  const x1 = from.x + NODE.width;
  const y1 = from.y + NODE.height / 2;
  const x2 = to.x;
  const y2 = to.y + NODE.height / 2;
  const mid = Math.max(64, Math.abs(x2 - x1) / 2);
  return `M ${x1} ${y1} C ${x1 + mid} ${y1}, ${x2 - mid} ${y2}, ${x2} ${y2}`;
}

function renderGraphView() {
  const positions = graphPositions();
  const selectedModule =
    state.selected.type === "module" ? getModules().find((module) => module.id === state.selected.id) : null;
  const activeIds = new Set([selectedModule?.id, ...(selectedModule?.depends_on || [])].filter(Boolean));
  const edgeMarkup = getModules()
    .flatMap((module) =>
      (module.depends_on || []).map((dependency) => {
        const from = positions.get(dependency);
        const to = positions.get(module.id);
        if (!from || !to) return "";
        const active = activeIds.has(module.id) && activeIds.has(dependency);
        return `<path class="edge${active ? " is-active" : ""}" d="${edgePath(from, to)}" />`;
      }),
    )
    .join("");

  const nodeMarkup = getModules()
    .map((module) => {
      const item = localizeModule(module);
      const pos = positions.get(item.id);
      const selected = state.selected.type === "module" && state.selected.id === item.id;
      const planned = item.status !== "implemented";
      return `
        <g class="graph-node${selected ? " is-selected" : ""}${planned ? " is-planned" : ""}" data-graph-node="${escapeHtml(item.id)}" tabindex="0" role="button" aria-label="${escapeHtml(item.displayTitle)}" transform="translate(${pos.x} ${pos.y})">
          <title>${escapeHtml(item.displaySummary)}</title>
          <rect class="node-rect" width="${NODE.width}" height="${NODE.height}"></rect>
          <rect class="node-chip" x="13" y="15" width="7" height="7"></rect>
          <text class="node-title" x="30" y="24">${escapeHtml(truncate(item.displayTitle, 25))}</text>
          <text class="node-meta" x="30" y="43">${escapeHtml(truncate(item.displayGroup, 26))}</text>
        </g>
      `;
    })
    .join("");

  refs.view.innerHTML = `
    <section class="atlas-section">
      <div class="section-header">
        <div>
          <div class="eyebrow">${escapeHtml(t().sections.graphKicker)}</div>
          <h1>${escapeHtml(t().sections.graphTitle)}</h1>
        </div>
        <p>${escapeHtml(t().sections.graphBody)}</p>
      </div>
      <div class="graph-wrap">
        <svg class="graph" viewBox="0 0 1230 810" role="img" aria-label="${escapeHtml(t().graphAria)}">
          <g class="edges">${edgeMarkup}</g>
          <g class="nodes">${nodeMarkup}</g>
        </svg>
      </div>
    </section>
  `;
  refs.view.querySelectorAll("[data-graph-node]").forEach((node) => {
    const select = () => selectEntity("module", node.dataset.graphNode, "graph");
    node.addEventListener("click", select);
    node.addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        select();
      }
    });
  });
}

function renderTimelineView() {
  refs.view.innerHTML = `
    <section class="atlas-section">
      <div class="section-header">
        <div>
          <div class="eyebrow">${escapeHtml(t().sections.timelineKicker)}</div>
          <h1>${escapeHtml(t().sections.timelineTitle)}</h1>
        </div>
        <p>${escapeHtml(t().sections.timelineBody)}</p>
      </div>
      <div class="timeline">
        ${state.data.tickets
          .map((ticket) => {
            const item = localizeTicket(ticket);
            return `
              <button class="timeline-item" type="button" data-ticket="${escapeHtml(item.id)}" data-status="${escapeHtml(item.status)}">
                <strong>${escapeHtml(item.id)}</strong>
                <span>
                  <strong>${escapeHtml(item.displayTitle)}</strong>
                  <small>${escapeHtml(item.displayTitleAlt)}</small>
                  <p>${escapeHtml(item.displayLoop)}</p>
                </span>
                <span class="status-pill" data-status="${escapeHtml(item.status)}">${escapeHtml(statusLabel(item.status))}</span>
              </button>
            `;
          })
          .join("")}
      </div>
    </section>
  `;
  refs.view.querySelectorAll("[data-ticket]").forEach((button) => {
    button.addEventListener("click", () => selectEntity("ticket", button.dataset.ticket, "timeline"));
  });
}

function renderDataView() {
  refs.view.innerHTML = `
    <section class="atlas-section">
      <div class="section-header">
        <div>
          <div class="eyebrow">${escapeHtml(t().sections.dataKicker)}</div>
          <h1>${escapeHtml(t().sections.dataTitle)}</h1>
        </div>
        <p>${escapeHtml(t().sections.dataBody)}</p>
      </div>
      <div class="table-grid">
        ${state.data.database
          .map((table) => {
            const item = localizeTable(table);
            return `
              <button class="table-row" type="button" data-table="${escapeHtml(item.id)}">
                <strong>${escapeHtml(item.displayTitle)}</strong>
                <p>
                  <span>${escapeHtml(item.displaySummary)}</span>
                  <small>${escapeHtml(item.displaySummaryAlt)}</small>
                </p>
                <span class="tag">${escapeHtml(item.hot_path ? t().tableTags.indexed : t().tableTags.table)}</span>
              </button>
            `;
          })
          .join("")}
      </div>
    </section>
  `;
  refs.view.querySelectorAll("[data-table]").forEach((button) => {
    button.addEventListener("click", () => selectEntity("table", button.dataset.table, "data"));
  });
}

function renderGuardrailsView() {
  refs.view.innerHTML = `
    <section class="atlas-section">
      <div class="section-header">
        <div>
          <div class="eyebrow">${escapeHtml(t().sections.guardrailKicker)}</div>
          <h1>${escapeHtml(t().sections.guardrailTitle)}</h1>
        </div>
        <p>${escapeHtml(t().sections.guardrailBody)}</p>
      </div>
      <div class="guardrail-grid">
        ${state.data.guardrails
          .map((guardrail) => {
            const item = localizeGuardrail(guardrail);
            return `
              <button class="guardrail" type="button" data-guardrail="${escapeHtml(item.id)}">
                <strong>${escapeHtml(item.displayTitle)}</strong>
                <small>${escapeHtml(item.displayTitleAlt)}</small>
                <p>${escapeHtml(item.displaySummary)}</p>
              </button>
            `;
          })
          .join("")}
      </div>
    </section>
  `;
  refs.view.querySelectorAll("[data-guardrail]").forEach((button) => {
    button.addEventListener("click", () => selectEntity("guardrail", button.dataset.guardrail, "guardrails"));
  });
}

function renderInspector() {
  const entity = getSelectedEntity();
  if (!entity) {
    refs.inspectorState.textContent = t().noSelection;
    refs.inspector.innerHTML = `<div class="inspector-empty">${escapeHtml(t().selectPrompt)}</div>`;
    return;
  }

  refs.inspectorState.textContent = t().types[state.selected.type] || state.selected.type;
  const title = entity.displayTitle || entity.title || entity.id;
  const secondaryTitle = entity.displayTitleAlt || "";
  const summary = entity.displaySummary || entity.summary || entity.service_loop || "";
  const secondarySummary = entity.displaySummaryAlt || "";
  const files = toList(entity.files);
  const tables = toList(entity.related_tables);
  const tickets = toList(entity.related_tickets);
  const sourceRefs = toList(entity.source_refs);
  const dependencies = toList(entity.depends_on);

  refs.inspector.innerHTML = `
    <article class="inspector-card">
      <h2 class="inspector-title">${escapeHtml(title)}</h2>
      ${secondaryTitle ? `<p class="inspector-subtitle">${escapeHtml(secondaryTitle)}</p>` : ""}
      <p class="inspector-summary">
        <span>${escapeHtml(summary)}</span>
        ${secondarySummary ? `<small>${escapeHtml(secondarySummary)}</small>` : ""}
      </p>
      <div class="token-list">
        ${entity.displayGroup ? `<span class="tag">${escapeHtml(entity.displayGroup)}</span>` : ""}
        ${entity.status ? `<span class="status-pill" data-status="${escapeHtml(entity.status)}">${escapeHtml(statusLabel(entity.status))}</span>` : ""}
        ${entity.phase ? `<span class="tag">${escapeHtml(entity.phase)}</span>` : ""}
      </div>
      ${files.length ? inspectorBlock(t().inspectorBlocks.files, `<div class="file-list">${files.map((file) => `<code>${escapeHtml(file)}</code>`).join("")}</div>`) : ""}
      ${dependencies.length ? inspectorBlock(t().inspectorBlocks.dependencies, tokenList(dependencies)) : ""}
      ${tables.length ? inspectorBlock(t().inspectorBlocks.tables, tokenList(tables)) : ""}
      ${tickets.length ? inspectorBlock(t().inspectorBlocks.tickets, tokenList(tickets)) : ""}
      ${sourceRefs.length ? inspectorBlock(t().inspectorBlocks.sourceRefs, `<ul class="source-list">${sourceRefs.map((ref) => `<li>${escapeHtml(ref)}</li>`).join("")}</ul>`) : ""}
    </article>
  `;
}

function inspectorBlock(title, content) {
  return `
    <section class="inspector-block">
      <h3>${escapeHtml(title)}</h3>
      ${content}
    </section>
  `;
}

function tokenList(items) {
  return `<div class="token-list">${items.map((item) => `<span class="tag">${escapeHtml(item)}</span>`).join("")}</div>`;
}

function render() {
  renderChrome();
  renderTabs();
  if (state.data) {
    renderModuleTree();
    renderSearchResults();
    renderCurrentView();
    renderInspector();
  }
}

function initEvents() {
  refs.tabs.forEach((button) => {
    button.addEventListener("click", () => setView(button.dataset.view));
  });

  refs.langButtons.forEach((button) => {
    button.addEventListener("click", () => setLanguage(button.dataset.lang));
  });

  refs.search.addEventListener("input", (event) => {
    state.query = event.target.value.trim();
    renderSearchResults();
  });

  refs.search.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      refs.search.value = "";
      state.query = "";
      renderSearchResults();
    }
    if (event.key === "Enter") {
      const first = refs.results.querySelector("[data-result-type]");
      if (first) first.click();
    }
  });

  window.addEventListener("hashchange", () => {
    const view = window.location.hash.replace("#", "");
    if (view && view !== state.view && refs.tabs.some((button) => button.dataset.view === view)) {
      state.view = view;
      render();
    }
  });
}

function initAmbientField() {
  const canvas = refs.canvas;
  const context = canvas.getContext("2d");
  const particles = [];
  let width = 0;
  let height = 0;
  let frame = 0;

  function resize() {
    const ratio = window.devicePixelRatio || 1;
    width = window.innerWidth;
    height = window.innerHeight;
    canvas.width = Math.floor(width * ratio);
    canvas.height = Math.floor(height * ratio);
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
    particles.length = 0;
    const count = Math.max(18, Math.min(42, Math.floor((width * height) / 43000)));
    for (let index = 0; index < count; index += 1) {
      particles.push({
        text: AMBIENT_TERMS[index % AMBIENT_TERMS.length],
        x: Math.random() * width,
        y: Math.random() * height,
        speed: 0.18 + Math.random() * 0.32,
        size: 11 + Math.random() * 15,
        alpha: 0.026 + Math.random() * 0.04,
      });
    }
  }

  function draw() {
    context.clearRect(0, 0, width, height);
    context.textBaseline = "middle";
    for (const item of particles) {
      context.globalAlpha = item.alpha;
      context.fillStyle = "#327a73";
      context.font = `600 ${item.size}px JetBrains Mono, Cascadia Mono, Microsoft YaHei, monospace`;
      context.fillText(item.text, item.x, item.y);
      if (!state.reducedMotion) {
        item.x += item.speed;
        item.y += Math.sin((frame + item.x) / 240) * 0.03;
        if (item.x > width + 180) item.x = -180;
      }
    }
    context.globalAlpha = 1;
    frame += 1;
    if (!state.reducedMotion) requestAnimationFrame(draw);
  }

  resize();
  window.addEventListener("resize", resize);
  draw();
}

async function boot() {
  initEvents();
  initAmbientField();
  const hashView = window.location.hash.replace("#", "");
  if (refs.tabs.some((button) => button.dataset.view === hashView)) {
    state.view = hashView;
  }
  renderChrome();
  renderTabs();
  try {
    await loadAtlasData();
    render();
  } catch (error) {
    refs.view.innerHTML = `
      <section class="atlas-section">
        <div class="section-header">
          <div>
            <div class="eyebrow">${escapeHtml(t().loadingErrorKicker)}</div>
            <h1>${escapeHtml(t().loadingErrorTitle)}</h1>
          </div>
          <p>${escapeHtml(error.message)}</p>
        </div>
      </section>
    `;
  }
}

boot();

# Polaris Core 漫画生图与分镜制作包 v0.1

> 本文把 `docs/polaris-core-comic-script-v0.md` 转成可制作提示词。当前先覆盖第 1-12 页；后续按同一模板继续第 13-72 页。

## 1. 使用原则

每一页都应先保证角色一致、信息锚点清楚，再追求画面华丽。

优先级如下：

1. Mona 形象稳定；
2. 哆啦A梦道具功能明确；
3. Polaris Core 的系统隐喻准确；
4. 画面有漫画分镜节奏；
5. 文案不要挤满画面。

如果图像模型不能稳定生成可读文字，文字应在后期排版添加。提示词里可以描述“预留标题区”“发光标签”“仪表盘标签”，但不要依赖模型生成准确中文。

## 2. 全局角色锁定

### Mona

```text
Mona, a delicate white-haired young anime researcher, one golden eye and one unfinished sketch-line eye, small star hair ornament, deep indigo star-map cloak with constellation lines and transparent glowing edges, white inner dress, short boots, holding a warm golden lantern, blue-violet transparent light veil around her hair like a starry aurora, floating square data particles and tiny stars, partially unfinished pencil sketch lines on parts of her body, calm intelligent expression, not childish, not over-chibi
```

### 哆啦A梦（自用版）

```text
Doraemon as the familiar blue robotic cat companion, round blue body, white face and belly, red nose, small bell, expressive but not stealing the scene, using pocket gadgets to visualize abstract learning mechanisms, standing beside Mona as a helper and comic guide
```

公开版替换时使用：

```text
original blue pocket mentor, round friendly robot companion, blue and white body, glowing tool pocket, warm gold accent, no copyrighted character features, helper who produces visual metaphor gadgets
```

## 3. 全局画风

### 主风格提示词

```text
elegant high-detail anime comic page, refined futuristic learning engine city, deep indigo and warm gold color contrast, luminous star maps, transparent holographic UI, constellation lines, soft cinematic glow, clean panel composition, intelligent quiet mood, premium educational sci-fi manga, delicate lighting, crisp shapes, readable visual hierarchy
```

### 负向提示词

```text
low age preschool style, over-chibi, messy composition, muddy colors, cluttered text, generic fantasy battle, random magic symbols, horror, grotesque expression, cheap poster design, blurry characters, inconsistent face, overexposed glow, unreadable tiny labels, text covering character faces, empty background
```

## 4. 页面提示词模板

每页生成时建议使用以下结构：

```text
PAGE <number>, <title>.
Comic page with <panel count> panels.
Main visual goal: <one sentence>.
Characters: <Mona lock>, <Doraemon lock if present>.
Setting: <scene>.
Panel 1: <composition>.
Panel 2: <composition>.
Panel 3: <composition>.
Panel 4: <composition>.
Text layout: leave clean empty caption boxes / title area; do not generate dense readable text.
Style: <global style>.
Negative: <global negative>.
```

## 5. 第一章逐页提示词（1-12 页）

### 第 1 页：城门外的线稿

**画面目标：** 建立星轨城、Mona 和哆啦A梦，抛出“验证理解”的高级感。

**分镜布局：** 整页大画 + 3 个小视觉焦点。建议竖版漫画页。

```text
PAGE 1, The unfinished line art at the city gate.
Full-page cinematic anime comic opening. Mona stands before a gigantic futuristic city gate labeled visually as Polaris Core, most districts dark and unlit under a deep indigo starry sky. Mona is a delicate white-haired young researcher with one golden eye and one unfinished sketch-line eye, star hair ornament, deep indigo constellation cloak, white dress, short boots, holding a weak warm golden lantern. Parts of her body remain unfinished pencil line art. Doraemon stands beside her, looking up at the gate, small and supportive, with his gadget pocket slightly bulging.
Composition: vast city gate dominates the upper page, Mona and Doraemon small in the lower foreground, transparent data streams and constellation rails faintly visible behind the gate. Reserve a clean title area across the upper third for the line "不要问你记住了没，问你真的懂了吗？" to be added later.
Mood: quiet, mysterious, intelligent, not childish.
Style: elegant high-detail anime comic page, refined futuristic learning engine city, deep indigo and warm gold contrast, luminous star maps, transparent holographic UI, soft cinematic glow.
Negative: low age preschool style, over-chibi, messy composition, muddy colors, cluttered text, random fantasy magic, blurry face, unreadable tiny labels.
```

**检查点：** Mona 的灯笼要弱，城市大部分区域要暗，主角不能像幼儿吉祥物。

### 第 2 页：100 分气泡

**画面目标：** 用“100 分气泡”戳破背诵等于理解的错觉。

**分镜布局：** 4 格，前两格观察，后两格动作。

```text
PAGE 2, The 100-point bubble.
Four-panel anime comic page. Setting remains outside the Polaris Core city gate.
Panel 1: A sweating student NPC rapidly recites code and definitions, a huge colorful bubble above his head marked as a perfect-score symbol, floating letter blocks inside the bubble.
Panel 2: Mona watches calmly from the side, her warm lantern does not brighten, her expression thoughtful and skeptical.
Panel 3: Doraemon pulls out a tiny gadget like a confidence-popping needle from his pocket and lightly touches the bubble, playful but not chaotic.
Panel 4: The bubble pops silently; disconnected letter blocks spill out and scatter on the ground, showing memorized fragments without structure. Mona remains composed.
Text layout: leave speech bubble space near Mona in panel 2 and Doraemon in panel 4.
Style: premium educational sci-fi manga, clean panel borders, deep indigo city background, warm gold lantern accent, crisp symbolic objects.
Negative: slapstick violence, ugly NPC, cluttered text, exaggerated comedy face, preschool style.
```

**检查点：** 气泡破裂是“认知错觉破裂”，不是打斗。

### 第 3 页：记忆保质期

**画面目标：** 表达 FSRS 管遗忘，不等于理解。

**分镜布局：** 4 格，道具演示页。

```text
PAGE 3, Memory shelf-life sticker.
Four-panel comic page inside the dim entrance hall of Polaris Core.
Panel 1: Doraemon presents a small glowing sticker gadget shaped like an hourglass label, symbolizing memory review timing.
Panel 2: Close-up of the sticker attached to Mona's golden lantern shell; the shell glows with cool blue timing marks, but the lantern flame inside remains weak.
Panel 3: Doraemon explains with a small diagram of a fading memory curve floating from the sticker, friendly and precise.
Panel 4: Mona looks into the lantern flame with a calm realization; her unfinished sketch-line hand is visible around the handle.
Text layout: reserve one small caption box for "FSRS predicts forgetting; it does not prove understanding" to be typeset later.
Style: delicate anime illustration, transparent holographic UI, warm lantern versus cool blue memory sticker, clean sci-fi educational tone.
Negative: too much text, fantasy spell effects, childish sticker overload, messy diagrams.
```

**检查点：** 灯笼外壳亮、灯芯不亮，这是本页核心。

### 第 4 页：三段路

**画面目标：** 第一次完整展示主命题。

**分镜布局：** 5 格，横向光路递进。

```text
PAGE 4, The three-part road.
Five-panel comic page. The Polaris Core gate opens into a luminous three-stage path.
Panel 1: Wide view, city gate opens, a glowing road splits into three connected segments.
Panel 2: First segment visualized with evidence tickets and a small verification seal, representing "verify real understanding".
Panel 3: Second segment visualized with a glowing map and highlighted foggy area, representing "locate ambiguity".
Panel 4: Third segment visualized with three task capsules, representing "targeted remediation".
Panel 5: Mona stands at the beginning of the path; her lantern turns slightly warmer for the first time, Doraemon watches quietly beside her.
Text layout: reserve three clean label areas on the road segments; exact Chinese text can be added later.
Style: elegant sci-fi manga, luminous road, symbolic UI, deep indigo city interior, warm gold accents.
Negative: single generic road, overdecorated fantasy portal, cluttered labels, unreadable typography.
```

**检查点：** 三段必须清楚，不能只画成普通冒险路。

### 第 5 页：不是一根分数条

**画面目标：** 把多维掌握度仪表盘做成第一章核心视觉。

**分镜布局：** 5 格，仪表逐个亮起。

```text
PAGE 5, Not a single score bar.
Five-panel anime comic page inside a futuristic control room. Mona stands before a precision dashboard with multiple elegant analog-digital gauges, no single total score bar.
Panel 1: Dashboard rises from the floor in front of Mona, many fine needles and glass dials.
Panel 2: Gauge R lights up with a small memory hourglass icon.
Panel 3: Gauge p_known lights up with a probability needle and evidence dots.
Panel 4: Gauge C lights up with two offset lines showing confidence versus correctness.
Panel 5: Gauge D lights up as a four-step depth stair: recall, explain, apply, transfer. Mona's lantern reflects in the glass.
Text layout: leave short label spaces on each gauge; do not attempt dense readable labels.
Style: refined futuristic UI, transparent glass, star-map reflections, warm gold and deep blue, high readability.
Negative: game HUD clutter, random numbers, giant score bar, cheap dashboard, unreadable text.
```

**检查点：** theta/q 不在本页展开，只可作为远处星图伏笔。

### 第 6 页：外部导师与核心引擎

**画面目标：** 明确外部 AI 能讲解，不能裁决掌握度。

**分镜布局：** 5 格，对照式。

```text
PAGE 6, External tutors and the core engine.
Five-panel comic page at the border of Polaris Core.
Panel 1: Several translucent external AI tutor silhouettes outside the gate, each with different style: code assistant, language coach, diagram tutor. They hold megaphones and lesson notes.
Panel 2: Mona points to them with a questioning expression; her lantern remains steady but cautious.
Panel 3: Doraemon pulls out two symbolic gadgets: a megaphone and a judge's gavel.
Panel 4: Doraemon gives the megaphone toward the external tutors, while the judge's gavel is placed back inside the city court behind a secure gate.
Panel 5: The city gate displays a clean glowing rule panel, visually implying "external judgment enters as evidence".
Style: clear architectural separation, tutors outside, engine inside, elegant sci-fi comic composition.
Negative: external tutors controlling the dashboard, chaotic crowd, legal courtroom overload, text-heavy UI.
```

**检查点：** 木槌要回到城内，不能在外部导师手里。

### 第 7 页：课程不能野蛮入城

**画面目标：** 解释课程不能直接塞进内核。

**分镜布局：** 4 格，海关安检隐喻。

```text
PAGE 7, A course cannot be shoved into the core.
Four-panel comic page at a futuristic customs checkpoint inside Polaris Core.
Panel 1: A chaotic pile of textbooks, videos, notes, flashcards, and problem sets rushes toward a glowing security gate.
Panel 2: The gate flashes red and blocks the pile; papers scatter in an orderly but dramatic way. The rejection is about structure, not value.
Panel 3: Mona crouches and picks up one scattered lecture page, thoughtful, not panicked.
Panel 4: Doraemon reaches into his pocket, preparing a standardization gadget, while the customs gate glows behind him.
Style: clean sci-fi customs area, symbolic educational materials, no messy visual noise, Mona calm and analytical.
Negative: junkyard chaos, aggressive guards, unreadable piles of text, slapstick destruction.
```

**检查点：** 拒绝的是“非结构化入城”，不是否定课程内容。

### 第 8 页：Domain Pack

**画面目标：** 把 Domain Pack 画成标准模块。

**分镜布局：** 4 格，变换过程。

```text
PAGE 8, Domain Pack appears.
Four-panel comic page.
Panel 1: Doraemon unfolds a glowing standard wrapping cloth around the chaotic course materials.
Panel 2: The materials transform into a transparent crystalline module with clean internal layers.
Panel 3: Close-up of the module surface with abstract file-card shapes representing pack.toml, concepts.toml, misconceptions.toml, rubric.md, moves.toml; do not rely on precise readable text.
Panel 4: The module slides into a city slot with a satisfying light connection; a distant district of Polaris Core lights up.
Mona watches, lantern glowing warmer.
Style: elegant transformation, glass module, warm gold insertion light, deep indigo city infrastructure.
Negative: magical explosion, childish wrapping cloth, cluttered file names, random cyberpunk grime.
```

**检查点：** 这页要有“内容自由，接入标准”的视觉秩序感。

### 第 9 页：包里不是魔法

**画面目标：** 展示 pack 内部结构，不把 Domain Pack 神秘化。

**分镜布局：** 5 格，模块剖面。

```text
PAGE 9, The pack is not magic.
Five-panel comic page showing a clean exploded view of the Domain Pack module.
Panel 1: The transparent module floats open like an architectural model.
Panel 2: Concepts appear as miniature buildings arranged in a city block.
Panel 3: Edges appear as roads and bridges between buildings.
Panel 4: Rubric appears as a precise measuring ruler and moves appear as small instruction cards.
Panel 5: Misconceptions appear as warning cards pinned to dangerous intersections. Mona illuminates each layer with her lantern.
Style: technical elegance, miniature city model, transparent layers, readable hierarchy, warm lantern inspection.
Negative: spellbook aesthetic, random folders, text clutter, low-detail icons.
```

**检查点：** `misconceptions` 是警示记录，不要画成固定概念站点。

### 第 10 页：调度器大厅

**画面目标：** 展示系统给 3 个候选任务，而不是唯一命令。

**分镜布局：** 5 格，机器吐出候选。

```text
PAGE 10, Next Task hall.
Five-panel comic page inside a vast scheduling hall.
Panel 1: Mona and Doraemon enter a circular hall with a giant elegant scheduling machine in the center.
Panel 2: Several transparent data ribbons flow into the machine, visually representing retention, calibration gap, misconception risk, and prerequisite status.
Panel 3: The machine outputs three glowing task capsules, not a single command.
Panel 4: Capsules show visual icons: review old concept, compare confusing concepts, introduce new concept.
Panel 5: One recommended capsule receives a warm gold outline; Mona observes that the system shows reasons, not orders.
Style: clean sci-fi operations room, transparent data streams, three clear capsules, warm gold recommendation, no clutter.
Negative: slot machine randomness, one giant command arrow, game loot chest, messy labels.
```

**检查点：** 必须是 3 个候选，推荐项只是高亮。

### 第 11 页：第一颗胶囊

**画面目标：** Mona 接受第一轮任务，但哆啦A梦不替她答。

**分镜布局：** 4 格，准备作答。

```text
PAGE 11, The first task capsule.
Four-panel comic page.
Panel 1: Mona picks up the recommended warm-gold task capsule; it displays a symbolic ownership icon rather than dense text.
Panel 2: The capsule unfolds into a holographic Rust ownership question, shown as abstract resource tokens and arrows.
Panel 3: Mona's unfinished sketch-line hand grips a light pen; her lantern floats beside the question, illuminating the problem structure.
Panel 4: Doraemon sits nearby and offers a blank glowing evidence ticket, clearly not answering for her.
Style: focused task moment, minimal background, warm lantern light, refined holographic question.
Negative: Doraemon solving the question, noisy code wall, battle stance, childish school test scene.
```

**检查点：** Mona 是主动作答者。

### 第 12 页：Attempt

**画面目标：** 把作答变成 evidence 和 attempt。

**分镜布局：** 5 格，进入证据法庭前的闭环启动。

```text
PAGE 12, Attempt created.
Five-panel comic page.
Panel 1: Mona writes an answer with a light pen; the strokes are neat explanatory lines, not decorative magic.
Panel 2: A small confidence selector appears before feedback, glowing softly beside her answer.
Panel 3: Her answer folds into a luminous evidence ticket stamped as an evidence item symbol.
Panel 4: A second record card appears behind it, representing an attempt created from the evidence.
Panel 5: A formal evidence court door lights up ahead; Mona looks toward it with calm focus, lantern warmer than before.
Style: elegant procedural transformation, evidence ticket, attempt record card, warm gold light, deep blue court doorway.
Negative: instant final score, too much text, chaotic paperwork, magical combat, overly cute expressions.
```

**检查点：** 本页不要出现 final score；只表现 evidence 与 attempt 创建。

## 6. 第二章逐页提示词（13-24 页）

### 第 13 页：暂记估值

**画面目标：** 让 provisional 看起来像“暂记账”，不是最终分数。

**分镜布局：** 5 格，状态即时变化 + 法庭远景预告。

```text
PAGE 13, provisional estimate stamp. Five-panel comic page in a clean sci-fi status chamber. Mona has just submitted her evidence ticket; a warm-gold provisional stamp lands on a floating status panel, clearly marked as temporary estimate rather than final grade. Doraemon holds a small stamp gadget beside her. A distant evidence court door glows ahead. Style: elegant UI, warm gold temporary stamp, no final score, calm procedural mood. Negative: final grade celebration, giant numeric score, chaotic paperwork.

Panel 1: Mona's glowing evidence ticket leaves her hand and enters a transparent intake slot.
Panel 2: A status panel wakes immediately beside her, but it is framed as temporary bookkeeping, not a final report.
Panel 3: Doraemon presses a small provisional stamp gadget; a warm-gold "temporary estimate" seal lands softly.
Panel 4: Mona looks surprised but calm, understanding that the system has moved without waiting for the slow background judge.
Panel 5: In the distance, the formal evidence court door lights up, showing that final review is still ahead.
Text layout: leave space for one caption explaining provisional update; avoid visible numeric score.
Style: elegant UI, warm gold temporary stamp, no final score, calm procedural mood.
Negative: final grade celebration, giant numeric score, chaotic paperwork, game score pop-up.
```

**检查点：** 不出现最终分数，只出现暂记估值。

### 第 14 页：Tier 1 法官

**画面目标：** 建立后台评分员也受系统规则约束。

**分镜布局：** 4 格，从票据入庭到规则亮起。

```text
PAGE 14, Tier 1 judge. Four-panel comic page in a futuristic evidence court. A luminous evidence ticket enters the court. A white-gloved AI judge figure sits at the bench, about to make a vague judgment, while red evidence-bound warning lights activate. Mona watches from the side with calm attention; Doraemon prepares a citation gadget. Style: refined sci-fi courtroom, transparent glass bench, strict but not scary. Negative: evil judge, legal clutter, comedy chaos.

Panel 1: The evidence ticket arrives on a rail into a transparent sci-fi courtroom; the room is elegant, strict, and quiet.
Panel 2: A Tier 1 AI judge figure with white gloves opens the ticket, surrounded by faint model-like light patterns.
Panel 3: The judge starts to make a vague "probably correct" gesture, but the court walls flash evidence-bound warning lights.
Panel 4: Mona sits in a side observation area, lantern steady; Doraemon reaches for a citation highlighter gadget.
Text layout: reserve a small rule panel area above the judge bench; do not rely on generated readable text.
Style: refined sci-fi courtroom, transparent glass bench, strict but not scary.
Negative: evil judge, legal clutter, comedy chaos, dark punishment scene.
```

**检查点：** 法官是后台评分员，不是最终权力中心。

### 第 15 页：strict-citation

**画面目标：** 把 strict-citation 画成“证据荧光笔 + 扫描门”。

**分镜布局：** 5 格，原文划线、三项校验、退回含糊判断。

```text
PAGE 15, strict citation scanner. Five-panel page. Doraemon uses a glowing evidence highlighter gadget to force exact quotes from Mona's original answer. A scanner checks quote substring, evidence id ownership, and length boundaries as abstract check icons. Invalid vague judgment fragments bounce away in red. Mona observes with a serious expression. Style: clean courtroom UI, glowing citation lines, precision, no dense readable text. Negative: random magic, unreadable JSON wall, messy red errors.

Panel 1: Doraemon pulls out a glowing citation highlighter from his pocket and hands it toward the court scanner.
Panel 2: The highlighter marks exact lines inside Mona's original answer ticket; the highlighted quote remains visibly attached to the source text.
Panel 3: Three transparent check gates appear: source evidence id, quote is substring, quote length boundary; each gate uses icons rather than dense text.
Panel 4: A vague floating phrase like "seems right" is rejected by a red scanning beam and bounces away.
Panel 5: Mona watches seriously, lantern reflected in the scanner glass, realizing that even AI judgment needs evidence.
Text layout: leave one narrow caption area for "Quote must come from the evidence text"; avoid walls of JSON.
Style: clean courtroom UI, glowing citation lines, precision, no dense readable text.
Negative: random magic, unreadable JSON wall, messy red errors, decorative highlighter with no source text.
```

**检查点：** 重点是“引用来自原文”，不是普通荧光装饰。

### 第 16 页：结构化判卷

**画面目标：** 显示评分输出必须结构化。

**分镜布局：** 4 格，卷宗字段亮起 + depth 四门。

```text
PAGE 16, structured grading dossier. Four-panel page. The AI judge submits a transparent JSON-like dossier with four clean sections represented by icons: score, depth, misconception, citations. Four depth doors appear: recall, explain, apply, transfer. The system scanner approves the structure with a green candidate seal. Style: elegant document UI, court light beams, structured order. Negative: dense code text, final celebration, messy interface.

Panel 1: The AI judge submits a transparent grading dossier, shaped like a clean glass document rather than a code wall.
Panel 2: Four sections light up as icons: score meter, depth gate, misconception card, citation chain.
Panel 3: The depth section unfolds into four elegant doors of increasing height, symbolizing recall, explain, apply, transfer.
Panel 4: A system scanner passes over the dossier and applies a green "valid structure" seal, not a mastery seal.
Text layout: keep labels short or add later in post-production; preserve clean empty boxes for annotations.
Style: elegant document UI, court light beams, structured order.
Negative: dense code text, final celebration, messy interface, giant pass/fail stamp.
```

**检查点：** `depth` 四门要清楚，但不强求模型生成可读英文。

### 第 17 页：断网与队列

**画面目标：** 失败评分进入 grade_queue，系统不中断。

**分镜布局：** 5 格，坏卷宗转入保险箱，Mona 状态不崩。

```text
PAGE 17, grade queue safe box. Five-panel page. A broken malformed grading dossier emits smoke and is diverted onto a conveyor belt into a glowing safe box labeled visually as review queue. Doraemon pats the safe box; Mona's lantern stays lit because provisional state remains active. Style: procedural safety, clean queue machine, warm reassurance. Negative: system crash, panic, dirty data entering dashboard.

Panel 1: A malformed grading dossier arrives with scrambled fields and small smoke puffs, but the court remains orderly.
Panel 2: A clean mechanical arm diverts the dossier away from the mastery dashboard before it can contaminate state.
Panel 3: The dossier travels on a conveyor belt into a glowing review safe box, visually representing grade_queue.
Panel 4: Doraemon pats the safe box with a reassuring gesture; a small retry counter icon glows on the front.
Panel 5: Mona's provisional status panel remains stable, her lantern still lit, showing graceful degradation.
Text layout: reserve a small safe-box label area; do not depend on readable queue internals.
Style: procedural safety, clean queue machine, warm reassurance.
Negative: system crash, panic, dirty data entering dashboard, broken city disaster.
```

**检查点：** 失败不是崩溃，是排队重试。

### 第 18 页：final 到达

**画面目标：** final 是回填到 attempt，不是擦掉 provisional。

**分镜布局：** 4 格，final 卡片回到原 attempt 槽位。

```text
PAGE 18, final score arrives. Four-panel page. A compliant final result returns from the evidence court as a sealed light card. It lands on the original attempt record beside the earlier provisional field, showing two coexisting slots. Mona points at the record, understanding that history is completed rather than overwritten. Style: transparent record cards, precise data slots, calm gold-blue light. Negative: eraser, replacement animation, single giant score.

Panel 1: A compliant final result card leaves the evidence court, sealed by a citation chain and a structured output mark.
Panel 2: The card travels back along a light rail toward the original attempt record, not toward a separate score board.
Panel 3: Close-up: the attempt card now shows two adjacent slots, one for provisional estimate and one for final result.
Panel 4: Mona points at the paired slots and looks relieved, seeing that history was completed rather than overwritten.
Text layout: leave two small field-label areas on the record card for post-production.
Style: transparent record cards, precise data slots, calm gold-blue light.
Negative: eraser, replacement animation, single giant score, celebratory final-grade splash.
```

**检查点：** 同一卡片上要有 provisional 与 final 的并存感。

### 第 19 页：没有橡皮擦的城市

**画面目标：** 事件溯源的“胶片重放”视觉。

**分镜布局：** 5 格，事件放映机把事实折叠成状态。

```text
PAGE 19, city without erasers. Five-panel page. Doraemon unfolds an event projector gadget. Film strips of Mona's past attempts fly out in chronological order. The final result triggers a replay of all attempts for the same concept, folding into a refreshed mastery state. Mona watches the process like a precise machine, not time travel magic. Style: cinematic film strips, data replay, elegant engineering metaphor. Negative: literal time travel, erased history, chaotic reels.

Panel 1: Doraemon unfolds a compact event projector, a gadget with film reels and holographic data rails.
Panel 2: Film strips of Mona's past attempts rise in chronological order, each strip connected to evidence tickets.
Panel 3: The final result card inserts into the correct place on the timeline, without deleting earlier frames.
Panel 4: The timeline folds through a transparent calculation prism into refreshed mastery gauges.
Panel 5: Mona watches calmly, understanding the city has no eraser, only replay and fold.
Text layout: leave one caption space for "facts immutable, state replayable".
Style: cinematic film strips, data replay, elegant engineering metaphor.
Negative: literal time travel, erased history, chaotic reels, magical rewind.
```

**检查点：** 画成重放折叠，不画成时间倒流改历史。

### 第 20 页：仪表盘重新落位

**画面目标：** 多维状态根据证据重新稳定。

**分镜布局：** 5 格，四个指标逐格回落到证据支撑位置。

```text
PAGE 20, dashboard settles again. Five-panel page. The multi-gauge dashboard updates: p_known needle shifts, calibration gap shows offset lines, depth gauge stays at explain, FSRS hourglass updates next review timing. Mona reads the dashboard with a measured expression. Style: transparent glass gauges, precise needles, warm lantern reflection. Negative: one score bar, random numbers, game level-up effect.

Panel 1: The multi-gauge dashboard receives the folded state and all needles tremble briefly.
Panel 2: The p_known probability needle moves to a new stable position based on evidence.
Panel 3: The calibration gauge shows two offset light lines, one for confidence and one for result.
Panel 4: The depth gauge stops at explain instead of jumping to transfer; the FSRS hourglass updates review timing nearby.
Panel 5: Mona studies the whole dashboard, seeing that state returned to what evidence can support.
Text layout: leave short label spaces on gauges; avoid random generated numbers.
Style: transparent glass gauges, precise needles, warm lantern reflection.
Negative: one score bar, random numbers, game level-up effect, fireworks.
```

**检查点：** p_known、C、D、R 四种视觉同时出现。

### 第 21 页：真实之镜

**画面目标：** 校准差不是羞辱，而是照见错觉。

**分镜布局：** 4 格，空心投影 + 自信/结果错位。

```text
PAGE 21, mirror of calibration. Four-panel page. Doraemon brings out a truth mirror. In the mirror, Mona's overconfident projection appears large but hollow and incomplete, with missing structure lines. Mona calmly brings her lantern closer, not frightened. Abstract UI shows high confidence versus low final result as two offset light bars. Style: reflective mirror, warm compassionate light, psychological clarity. Negative: horror monster, body shaming, ugly caricature.

Panel 1: Doraemon sets a tall truth mirror in the chamber; the frame is warm gold and deep blue, not ominous.
Panel 2: In the mirror, Mona's overconfident projection appears larger than reality but hollow inside, with missing structural lines.
Panel 3: Beside the mirror, two offset light bars show confidence high and result lower, without using humiliating imagery.
Panel 4: Mona lifts her lantern closer to the mirror with a steady expression, choosing to inspect the gap.
Text layout: reserve a small explanation box near the light bars; avoid overloading the mirror.
Style: reflective mirror, warm compassionate light, psychological clarity.
Negative: horror monster, body shaming, ugly caricature, shame scene.
```

**检查点：** 投影要“空心”，不要画成丑化 Mona。

### 第 22 页：幻影掌握

**画面目标：** 粉碎幻影掌握并转成校准记录。

**分镜布局：** 4 格，幻影碎成数据回账本。

```text
PAGE 22, phantom mastery dissolves. Four-panel page. The hollow mirror projection becomes a thin phantom behind Mona. Mona raises her lantern; warm light passes through the phantom and reveals missing structure lines. The phantom dissolves into star particles that flow back into the evidence ledger as calibration records. Mona's sketch-line arm gains a small patch of color. Style: poetic but precise, warm gold particles, no combat violence. Negative: monster fight, shame, dramatic explosion.

Panel 1: The hollow projection slips out of the mirror as a thin translucent phantom behind Mona.
Panel 2: Mona turns and raises her lantern; warm light passes through the phantom instead of attacking it.
Panel 3: Missing structure lines inside the phantom become visible, then dissolve into ordered star particles.
Panel 4: The particles flow back into a calibration ledger; a small patch of Mona's sketch-line arm becomes colored.
Text layout: leave a small caption area for "phantom mastery becomes calibration evidence".
Style: poetic but precise, warm gold particles, no combat violence.
Negative: monster fight, shame, dramatic explosion, ugly ghost.
```

**检查点：** 幻影不是敌人尸体，而是回到账本的数据。

### 第 23 页：第一张诊断单

**画面目标：** 诊断单显示根源路径，而不是简单对错。

**分镜布局：** 5 格，打印诊断单并显示概念路径。

```text
PAGE 23, first diagnosis sheet. Five-panel page. A long luminous diagnosis sheet prints from the court side. It shows an abstract concept path with three nodes: Ownership, Borrowing, Lifetime as icons. Borrowing foundation glows red, Lifetime tower glows yellow. Mona holds the sheet under her lantern and sees the root cause path. Style: clear diagnostic map, elegant paper-light hybrid, warm focused expression. Negative: red X wrong answer sheet, dense text, grade report.

Panel 1: A slim printer slot in the evidence court emits a long luminous diagnosis sheet.
Panel 2: Close-up of the sheet as an abstract path map, not a grade report.
Panel 3: Three icon nodes appear: ownership as a resource token, borrowing as a bridge, lifetime as a tower.
Panel 4: The borrowing foundation node glows red while the lifetime tower glows yellow, showing root focus.
Panel 5: Mona holds the sheet under her lantern, seeing a cause path instead of a simple wrong mark.
Text layout: leave node labels for post-production; avoid dense generated text.
Style: clear diagnostic map, elegant paper-light hybrid, warm focused expression.
Negative: red X wrong answer sheet, dense text, grade report, shame report.
```

**检查点：** 诊断单应像概念路径，不像考试成绩单。

### 第 24 页：通往知识地铁

**画面目标：** 转场到第 25 页知识地铁。

**分镜布局：** 4 格，法庭后墙打开成图谱轨道。

```text
PAGE 24, passage to the knowledge subway. Four-panel page. The evidence court back wall opens into a star-lit underground rail map. Concept stations glow in the distance. Doraemon wears a small conductor cap and blows a tiny whistle. Mona folds the diagnosis sheet, lifts her lantern, and steps toward the tracks. Style: cinematic transition, luminous rails, deep blue tunnel, anticipation. Negative: ordinary subway, crowded station, goofy train gag.

Panel 1: The back wall of the evidence court splits open along thin constellation seams.
Panel 2: Beyond it is a star-lit underground network, with concept stations glowing like crystal nodes.
Panel 3: Doraemon wears a tiny conductor cap and points down the rail, playful but restrained.
Panel 4: Mona folds the diagnosis sheet, raises her lantern, and steps toward the knowledge subway.
Text layout: leave a transition caption area; keep the tunnel visually clean.
Style: cinematic transition, luminous rails, deep blue tunnel, anticipation.
Negative: ordinary subway, crowded station, goofy train gag, cluttered signs.
```

**检查点：** 轨道必须像知识图谱，不只是普通地铁。

## 7. 第三章逐页提示词（25-36 页）

### 第 25 页：进入知识地铁

**画面目标：** 图谱诊断给出候选路线，Mona 学会读结构。

**分镜布局：** 4 格，三维地铁图 + 3 个路线候选。

```text
PAGE 25, entering the knowledge subway. Four-panel page. Mona and Doraemon stand on a floating platform above a 3D luminous metro network, each station shaped like a crystal concept building. A diagnosis sheet shows a highlighted path. A system panel offers three route options: challenge tower, return to foundation, compare confusing fork; one route glows warm gold as recommended. Style: elegant 3D knowledge map, clear route options, premium sci-fi manga. Negative: single forced arrow, ordinary subway clutter.

Panel 1: Wide shot of Mona and Doraemon on a floating platform above a three-dimensional luminous metro network; each station is a crystal concept building.
Panel 2: Mona looks down at the diagnosis sheet from page 23; the sheet path aligns with tracks below.
Panel 3: A transparent system panel offers three route cards: challenge tower, return to foundation, compare confusing fork.
Panel 4: The recommended route glows warm gold, while Mona's lantern lights the reason line instead of a forced command arrow.
Text layout: reserve three route-card label boxes for later typography.
Style: elegant 3D knowledge map, clear route options, premium sci-fi manga.
Negative: single forced arrow, ordinary subway clutter, random route maze, crowded station.
```

**检查点：** 必须有 3 个路线候选。

### 第 26 页：前置关系不是惩罚

**画面目标：** 前置关系是承重结构，不是惩罚性退回。

**分镜布局：** 4 格，高塔、地基、透视望远镜。

```text
PAGE 26, prerequisite is not punishment. Four-panel page. Mona looks toward a tall Lifetime tower with a red glowing foundation pillar. Doraemon uses a transparent telescope gadget showing a prerequisite tag. A side view reveals the tower depends on the foundation station. Mona turns lantern toward the foundation path. Style: structural metaphor, tower and foundation, calm correction. Negative: punishment scene, failure shame, battle tower.

Panel 1: Mona looks toward a tall crystal Lifetime tower, beautiful but unstable, with a red glowing foundation pillar underneath.
Panel 2: Doraemon raises a transparent telescope gadget; through it, the foundation structure becomes visible.
Panel 3: Cutaway side view: the tower's upper floors depend on a lower foundation station, shown as a load-bearing path.
Panel 4: Mona calmly turns her lantern toward the foundation path, accepting the structural route.
Text layout: leave one small technical label area near the telescope view.
Style: structural metaphor, tower and foundation, calm correction.
Negative: punishment scene, failure shame, battle tower, collapsed disaster.
```

**检查点：** 前置缺口是诊断焦点，不是失败标签。

### 第 27 页：地基站的旧记录

**画面目标：** 通过旧 attempts 说明“见过”不等于“深度掌握”。

**分镜布局：** 4 格，记录墙 + 两条历史胶片。

```text
PAGE 27, old records at the foundation station. Four-panel page. Inside a crystal foundation station, past attempts appear as film strips on the wall. One strip shows high confidence with low final result; another shows recall success but explain depth not passed. Mona studies the records under her lantern. Style: quiet archive, evidence film strips, reflective mood. Negative: messy file room, exam shame.

Panel 1: Mona and Doraemon arrive inside the foundation station; the walls are transparent archives of past attempts.
Panel 2: First film strip shows a confidence icon glowing high while the final result icon is lower.
Panel 3: Second film strip shows recall success, but the explain-depth gate remains dim.
Panel 4: Mona studies both records under the lantern, realizing the concept was only shallowly touched.
Text layout: leave small icon labels only; do not generate long score text.
Style: quiet archive, evidence film strips, reflective mood.
Negative: messy file room, exam shame, angry teacher.
```

**检查点：** 表现“浅层见过”，不是完全没学。

### 第 28 页：岔路出现

**画面目标：** 用相似岔路表现概念边界模糊。

**分镜布局：** 4 格，两个路牌、重叠影子、灯笼闪烁。

```text
PAGE 28, the fork appears. Four-panel page. Behind the foundation station, two nearly identical luminous roads split apart. Road signs symbolize String and &str. Their shadows overlap and whisper "almost the same" as abstract speech shapes. Mona's lantern flickers slightly. Style: subtle ambiguity, dark fork, clean symbolic roads. Negative: horror monsters, unreadable sign clutter.

Panel 1: Behind the foundation station, the track splits into two nearly identical roads.
Panel 2: Close-up of two clean road signs, one for String and one for &str, with minimal text space for post-production.
Panel 3: The two signs cast overlapping shadows that look almost identical, whispering as abstract speech shapes.
Panel 4: Mona pauses; her lantern flickers slightly, signaling conceptual ambiguity rather than fear.
Text layout: keep signs simple and readable; add exact labels later if needed.
Style: subtle ambiguity, dark fork, clean symbolic roads.
Negative: horror monsters, unreadable sign clutter, chaotic branching maze.
```

**检查点：** 模糊感来自边界不清。

### 第 29 页：confusion edge

**画面目标：** 清楚表现 `confusion` 是一条边。

**分镜布局：** 4 格，手电筒揭示红色桥。

```text
PAGE 29, confusion edge revealed. Four-panel page. Doraemon uses a reveal flashlight gadget. The beam exposes a red bridge between the two similar roads, visually tagged as a confusion edge. Mona sees that the road connection means "easy to confuse", not "same thing". Style: diagnostic flashlight, red bridge, precise graph metaphor. Negative: monster fight, generic danger sign.

Panel 1: Doraemon pulls out a reveal flashlight gadget, aiming it at the space between the two roads.
Panel 2: The flashlight beam exposes a red translucent bridge connecting the two roads.
Panel 3: Close-up: the red bridge is visually different from prerequisite tracks, showing edge type rather than node identity.
Panel 4: Mona's lantern steadies as she understands the connection means "easy to confuse", not "same".
Text layout: leave one short edge-label area on the bridge.
Style: diagnostic flashlight, red bridge, precise graph metaphor.
Negative: monster fight, generic danger sign, red node instead of red edge.
```

**检查点：** `confusion` 是边，不是误解节点。

### 第 30 页：误解不是节点

**画面目标：** 把 misconception 表现成可追踪错误模式。

**分镜布局：** 4 格，记录卡、证据板、引用线。

```text
PAGE 30, misconception as traceable record. Four-panel page. A misconception record card appears with abstract fields: pattern, concept scope, evidence links. The card is pinned to an evidence board, connected by light threads to past answer quotes. Mona points at the card, understanding it as an error pattern, not a city station. Style: evidence board, clean cards, gold citation threads. Negative: fixed monster station, vague red warning.

Panel 1: A transparent misconception record card appears above the fork, not as a station on the map.
Panel 2: The card has clean abstract fields: pattern, concept scope, evidence links, represented by icons.
Panel 3: Gold citation threads connect the card to highlighted lines in Mona's past answers.
Panel 4: Mona points to the card, recognizing it as a traceable error pattern.
Text layout: reserve small field areas; avoid making the model generate exact schema text.
Style: evidence board, clean cards, gold citation threads.
Negative: fixed monster station, vague red warning, free-floating villain with no evidence.
```

**检查点：** 误解是记录/模式。

### 第 31 页：辨析任务启动

**画面目标：** 调度器给出辨析任务候选，Mona 主动选择。

**分镜布局：** 4 格，三张任务卡 + 对照台。

```text
PAGE 31, discrimination task begins. Four-panel page. The scheduler offers three comparison cards: boundary, counterexample, distinguishing cue. Mona chooses boundary plus counterexample. Doraemon moves the two concept roads onto a comparison table. Style: clear task cards, elegant comparison table, Mona decisive. Negative: random quiz, one forced option.

Panel 1: The scheduler presents three elegant comparison task cards above the fork.
Panel 2: Cards use icons: boundary line, counterexample spark, distinguishing cue magnifier.
Panel 3: Mona selects boundary and counterexample cards, her lantern highlighting the choice.
Panel 4: Doraemon transforms the two roads into a clean comparison table for inspection.
Text layout: keep card labels short; add final Chinese text later.
Style: clear task cards, elegant comparison table, Mona decisive.
Negative: random quiz, one forced option, cluttered flashcards.
```

**检查点：** 任务选择仍是候选式。

### 第 32 页：拥有者与借用者

**画面目标：** 用金库和钥匙建立 `String` / `&str` 边界。

**分镜布局：** 4 格，对照台两侧 + Mona 画边界。

```text
PAGE 32, owner and borrower. Four-panel page. Under the String road, a glowing vault represents owned data. Under the &str road, a key and library card represent borrowed view. Mona draws a warm boundary line with her light pen, separating the concepts. Style: crisp metaphor, vault versus key, warm line, educational clarity. Negative: literal code wall, combat slash.

Panel 1: On the left side of the comparison table, a glowing vault opens under the String road, representing owned data.
Panel 2: On the right side, a key and library card glow under the &str road, representing borrowed view.
Panel 3: Mona draws a warm gold boundary line between the two with her light pen.
Panel 4: The two roads become visually distinct, no longer overlapping in shadow.
Text layout: leave small object labels for post-production; keep metaphor clear even without text.
Style: crisp metaphor, vault versus key, warm line, educational clarity.
Negative: literal code wall, combat slash, ownership monster.
```

**检查点：** 金库与钥匙对比要清楚。

### 第 33 页：证据输入

**画面目标：** Mona 的辨析解释进入 evidence。

**分镜布局：** 4 格，写解释、入槽、扫描、等待。

```text
PAGE 33, evidence input. Four-panel page. Mona writes her boundary explanation into a glowing ticket. The ticket enters an evidence slot. A strict-citation scan highlights the key sentence. The system shows calm pending review status, no celebration. Style: procedural evidence flow, warm ticket, precise scan. Negative: final score, fireworks.

Panel 1: Mona writes a concise boundary explanation on a glowing ticket with her light pen.
Panel 2: The ticket slides into a dedicated evidence slot near the comparison table.
Panel 3: A citation scan beam highlights the key sentence in the ticket, showing future strict-citation compatibility.
Panel 4: The system displays a calm pending review state; Mona's lantern remains steady.
Text layout: leave one short quote highlight region; do not depend on generated readable answer text.
Style: procedural evidence flow, warm ticket, precise scan.
Negative: final score, fireworks, instant mastery badge.
```

**检查点：** 只到证据入库与等待评分。

### 第 34 页：诊断变清晰

**画面目标：** 表示模糊处被修复了一部分。

**分镜布局：** 4 格，地基颜色转黄、诊断单更新、Mona 总结。

```text
PAGE 34, diagnosis becomes clearer. Four-panel page. The previously red foundation pillar turns yellow, showing partial clarification. The diagnosis sheet updates focus toward boundary comparison. Mona says the system makes ambiguity repairable. Her cloak gains one solid gold constellation line. Style: subtle progress, stable lantern, diagnostic map. Negative: instant full mastery, level-up explosion.

Panel 1: The previously red foundation pillar shifts to yellow, showing partial clarification rather than full mastery.
Panel 2: The diagnosis sheet updates its focus from the high tower toward boundary comparison.
Panel 3: Mona holds the sheet and speaks with calm understanding, not triumph.
Panel 4: A single gold constellation line on her cloak becomes solid, matching the repaired boundary.
Text layout: reserve one speech bubble for Mona's summary.
Style: subtle progress, stable lantern, diagnostic map.
Negative: instant full mastery, level-up explosion, full-color transformation.
```

**检查点：** 进步是变清晰，不是满分通关。

### 第 35 页：类型化超图展开

**画面目标：** 让“线的类型”成为本页主角。

**分镜布局：** 4 格，地图升起、边类型分色、Mona 总结。

```text
PAGE 35, typed hypergraph unfolds. Four-panel page. The city map rises into the air with different colored edge types: prerequisite, confusion, component, instantiation, maps-to as distinct visual lines. Doraemon explains quietly; Mona sees each line's function. Style: grand map reveal, colored edge taxonomy, clean visual hierarchy. Negative: tangled spaghetti graph, unreadable labels.

Panel 1: The city map rises into the air above the platform, expanding from local fork to larger graph.
Panel 2: Different edge types glow in distinct colors and line styles: foundation track, red confusion bridge, component cable, instantiation beam, maps-to arc.
Panel 3: Doraemon points quietly at the edge taxonomy, staying smaller than the map.
Panel 4: Mona stands in front of the map, lantern aligned with the colored lines, understanding that each connection has a different teaching consequence.
Text layout: leave a legend area; add exact edge labels later.
Style: grand map reveal, colored edge taxonomy, clean visual hierarchy.
Negative: tangled spaghetti graph, unreadable labels, one-color network.
```

**检查点：** 线的类型要比数量更重要。

### 第 36 页：章末钩子

**画面目标：** 未见任务引出 MIRT 星图。

**分镜布局：** 4 格，黑立方体、attempts 空、护目镜。

```text
PAGE 36, unknown black cube. Four-panel page. In the deep map, a black cube descends as an unseen task. The panel shows attempts equals zero as a symbolic empty counter. Mona asks how the system judges without history. Doraemon raises pilot goggles, hinting at the star radar. Style: suspenseful but precise, black cube, glowing map. Negative: monster boss fight.

Panel 1: In the deep graph map, a matte black cube descends into a quiet empty station.
Panel 2: A small counter beside it is empty, symbolizing no attempts for this task.
Panel 3: Mona looks at the cube and asks how the system can estimate without history.
Panel 4: Doraemon lifts pilot goggles and opens a radar gadget case, hinting at MIRT.
Text layout: reserve one small empty-counter label area; avoid making the cube monstrous.
Style: suspenseful but precise, black cube, glowing map.
Negative: monster boss fight, evil artifact, random magic cube.
```

**检查点：** 黑立方体是未见任务，不是反派。

## 8. 第四章逐页提示词（37-48 页）

### 第 37 页：潜能力雷达仪

**画面目标：** 建立 MIRT 雷达空间，但避免人格标签化。

**分镜布局：** 4 格，雷达展开、Mona 成为星图坐标点。

```text
PAGE 37, latent ability radar. Four-panel page. Doraemon unfolds a holographic radar. A star-coordinate system appears, with Mona as a warm gold point. Axes are unnamed latent dimensions, not labels like smart or dumb. Mona watches analytically. Style: star radar, transparent coordinates, refined sci-fi. Negative: personality test chart, horoscope vibe.

Panel 1: Doraemon opens the latent ability radar, a circular holographic instrument with clean star-grid lenses.
Panel 2: A transparent coordinate field expands around Mona, deep blue with fine constellation axes.
Panel 3: Mona appears as a warm gold point inside the star map, connected to tiny evidence trails.
Panel 4: Mona studies the axes and sees they are unnamed latent dimensions, not character labels.
Text layout: leave a minimal caption box for "latent ability vector"; do not generate personality labels.
Style: star radar, transparent coordinates, refined sci-fi.
Negative: personality test chart, horoscope vibe, fortune telling, smart/dumb axis.
```

**检查点：** theta 不是人格标签。

### 第 38 页：theta 星图

**画面目标：** theta 来自作答证据的轨迹。

**分镜布局：** 4 格，坐标点、历史证据光迹、线稿眼发光。

```text
PAGE 38, theta star map. Four-panel page. Mona's theta vector appears as coordinates with tiny light trails from past attempts. Evidence traces feed into the coordinate point. Her sketch-line eye briefly glows as if completing coordinates. Style: data-derived star map, warm evidence trails, elegant abstraction. Negative: mystical prophecy, MBTI-style profile.

Panel 1: Close-up of Mona's gold point in the star map, now split into coordinate components.
Panel 2: Tiny light trails from past attempt cards feed into the coordinate point.
Panel 3: Evidence tickets orbit briefly and then become faint coordinate history marks.
Panel 4: Mona's unfinished sketch-line eye glows softly, suggesting coordinates being refined by evidence.
Text layout: reserve small notation areas only; keep the page visual.
Style: data-derived star map, warm evidence trails, elegant abstraction.
Negative: mystical prophecy, MBTI-style profile, fixed destiny aura.
```

**检查点：** theta 来自证据。

### 第 39 页：q 方向箭头

**画面目标：** q 是概念需要调用能力的方向。

**分镜布局：** 4 格，黑立方体裂开成方向箭头。

```text
PAGE 39, q direction arrow. Four-panel page. The black cube opens in radar view and reveals a luminous direction arrow labeled visually as concept loading. The arrow points toward relevant dimensions in Mona's star map. Doraemon gestures to the direction; Mona realizes tasks can change while ability direction remains related. Style: clean vector arrow, star-grid, precise relation. Negative: compass fantasy, random arrows.

Panel 1: The black cube enters the radar field and becomes semi-transparent.
Panel 2: It opens to reveal a clean luminous direction arrow, the q vector, pointing across the star grid.
Panel 3: The arrow aligns with several dimensions near Mona's theta point, showing required latent abilities.
Panel 4: Mona sees that the task surface changed, but the underlying ability direction can still be estimated.
Text layout: keep q as a small symbol; avoid dense mathematical notation here.
Style: clean vector arrow, star-grid, precise relation.
Negative: compass fantasy, random arrows, weapon arrow attack.
```

**检查点：** q 是概念方向，不是题目名字。

### 第 40 页：公式出现

**画面目标：** 用视觉拆开 `q · theta - b - d_t`。

**分镜布局：** 4 格，公式、难度坡、任务门槛、Mona 总结。

```text
PAGE 40, MIRT formula. Four-panel page. The formula p_hat = sigmoid(q dot theta minus b minus d_t) appears as a clean holographic equation. b becomes a slope or hill; d_t becomes task gates of different heights: recall low, transfer high. Mona sees that same concept has different task thresholds. Style: elegant math visualization, readable hierarchy, no dense chalkboard. Negative: wrong formula, cluttered equations.

Panel 1: A clean holographic formula appears above the radar: p_hat = sigmoid(q dot theta minus b minus d_t), expressed with spacious math typography.
Panel 2: The term b becomes a concept difficulty hill the arrow must climb.
Panel 3: The term d_t becomes four task gates of different heights, with transfer visibly higher than recall.
Panel 4: Mona compares two gates for the same concept, realizing task type changes the threshold.
Text layout: formula can be added in post-production; leave clean equation band.
Style: elegant math visualization, readable hierarchy, no dense chalkboard.
Negative: wrong formula, cluttered equations, random algebra storm.
```

**检查点：** 必须包含 `- d_t` 的视觉位置。

### 第 41 页：预测不是判分

**画面目标：** 预测概率不能替代证据法庭。

**分镜布局：** 4 格，概率光环、法庭未开、Mona 留证据。

```text
PAGE 41, prediction is not grading. Four-panel page. The radar emits a probability ring around the black cube. The evidence court remains closed in the background. Doraemon shows that radar gives possibility only. Mona grips her light pen, ready to create evidence. Style: contrast between radar and court, restrained tension. Negative: automatic pass stamp, final grade.

Panel 1: The radar emits a probability ring around the black cube, elegant but translucent.
Panel 2: In the background, the evidence court remains closed and unlit, showing no final judgment yet.
Panel 3: Doraemon places one paw on the radar and points to the closed court, separating prediction from proof.
Panel 4: Mona grips her light pen and steps toward the task, ready to create evidence herself.
Text layout: leave a caption area for "prediction is not proof".
Style: contrast between radar and court, restrained tension.
Negative: automatic pass stamp, final grade, radar approving mastery.
```

**检查点：** 雷达预测不能盖 final 章。

### 第 42 页：迁移试炼

**画面目标：** 迁移是把底层结构用到新场景。

**分镜布局：** 4 格，新题场景、旧结构线、锁孔扣合。

```text
PAGE 42, transfer trial. Four-panel page. The cube transforms into a new scenario, not the previous String/&str fork. Mona draws the ownership-borrowing boundary line into this new context. Old structure connects to new lock, opening a path. Style: transfer metaphor, structure line entering new scene, elegant problem solving. Negative: rote copying, battle slash.

Panel 1: The black cube unfolds into a new unfamiliar scenario, visually different from the previous String/&str fork.
Panel 2: Mona recalls the ownership-borrowing boundary as a warm gold structure line behind her.
Panel 3: She draws that line into the new scenario with her light pen, aligning it with a new lock shape.
Panel 4: The lock clicks open and a narrow path appears, showing transfer without claiming final mastery.
Text layout: no code wall; keep the transfer action visual.
Style: transfer metaphor, structure line entering new scene, elegant problem solving.
Negative: rote copying, battle slash, magic attack.
```

**检查点：** 强调迁移，不是背同一道题。

### 第 43 页：真实记录落盘

**画面目标：** 新迁移尝试回到 evidence / attempt 流程。

**分镜布局：** 4 格，作答、attempt、provisional、final 远处排队。

```text
PAGE 43, record lands. Four-panel page. Mona's response becomes an attempt card and evidence ticket. A provisional stamp lands, while a final grading cursor waits far away in the court queue. Style: procedural record creation, layered cards, calm queue. Negative: immediate final score, celebration.

Panel 1: Mona writes the transfer answer on a new glowing ticket.
Panel 2: The answer splits into an evidence ticket and attempt record card, aligned with the same pipeline from chapter two.
Panel 3: A provisional stamp lands on the local state panel.
Panel 4: Far in the background, a final grading cursor waits near the court queue, keeping the asynchronous flow visible.
Text layout: reserve a small pipeline caption; avoid final score.
Style: procedural record creation, layered cards, calm queue.
Negative: immediate final score, celebration, skipped evidence.
```

**检查点：** final 仍在后面。

### 第 44 页：BKT-MIRT 天平

**画面目标：** 用天平表现先验与真实记录。

**分镜布局：** 4 格，左 MIRT、右 BKT、少记录偏左。

```text
PAGE 44, BKT MIRT balance scale. Four-panel page. Doraemon displays a refined balance scale: left plate holds MIRT star prediction, right plate holds BKT evidence ledger. With few attempts, the left plate glows stronger. Mona observes the balance. Style: elegant scale, star map versus ledger, warm gold contrast. Negative: biased scale, mystical fate.

Panel 1: Doraemon unfolds a delicate balance scale gadget in the radar chamber.
Panel 2: The left plate holds a small star map prediction, representing MIRT prior.
Panel 3: The right plate holds a thin evidence ledger with only a few attempt tickets, representing sparse BKT evidence.
Panel 4: The left plate glows stronger for now, while Mona observes that this is guidance, not final truth.
Text layout: leave small left/right labels for post-production.
Style: elegant scale, star map versus ledger, warm gold contrast.
Negative: biased scale, mystical fate, judge declaring mastery.
```

**检查点：** 左先验、右证据。

### 第 45 页：证据逐渐接管

**画面目标：** 随着 attempts 增加，BKT 记录更有权重。

**分镜布局：** 4 格，证据票据落入右托盘，天平稳定。

```text
PAGE 45, evidence takes over. Four-panel page. More evidence tickets fall onto the BKT ledger side. The scale gradually stabilizes toward the evidence side. A small lambda curve appears as abstract n increasing. Mona says evidence steers. Style: gradual shift, calm confidence, data accumulation. Negative: sudden full mastery, random math clutter.

Panel 1: Several new evidence tickets fall one by one onto the right ledger plate.
Panel 2: The balance slowly shifts, not abruptly, as the evidence side gains weight.
Panel 3: A small abstract curve shows n increasing and the system trusting local evidence more.
Panel 4: Mona's lantern reflects on the now steadier scale, implying evidence steers the model.
Text layout: keep lambda/n hints minimal; add exact math later if needed.
Style: gradual shift, calm confidence, data accumulation.
Negative: sudden full mastery, random math clutter, fireworks.
```

**检查点：** 证据多时更信真实记录。

### 第 46 页：几何层举手

**画面目标：** 几何相似度可以提议，但不能裁决。

**分镜布局：** 4 格，几何小助手递报告，法庭标 proposal only。

```text
PAGE 46, geometry layer raises hand. Four-panel page. Small geometric block assistants bring a similarity report with high visual resemblance. They politely raise hands before the court. The evidence court sign says proposal only. Doraemon taps gavel, showing appearance is not truth. Style: cute but restrained geometry assistants, formal proposal moment. Negative: geometry decides truth, slapstick crowd.

Panel 1: Small geometric block assistants roll in a similarity report, showing two concepts with high visual resemblance.
Panel 2: They raise hands politely before the evidence court, asking to propose a candidate.
Panel 3: The court displays a "proposal only" symbol; the mastery dashboard stays locked.
Panel 4: Doraemon taps the gavel lightly while Mona watches, understanding appearance is not truth.
Text layout: leave a report title area; avoid dense similarity numbers.
Style: cute but restrained geometry assistants, formal proposal moment.
Negative: geometry decides truth, slapstick crowd, geometry changing state.
```

**检查点：** 几何只能提议。

### 第 47 页：结构审查

**画面目标：** 结构和边类型负责审查几何候选。

**分镜布局：** 4 格，两张 2-hop 图并排，对齐与划掉。

```text
PAGE 47, structural review. Four-panel page. Two concept neighborhoods unfold as 2-hop graphs. Matching edge types glow; mismatched edges fade. Mona watches similar-looking lines being accepted or rejected by structure. Style: clean graph comparison, edge-type matching, precise visual audit. Negative: tangled graph, all lines accepted.

Panel 1: Two concept neighborhoods unfold side by side as clean 2-hop graph diagrams.
Panel 2: The system pairs similar nodes with thin gold lines.
Panel 3: Matching edge types glow brightly, while mismatched edges fade or are crossed out.
Panel 4: Mona watches the review result and sees that structural evidence filters visual resemblance.
Text layout: reserve a small legend area; no spaghetti graph.
Style: clean graph comparison, edge-type matching, precise visual audit.
Negative: tangled graph, all lines accepted, random network noise.
```

**检查点：** 审的是结构与边类型。

### 第 48 页：章末转夜

**画面目标：** 白天学习结束，夜间校准开始。

**分镜布局：** 4 格，城市高点、日间事件封存、深蓝夜班。

```text
PAGE 48, day turns into consolidation night. Four-panel page. Mona stands on a high platform; her body star map overlaps with the city graph below. The city lights turn deep blue. A system notice shows day events sealed and residual sorting ready. Doraemon whispers that learning happens by day, calibration by night. Style: grand twilight transition, deep blue glow, quiet machinery. Negative: spooky night, shutdown panic.

Panel 1: Mona stands on a high platform; her body star map overlays the city graph below.
Panel 2: The day's evidence tickets and attempts are sealed into orderly light capsules.
Panel 3: The city shifts into deep blue night mode; underground calibration machinery begins to glow.
Panel 4: Doraemon speaks quietly beside Mona, pointing toward the midnight calibration room.
Text layout: leave a small system notice area; keep the mood calm.
Style: grand twilight transition, deep blue glow, quiet machinery.
Negative: spooky night, shutdown panic, haunted city.
```

**检查点：** 夜晚是校准，不是恐怖。

## 9. 第五章逐页提示词（49-60 页）

### 第 49 页：夜班不是魔法

**画面目标：** 夜间巩固是工程夜班，不是偷偷变魔法。

**分镜布局：** 4 格，闭馆、门牌、Mona 提问、哆啦A梦纠偏。

```text
PAGE 49, night shift is not magic. Four-panel page. Polaris Core closes busy halls and lights a deep underground calibration room. Mona asks if the system secretly gets smarter while she sleeps. Doraemon points to a door labeled Nightly Consolidation, calm and serious. Style: quiet engineering night shift, blue-gold lighting. Negative: magical self-evolution, secret cheating.

Panel 1: The busy scheduling halls of Polaris Core dim down, while a deep underground calibration corridor lights up.
Panel 2: A heavy glass door glows with the visual idea of Nightly Consolidation; tools behind it look like engineering equipment, not wizard objects.
Panel 3: Mona asks whether the system secretly gets smarter while she sleeps, her lantern soft and questioning.
Panel 4: Doraemon points to the door and a rule panel, showing that the night shift organizes evidence but cannot secretly lower standards.
Text layout: reserve one small door label area; avoid dense technical text.
Style: quiet engineering night shift, blue-gold lighting.
Negative: magical self-evolution, secret cheating, wizard laboratory.
```

**检查点：** 夜间整理证据，不偷改规则。

### 第 50 页：残差碎片

**画面目标：** 残差是预测和现实之间的差。

**分镜布局：** 4 格，红蓝碎片、Mona 观察、缝隙隐喻。

```text
PAGE 50, residual fragments. Four-panel page. Red and blue glowing fragments lie on the calibration room floor. Red means predicted correct but wrong; blue means predicted wrong but correct, shown by icons not text. Mona studies the gap between model and reality. Style: abstract residual shards, clean floor grid, quiet analysis. Negative: broken glass danger, random crystals.

Panel 1: The calibration room floor is a clean grid scattered with red and blue glowing fragments.
Panel 2: A red fragment shows an icon of a confident prediction missing the mark.
Panel 3: A blue fragment shows an icon of an underestimated answer succeeding.
Panel 4: Mona kneels and holds one fragment near her lantern, seeing it as a gap between model and reality.
Text layout: use icon-based meaning; add exact labels later.
Style: abstract residual shards, clean floor grid, quiet analysis.
Negative: broken glass danger, random crystals, disaster debris.
```

**检查点：** 残差是模型与现实缝隙。

### 第 51 页：按周聚合

**画面目标：** 残差按时间聚合，相关模式浮现。

**分镜布局：** 4 格，周抽屉、概念行、同步波形。

```text
PAGE 51, weekly aggregation. Four-panel page. Residual fragments flow into transparent drawers arranged by week. Concept rows show small wave patterns. Several waves rise and fall together. Mona notices repeated shared error patterns. Style: data archive, weekly drawers, synchronized light curves. Negative: messy spreadsheet, dense text.

Panel 1: Residual fragments flow into transparent drawers arranged along a weekly timeline.
Panel 2: Each concept becomes a neat row with a small glowing residual wave.
Panel 3: Several rows pulse together, their waves rising and falling in sync.
Panel 4: Mona traces the synchronized lines with her lantern, realizing repeated shared errors may reveal a missing layer.
Text layout: keep drawers mostly visual; avoid spreadsheet text.
Style: data archive, weekly drawers, synchronized light curves.
Negative: messy spreadsheet, dense text, random stock chart.
```

**检查点：** 相关性来自时间聚合。

### 第 52 页：候选新维度

**画面目标：** 候选维度诞生，但仍只是 proposal。

**分镜布局：** 4 格，残差线编织、proposal 标签、哆啦A梦拦住。

```text
PAGE 52, candidate new dimension. Four-panel page. Synchronized residual lines weave into a new luminous star ring. A clean proposal tag appears, clearly marked as candidate. Doraemon blocks it from rushing into the main network. Style: beautiful but restrained abstraction, proposal label, no acceptance yet. Negative: instant new truth, magic upgrade.

Panel 1: The synchronized residual waves lift out of the archive drawers.
Panel 2: They weave into a delicate luminous star ring, beautiful but unfinished.
Panel 3: A transparent "proposal/candidate" tag attaches to the ring.
Panel 4: Doraemon gently holds up a gate sign before it can merge with the main network; Mona watches approvingly.
Text layout: leave a clean proposal tag area.
Style: beautiful but restrained abstraction, proposal label, no acceptance yet.
Negative: instant new truth, magic upgrade, core model transformation.
```

**检查点：** 候选不是接纳。

### 第 53 页：当前 v1 的审计仓

**画面目标：** 当前实现只记录候选与审计轨迹。

**分镜布局：** 4 格，透明审计仓、`consolidation_runs` 账本、状态牌。

```text
PAGE 53, current v1 audit chamber. Four-panel page. The candidate star ring is stored in a transparent audit chamber. Outside the chamber, a consolidation_runs ledger glows with status fields. A sign indicates current v1 records candidates and audit trails, not default core model changes. Mona nods. Style: transparent containment, audit ledger, disciplined engineering. Negative: candidate merged into core.

Panel 1: The candidate star ring moves into a transparent audit chamber, not into the city core.
Panel 2: A glowing ledger beside the chamber represents consolidation_runs, with simple status rows and timestamps as icons.
Panel 3: A status plaque visually states current v1 records candidates and audit trails; the main model gate remains closed.
Panel 4: Mona nods, lantern steady, accepting "record first, believe later."
Text layout: reserve a status plaque area; add exact text later.
Style: transparent containment, audit ledger, disciplined engineering.
Negative: candidate merged into core, flashy evolution, unreviewed upgrade.
```

**检查点：** 当前 v1 不默认改核心模型。

### 第 54 页：未来验证门

**画面目标：** 未来阶段必须过留出集验证门。

**分镜布局：** 4 格，远处验证门、门卫、留出集卡片。

```text
PAGE 54, future hold-out validation gate. Four-panel page. A distant steel-glass gate reads as validation gate. A gatekeeper holds future held-out data cards. The candidate star ring must prove prediction improvement before entry. Style: formal validation gate, future data cards, aspirational but strict. Negative: arbitrary gate, fantasy boss door.

Panel 1: In the distance beyond the audit chamber stands a steel-glass validation gate, elegant and imposing.
Panel 2: A calm gatekeeper figure holds a stack of future held-out data cards.
Panel 3: The candidate star ring projects a trial prediction toward those cards, but the gate remains closed.
Panel 4: Mona sees that future acceptance depends on prediction improvement, not beauty.
Text layout: leave a small gate-label area; keep it as future target.
Style: formal validation gate, future data cards, aspirational but strict.
Negative: arbitrary gate, fantasy boss door, easy pass.
```

**检查点：** 这是未来目标/验证门。

### 第 55 页：失败也有价值

**画面目标：** 驳回候选也要留下原因。

**分镜布局：** 4 格，候选未通过、归档、原因卡、Mona 理解。

```text
PAGE 55, rejected candidate archived. Four-panel page. A candidate fails validation and is gently archived into a transparent rejected drawer with reason tags. Mona sees that failure stays auditable. Style: calm archive, clear rejected status, no shame. Negative: kicking, trashing, destroying evidence.

Panel 1: A candidate star ring does not open the validation gate; its light dims gently.
Panel 2: Instead of being destroyed, the candidate is placed into a transparent rejected archive drawer.
Panel 3: Small reason tags attach to the archive card, represented by clean icons.
Panel 4: Mona reads the archive calmly, seeing that a failed abstraction still improves auditability.
Text layout: leave reason-tag spaces; no dense explanation.
Style: calm archive, clear rejected status, no shame.
Negative: kicking, trashing, destroying evidence, angry gatekeeper.
```

**检查点：** 驳回也留审计记录。

### 第 56 页：参数档案馆

**画面目标：** A/B/C 参数分类可视化。

**分镜布局：** 4 格，档案馆全景 + 三类抽屉。

```text
PAGE 56, parameter archive. Four-panel page. Mona and Doraemon enter a huge wall of drawers divided into A, B, C sections. A is red sealed governance, B is yellow empirical defaults, C is blue online fitting. Style: grand archive wall, color-coded drawers, precise labels as icons. Negative: chaotic library, magic scrolls.

Panel 1: Mona and Doraemon enter a grand parameter archive, a wall of precise drawers stretching upward.
Panel 2: A section glows red and sealed, representing structure/governance parameters.
Panel 3: B section glows yellow, representing empirical defaults that can be taken over by data.
Panel 4: C section glows blue with flowing light, representing online fitted quantities.
Text layout: reserve A/B/C symbols on section headers.
Style: grand archive wall, color-coded drawers, precise labels as icons.
Negative: chaotic library, magic scrolls, random knobs.
```

**检查点：** A/B/C 三类要分色。

### 第 57 页：不能偷偷改及格线

**画面目标：** A 类治理参数锁住。

**分镜布局：** 4 格，拉抽屉、红锁、Doraemon 解释、Mona 接受。

```text
PAGE 57, cannot secretly lower standards. Four-panel page. Mona tries to open the grade quote minimum drawer in A section; it remains locked with a red governance seal. Doraemon explains protection through gesture. Mona understands this is not conservatism but safety. Style: red locked drawer, warm lantern, ethical clarity. Negative: system cheating, lowering bar.

Panel 1: Mona reaches for a red A-section drawer associated with quote minimum governance.
Panel 2: The drawer remains locked, a red seal glowing with "not auto-tunable" symbolism.
Panel 3: Doraemon points from the locked drawer back to the evidence court, showing why the standard protects trust.
Panel 4: Mona lowers her hand and smiles slightly, understanding this is safety, not stubbornness.
Text layout: keep drawer label minimal; add exact parameter name later if needed.
Style: red locked drawer, warm lantern, ethical clarity.
Negative: system cheating, lowering bar, villain lock.
```

**检查点：** A 类锁住。

### 第 58 页：B 类可以学

**画面目标：** B 类参数通过重放被数据接管。

**分镜布局：** 4 格，黄抽屉、重放轨道、候选参数比较。

```text
PAGE 58, B-class parameters can be learned. Four-panel page. A yellow B drawer opens, revealing replay tracks of historical attempts. Candidate parameter values run through replay rails and compare outcomes. Mona watches data choose a better default. Style: replay experiment tracks, yellow archive, scientific calm. Negative: arbitrary tuning, magic knobs.

Panel 1: A yellow B-section drawer opens smoothly, revealing a miniature replay laboratory inside.
Panel 2: Historical attempts run along parallel replay rails.
Panel 3: Several candidate parameter tokens travel through the rails and produce comparable outcome lights.
Panel 4: Mona watches the best-supported token glow, seeing data take over an empirical default.
Text layout: leave small comparison lanes; avoid random numbers.
Style: replay experiment tracks, yellow archive, scientific calm.
Negative: arbitrary tuning, magic knobs, untested parameter change.
```

**检查点：** B 类通过重放验证。

### 第 59 页：慢速抽象

**画面目标：** 夜间巩固的克制美感。

**分镜布局：** 4 格，校准室全景、各子系统安静运转、灯笼变稳。

```text
PAGE 59, slow abstraction. Four-panel page. Wide view of calibration room: residual drawers, proposal chamber, validation gate, parameter archive all operating quietly. Mona's lantern stops getting brighter and instead becomes steady. Style: quiet machine poetry, disciplined intelligence, deep blue night. Negative: fireworks, instant evolution.

Panel 1: Wide view of the midnight calibration room, showing residual drawers, proposal chamber, validation gate, and parameter archive in one composition.
Panel 2: Each station runs quietly with small precise light motions, no spectacle.
Panel 3: Mona stands in the middle; her lantern flame becomes steadier rather than brighter.
Panel 4: Doraemon closes his pocket for once, letting the room's disciplined machinery speak for itself.
Text layout: reserve one calm caption area for "slow abstraction".
Style: quiet machine poetry, disciplined intelligence, deep blue night.
Negative: fireworks, instant evolution, flashy upgrade ceremony.
```

**检查点：** 灯笼变稳。

### 第 60 页：清晨

**画面目标：** 夜班结束，地图更清楚但不夸张。

**分镜布局：** 4 格，开馆、注释更新、Mona 保留线稿、列车汽笛。

```text
PAGE 60, morning after consolidation. Four-panel page. Polaris Core reopens at dawn. The map is not dramatically transformed, but annotations are cleaner and a few paths clearer. Mona's cloak constellation lines are more complete while some sketch lines remain. A distant train whistle hints at the next chapter. Style: gentle dawn, refined progress, quiet optimism. Negative: total transformation, all line art gone.

Panel 1: Dawn light enters Polaris Core as the night calibration room powers down.
Panel 2: The city map reappears with cleaner annotations and a few clarified paths, not a whole new city.
Panel 3: Mona's cloak constellation lines are more complete, but hair edge and one eye still retain unfinished sketch-line details.
Panel 4: A distant train whistle and station light introduce the next chapter.
Text layout: leave transition caption area; no victory banner.
Style: gentle dawn, refined progress, quiet optimism.
Negative: total transformation, all line art gone, triumphant final ending.
```

**检查点：** 保留未完成线稿。

## 10. 第六章逐页提示词（61-70 页）

### 第 61 页：外部导师入口

**画面目标：** MCP 是有权限边界的入口。

**分镜布局：** 4 格，侧门打开、导师进站、通行证、Mona 观察。

```text
PAGE 61, MCP external tutor entrance. Four-panel page. A side gate opens in Polaris Core. Various external tutor figures enter with passes: code assistant, language coach, research tutor. They carry tools, not crowns. Mona observes the boundary. Style: diverse tutors, formal access gate, elegant station mood. Negative: tutors taking over core dashboard.

Panel 1: A side gate in Polaris Core opens like a formal station entrance, marked visually as an external interface.
Panel 2: Different tutor figures enter: code assistant, language coach, research tutor, each with distinct tools.
Panel 3: Each tutor carries a pass card, not a crown or master key.
Panel 4: Mona watches the gate boundary, lantern reflecting on the access rules.
Text layout: reserve small pass-card areas; avoid crowded labels.
Style: diverse tutors, formal access gate, elegant station mood.
Negative: tutors taking over core dashboard, chaotic crowd, celebrity entrance.
```

**检查点：** 外部导师有通行证，没有王冠。

### 第 62 页：同一事实源

**画面目标：** 不同导师围绕同一本账工作。

**分镜布局：** 4 格，提交 evidence、读取 status、获取 instruction、连接核心账本。

```text
PAGE 62, one source of truth. Four-panel page. One tutor submits Mona's answer into an evidence slot. Another reads a glowing status resource. A third receives a teaching instruction card. All connect to the same central ledger. Style: multi-tool harmony, single evidence core, clean data lines. Negative: separate inconsistent dashboards.

Panel 1: A code tutor submits Mona's answer into the same evidence intake slot used before.
Panel 2: A language coach reads a glowing status resource panel connected to the central ledger.
Panel 3: A research tutor receives a teaching instruction card from Polaris Core.
Panel 4: All three data lines converge on one central evidence ledger, with Mona standing beside it.
Text layout: leave small tool labels for post-production.
Style: multi-tool harmony, single evidence core, clean data lines.
Negative: separate inconsistent dashboards, tutors writing their own mastery scores.
```

**检查点：** 多工具围着同一事实源。

### 第 63 页：不能直接改掌握度

**画面目标：** 外部 AI 的判断只能作为 evidence。

**分镜布局：** 4 格，贴纸被拦、重定向 evidence。

```text
PAGE 63, cannot directly edit mastery. Four-panel page. An enthusiastic external tutor tries to place a mastered sticker on the dashboard. A transparent system gate blocks it. The sticker is redirected into evidence intake. Doraemon explains by pointing to the evidence path. Style: firm but friendly boundary, clean redirect arrow. Negative: external AI changing gauges.

Panel 1: An enthusiastic external tutor holds a "mastered" sticker near the mastery dashboard.
Panel 2: A transparent system gate gently blocks the sticker before it touches any gauge.
Panel 3: The sticker transforms into an evidence note and is redirected along an intake path.
Panel 4: Doraemon points to the path while Mona nods, seeing judgment becomes evidence, not authority.
Text layout: reserve a small boundary-rule caption area.
Style: firm but friendly boundary, clean redirect arrow.
Negative: external AI changing gauges, aggressive rejection, broken dashboard.
```

**检查点：** 外部判断只能作为 evidence。

### 第 64 页：万国列车进站

**画面目标：** 多领域进入，但 Rust 只是第一节车厢。

**分镜布局：** 4 格，列车进站、不同领域车厢、标准打包布。

```text
PAGE 64, world train arrives. Four-panel page. A huge elegant train enters the central station. Cars visually represent Rust, English, finance, medicine, Japanese. Mona asks why there are other domains. Doraemon holds the standard wrapping cloth. Style: grand station, multi-domain train, deep blue and warm station lights. Negative: carnival train, chaotic signage.

Panel 1: A grand elegant train enters the central station of Polaris Core.
Panel 2: Different train cars visually represent Rust, English, finance, medicine, and Japanese through icons and architecture.
Panel 3: Mona looks surprised, comparing the new cars to the Rust examples she has seen.
Panel 4: Doraemon holds up the standard wrapping cloth, ready to show that every domain must become a pack.
Text layout: keep domain icons mostly visual; add labels later if needed.
Style: grand station, multi-domain train, deep blue and warm station lights.
Negative: carnival train, chaotic signage, random travel poster.
```

**检查点：** Rust 只是第一节车厢。

### 第 65 页：课程变成 pack

**画面目标：** 新领域也必须通过 pack 协议。

**分镜布局：** 4 格，散乱内容、validator 拒绝、模块化。

```text
PAGE 65, course becomes pack. Four-panel page. A train car unloads scattered materials. Validator gate refuses raw chaos. Doraemon's standard cloth transforms them into pack module cards. Mona sees that content differs but interface is standard. Style: station customs, clean transformation, module cards. Negative: direct content dumping into core.

Panel 1: A finance or language train car unloads scattered materials: books, exercises, notes, examples.
Panel 2: A validator gate blocks the raw pile with an orderly red warning, not hostility.
Panel 3: Doraemon's standard cloth wraps the materials and transforms them into clean pack module cards.
Panel 4: Mona compares the new modules to the earlier Rust pack and sees the same interface pattern.
Text layout: reserve module-card labels for post-production.
Style: station customs, clean transformation, module cards.
Negative: direct content dumping into core, trash pile, arbitrary magic.
```

**检查点：** 接入协议是重点。

### 第 66 页：协议缺口

**画面目标：** `ingest.toml` 仍是未来协议缺口。

**分镜布局：** 4 格，半透明模块、未来票标记、不假装完成。

```text
PAGE 66, protocol gap. Four-panel page. A semi-transparent ingest.toml module floats unfinished beside the pack. It is outlined in light but not fully solid. Mona writes a small future-ticket note beside it. Style: transparent unfinished module, respectful future work marker, no broken error. Negative: pretending complete, red failure alarm.

Panel 1: Beside the complete pack module cards, a semi-transparent ingest module outline floats unfinished.
Panel 2: The module is outlined in blue light but not solid, showing planned protocol work.
Panel 3: Mona writes a small future-ticket marker beside it, not a fix inside the current script.
Panel 4: The system shelves it in a planned-work slot, clean and intentional.
Text layout: reserve a small future-work label area.
Style: transparent unfinished module, respectful future work marker, no broken error.
Negative: pretending complete, red failure alarm, cracked broken module.
```

**检查点：** `ingest.toml` 是后续稳定化，不要画成已完成。

### 第 67 页：多 pack 的潜在映射

**画面目标：** 跨域前需要 latent dimension 映射。

**分镜布局：** 4 格，多领域星图、坐标错位、映射网格。

```text
PAGE 67, multi-pack latent mapping. Four-panel page. Multiple domain maps light up at once. Their star-coordinate systems partly overlap and look misaligned. A mapping grid appears between them. Mona says cross-domain requires clear coordinate mapping first. Style: multiple star maps, alignment grid, analytical mood. Negative: instant perfect cross-domain transfer.

Panel 1: Several domain districts light up at once, each with its own star-coordinate overlay.
Panel 2: The overlays partially overlap but are slightly misaligned, showing a coordination problem.
Panel 3: A mapping grid appears between the domains, with careful alignment markers.
Panel 4: Mona studies the grid with her lantern, understanding that cross-domain transfer needs a clear coordinate system first.
Text layout: keep mapping labels minimal; add exact wording later.
Style: multiple star maps, alignment grid, analytical mood.
Negative: instant perfect cross-domain transfer, mystical universal truth.
```

**检查点：** 多 pack 前要说清坐标。

### 第 68 页：引擎大于内容

**画面目标：** 同一引擎支撑不同课程街区。

**分镜布局：** 4 格，城市剖面、不同街区、同一底层管线。

```text
PAGE 68, engine greater than content. Four-panel page. Wide city view: different domain districts have distinct architectural skins, but underneath all share the same evidence, attempt, fold, diagnosis pipes. Doraemon points to the shared underlayer. Mona sees the loop. Style: cutaway city infrastructure, elegant systems thinking. Negative: one giant course catalog.

Panel 1: Wide view of several domain districts with distinct visual skins.
Panel 2: The city becomes a cutaway showing shared underlayer pipelines beneath all districts.
Panel 3: The shared pipes are labeled visually by icons: evidence, attempt, fold, diagnosis.
Panel 4: Doraemon points to the underlayer while Mona sees that the engine, not content quantity, is the core.
Text layout: use icon labels, not long text.
Style: cutaway city infrastructure, elegant systems thinking.
Negative: one giant course catalog, marketing brochure layout.
```

**检查点：** 强调学习机制，不是课程数量。

### 第 69 页：跨域微光

**画面目标：** 跨域共鸣只能作为候选。

**分镜布局：** 4 格，星图细线、candidate 标记、验证符号。

```text
PAGE 69, cross-domain glimmer. Four-panel page. In the high MIRT star sky, a thin candidate line connects programming abstraction and language structure constellations. Evidence court and validation gate glow in the distance, reminding it is candidate only. Style: subtle star connection, validation symbols, poetic restraint. Negative: declaring truth, mystical destiny.

Panel 1: High above the city, programming abstraction and language structure appear as distant constellations.
Panel 2: A thin luminous line reaches between them, beautiful but faint.
Panel 3: A candidate tag attaches to the line, while evidence court and validation gate glow far below.
Panel 4: Mona looks up, impressed but cautious, understanding the line is a hypothesis.
Text layout: leave small candidate tag space.
Style: subtle star connection, validation symbols, poetic restraint.
Negative: declaring truth, mystical destiny, cosmic prophecy.
```

**检查点：** 连线标候选，不是真理。

### 第 70 页：回到主命题

**画面目标：** 全书系统结构总复盘。

**分镜布局：** 4 格，城市四层剖面、三束光汇合。

```text
PAGE 70, return to the core proposition. Four-panel page. Mona unfolds the full Polaris Core city map: underground event replay gears, ground graph network, midair evidence court, high star radar. Three beams merge into the loop of verify, locate, remediate. Style: grand system recap, layered city cutaway, warm lantern center. Negative: vague inspirational poster.

Panel 1: Mona unfolds the full Polaris Core city map like a luminous scroll.
Panel 2: The city appears as layered cutaway: underground event replay gears, ground graph network, midair evidence court, high star radar.
Panel 3: Three beams of light connect the layers into the loop: verify real understanding, locate ambiguity, remediate precisely.
Panel 4: Mona stands at the center with her lantern, now steady and warm.
Text layout: reserve three large clean phrase areas for the core loop.
Style: grand system recap, layered city cutaway, warm lantern center.
Negative: vague inspirational poster, generic hero pose, cluttered architecture.
```

**检查点：** 四层结构要清楚。

## 11. 尾声逐页提示词（71-72 页）

### 第 71 页：Mona 的总结

**画面目标：** 情绪收束，保留未完成线稿。

**分镜布局：** 整页大图，Mona 与系统结构同框。

```text
PAGE 71, Mona's final summary. Full-page image. Mona stands inside the city gate, lantern illuminating the clear system structure behind her. She is more complete but still keeps a few unfinished sketch lines at hair, eye, and cloak edge. Doraemon peeks from the side holding a small evidence-first sign; Mona gently lights it with her lantern instead of pushing him away. Style: emotional but restrained finale, elegant anime illustration, warm gold lantern, deep indigo city. Negative: complete magical girl transformation, slapstick hit, childish ending.

Full-page composition: Mona stands just inside the Polaris Core gate, facing the reader at a slight angle.
Background: behind her, the city structure is visible but softened: evidence court, graph map, replay gears, star radar.
Character detail: Mona is mostly colored now, but hair edge, one eye detail, and cloak border retain delicate unfinished sketch lines.
Doraemon detail: Doraemon peeks from one side holding a small sign symbolizing evidence first; he is supportive, not stealing the page.
Lighting: Mona's lantern gently lights both the sign and the path behind her.
Text layout: leave generous empty space for Mona's final poetic line.
Style: emotional but restrained finale, elegant anime illustration, warm gold lantern, deep indigo city.
Negative: complete magical girl transformation, slapstick hit, childish ending, crowded finale.
```

**检查点：** 线稿不能完全消失。

### 第 72 页：封底

**画面目标：** 克制、高级、可排版。

**分镜布局：** 单幅封底，中心 logo + 极细星图线。

```text
PAGE 72, back cover. Minimal deep indigo cover page. Center a refined glowing Polaris Core emblem surrounded by fine constellation lines and subtle circuit traces. No characters or only tiny silhouettes of Mona and Doraemon at the bottom edge if needed. Leave clean typography space for title and tagline. Style: premium sci-fi book cover, quiet, elegant, high contrast. Negative: busy poster, mascot overload, random symbols.

Single image: a minimal deep indigo back cover with a refined glowing Polaris Core emblem in the center.
Background: very fine constellation lines and subtle circuit traces, low contrast, elegant.
Optional character detail: tiny silhouettes of Mona and Doraemon near the lower edge, almost like a signature, not the focus.
Typography space: leave large clean areas for title, tagline, and version note.
Style: premium sci-fi book cover, quiet, elegant, high contrast.
Negative: busy poster, mascot overload, random symbols, cluttered typography.
```

**检查点：** 封底克制，高级，留排版空间。

## 12. 全书制作检查清单

生成第 1-72 页时，每页都要过以下检查：

- Mona 的白发、星图斗篷、暖金灯笼、局部线稿感稳定。
- 哆啦A梦出现时是辅助道具位，不抢主视觉。
- 不依赖模型生成精确中文；文字预留给后期排版。
- 主命题的三个阶段在第 4、70、72 页前后呼应。
- 第 5 页是多维仪表，不是单一分数条。
- 第 10 页和第 25 页是候选式选择，不是唯一命令。
- 第 12 页只到 evidence 和 attempt，不提前进入 final 评分。
- 第 15 页必须表现 strict-citation 约束原文引用。
- 第 30 页必须表现误解是记录/模式，不是图谱节点。
- 第 40 页公式必须包含 `- d_t`。
- 第 53 页必须标明当前 v1 只记录候选和审计轨迹。
- 第 63 页必须表现外部 AI 不能直接改掌握度。
- 第 71 页保留少量未完成线稿，表达持续学习。

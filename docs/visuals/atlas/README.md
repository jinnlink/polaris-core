# Polaris Porcelain Intelligence Atlas

这是 Polaris Core 的本地静态交互式开发者架构图谱。它用于解释项目结构、学习闭环、数据模型、票据进度和验证护栏，不参与产品运行时，也不替代 P17 的 Tauri 学习者工作区。

票据时间线由 `docs/tickets/QUEUE.md` 动态生成；Atlas 不维护第二份完成状态。

## 生成数据

```powershell
python docs\visuals\atlas\scripts\build_atlas_data.py --write
```

## 校验

```powershell
python docs\visuals\atlas\scripts\build_atlas_data.py --check
python docs\visuals\atlas\scripts\validate_atlas.py
```

## 本地预览

```powershell
python -m http.server 4173 -d docs\visuals\atlas
```

打开：

```text
http://localhost:4173/
```

## 设计锁定

- 风格：Polaris Porcelain Intelligence Atlas / 瓷白智能图谱。
- 色彩：Radix Air Sage，高明度瓷白、浅鼠尾草、薄石墨线。
- 动效：Quiet Motion System，背景字符场、路径点亮、连续切换、轻玻璃 Inspector。
- 借鉴小米 MiMo：慢速空间动效和高级留白节奏。
- 不借鉴小米 MiMo：品牌橙、Logo、字形、营销海报结构。

## 回滚

如需移除 Atlas：

```powershell
Remove-Item -Recurse -Force docs\visuals\atlas
```

执行删除前确认目标路径位于 `C:\MyProject\polaris-core`。

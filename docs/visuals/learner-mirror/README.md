# Learner Mirror Static Panel

P07B 静态学习者状态镜子。这个目录是无构建、无网络依赖的静态面板，用 `data/sample.json` 渲染：

- 自信 vs 实际表现曲线
- 相分布条
- 近期断言
- 行动提示

`data/sample.json` 是人工编写的脱敏夹具，不来自真实用户数据库。页面会优先读取 `./data/sample.json`；在某些浏览器直接用 `file://` 打开并阻止本地 `fetch` 时，`app.js` 会使用同结构的内置 synthetic fallback，保证 HTML 仍可直接打开查看。

校验命令：

```powershell
python docs\visuals\learner-mirror\scripts\validate_learner_mirror.py
```

校验脚本只检查静态文件存在、sample JSON 必要字段、脱敏标记、曲线排序、相分布覆盖、断言与断言中的行动提示；不访问网络，不读取或写入真实数据。

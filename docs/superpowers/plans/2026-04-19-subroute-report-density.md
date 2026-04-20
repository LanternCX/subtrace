# 状态：Archive

# Subroute 报告页信息密度调整实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 调整 HTML 报告页布局，提高信息密度，删除重复的地区分组区域。

**Architecture:** 保持当前单文件 HTML 生成为主，在 `src/report.rs` 内收紧布局结构与样式，避免引入新的页面层级。通过现有自动化测试约束输出结构，确保删除重复区域后交互仍然完整。

**Tech Stack:** Rust、内联 HTML/CSS/JavaScript、cargo test

---

### 任务 1：补充页面结构约束测试

**Files:**
- Modify: `src/report.rs`

- [ ] 将现有 HTML 输出测试补充为新的页面结构预期。
- [ ] 明确校验：保留 `graph-root`、`detail-panel`、`地区筛选`。
- [ ] 明确校验：输出不再包含“地区分组”。
- [ ] 明确校验：输出包含新的并排工作区标识和较窄详情区标识。

### 任务 2：调整报告页布局与内容

**Files:**
- Modify: `src/report.rs`

- [ ] 将关系图区和详情区收敛为同一工作区内的左右布局。
- [ ] 压缩关系图容器留白与整体占比。
- [ ] 压缩详情区局部留白，使其更适合作为辅助阅读区。
- [ ] 删除底部“地区分组”区块及其输出内容。
- [ ] 删除只为地区分组服务的生成逻辑，保留详情区所需的对象按钮能力。

### 任务 3：验证输出与回归

**Files:**
- Modify: `docs/superpowers/specs/2026-04-19-subroute-report-density-design.md`
- Modify: `docs/superpowers/plans/2026-04-19-subroute-report-density.md`

- [ ] 运行相关测试，确认新的页面结构断言通过。
- [ ] 运行格式化，确保文件保持一致风格。
- [ ] 将本次 spec 和 plan 状态更新为 `Archive`。

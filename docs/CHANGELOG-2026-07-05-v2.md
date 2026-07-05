# CHANGELOG 2026-07-05 v2

## 1. 修改概述

本次任务实现 Task 11：前端 Query Filter 更新发送到后端引擎。
当用户在 QueryBar 中修改过滤条件时，前端仍会先更新本地 store 的 `filterQuery`，并在存在活动设备时通过 WebSocket 向 engine 发送 `set_filter` 消息。
该变更让后端可以接收过滤查询，从而为后续后端过滤逻辑对接提供前端消息通路。
本次未实现后端搜索、E2E 测试或验收清单，也未改变 SearchBar 行为。
`EngineClient.send` 已满足关闭连接时返回 `false`、发送成功后返回 `true` 的要求，因此未额外修改该文件。

## 2. 需求来源

### 用户需求
Implement Task 11 only in `/home/noah/Codes/PC/AndroidLogcatStudio/.worktrees/als-mvp` on branch `feature/als-mvp`。目标是 Send Query Filter updates from frontend to backend。要求 `EngineClient.send(message)` 在 socket 缺失或非 OPEN 时返回 `false`，发送后返回 `true`；`App.tsx` 在 QueryBar 变化时更新 store 的 `filterQuery`，并在存在 `activeDeviceId` 时发送 `{ type: 'set_filter', deviceId: activeDeviceId, query: next }`；SearchBar 仍只更新 store，不发送 `set_search`。

### 目标结果
QueryBar 的每次输入变更都会同步更新前端状态，并在已有活动设备时把最新过滤查询发送给 engine；SearchBar 行为保持不变；Task 9/10 的连接逻辑和 UI 组成保持不变。

## 3. 修改范围

| 文件 | 类型 | 修改内容 |
|------|------|----------|
| `src/renderer/App.tsx` | 修改 | 新增 `handleFilterChange` 回调，QueryBar 变化时更新 store，并在存在活动设备时通过 `EngineClient.send` 发送 `set_filter` 消息。 |
| `src/renderer/api/engineClient.ts` | 未修改 | 检查后确认 `send` 已在 socket 缺失或非 OPEN 时返回 `false`，发送后返回 `true`，满足需求。 |
| `src/renderer/state/appStore.ts` | 未修改 | 现有 `setFilterQuery` 已满足状态更新需求，无需调整。 |
| `docs/CHANGELOG-2026-07-05-v2.md` | 新增 | 按 dev-change 要求记录本次变更。 |

## 4. 核心实现说明

### 修改前
QueryBar 的 `onChange` 直接绑定 `setFilterQuery`，只更新 Zustand store 中的 `filterQuery`。前端不会在过滤条件变化时向后端 engine 发送过滤更新消息。

### 修改后
`App.tsx` 增加 `handleFilterChange(next)`：
1. 调用 `setFilterQuery(next)` 保持原有本地状态更新。
2. 如果 `activeDeviceId` 存在，则调用 `clientRef.current?.send({ type: 'set_filter', deviceId: activeDeviceId, query: next })` 把过滤条件发送给 engine。
3. QueryBar 的 `onChange` 改为绑定该回调。

### 为什么这样实现
该实现直接复用已有 `EngineClient` 和 `clientRef`，不改变连接建立逻辑，也不引入新的状态层或副作用管理。使用 `useCallback` 保持回调依赖明确，并让 QueryBar 仍通过单一 `onChange` 接口接收更新。

### 为什么没有采用其他方案
未采用 debounce：MVP 要求可以每次变更都发送，避免增加时序复杂度。
未修改 store：发送 engine 消息依赖 `EngineClient` 实例，当前 store 只负责应用状态，保持职责分离更简单。
未实现 SearchBar 后端消息：用户明确要求 SearchBar 仍只更新 store，不发送 `set_search`。

## 5. 关键函数说明

### handleFilterChange
- 作用：处理 QueryBar 过滤查询变化，同时更新本地 store 并按条件通知 engine。
- 输入：`next: string`，用户输入的最新过滤查询。
- 输出：无显式返回值。
- 调用关系：作为 `QueryBar` 的 `onChange` 回调；内部调用 `setFilterQuery` 和 `EngineClient.send`。
- 注意事项：仅在 `activeDeviceId` 存在时发送 `set_filter`；`EngineClient.send` 在 socket 未打开时返回 `false`，此处不抛错、不阻断 UI 更新。

### EngineClient.send
- 作用：向 engine WebSocket 发送客户端协议消息。
- 输入：`ClientMessage`。
- 输出：`boolean`，socket 缺失或非 OPEN 时为 `false`，发送后为 `true`。
- 调用关系：本次由 `App.tsx` 的 `handleFilterChange` 调用。
- 注意事项：该函数本次未修改，检查后确认已有逻辑满足任务要求。

## 6. 配置变更

无配置变更。

## 7. 影响范围分析

### 直接影响
`src/renderer/App.tsx` 中 QueryBar 的变更处理流程。

### 间接影响
后端 engine 会在活动设备存在且 WebSocket 已连接时收到 `set_filter` 客户端消息；后端是否实际应用过滤取决于已有 engine 实现。

### 风险点
过滤输入每次变化都会发送消息，极快输入时可能产生较多 WebSocket 消息；这是 MVP 明确接受的行为。若尚未选中活动设备，消息不会发送，只会更新前端状态。

### 兼容性
保留原有 store 更新逻辑、连接逻辑、SearchBar 行为和 UI 结构。`EngineClient.send` 的关闭 socket 行为已是返回 `false`，不会因本次变更引入关闭连接异常。

## 8. 验证记录

| 验证项 | 结果 | 说明 |
|--------|------|------|
| 编译通过 | 是 | 已运行 `npm run build`，TypeScript 与 Vite 构建通过。 |
| 单元测试通过 | 是 | 已运行 `cargo test -p als-engine`，18 个测试通过；存在既有 warning：`RingBuffer::len` 未使用。 |
| 功能验证通过 | 【未验证】 | 未启动 Electron/浏览器进行交互验证；本任务要求的检查项未包含 E2E。 |
| 回归测试通过 | 【部分验证】 | 通过前端构建与 engine 单元测试；未执行完整端到端回归。 |

## 9. 回滚方案

如出现问题，应回滚以下文件：
- `src/renderer/App.tsx`
- `docs/CHANGELOG-2026-07-05-v2.md`

无需恢复配置文件，因为本次没有配置变更。回滚风险较低，但回滚后 QueryBar 将恢复为仅更新前端 store，不再向 engine 发送 `set_filter` 消息。

## 10. 后续优化建议（可选）

- 后续可在确认后端过滤行为稳定后增加针对 `set_filter` 消息发送的前端单元测试或组件测试。
- 若过滤输入导致消息过多，可在后续非 MVP 阶段添加轻量 debounce 或合并策略。
- 可在后续任务中实现 SearchBar 的后端搜索消息发送，但本次明确不包含该内容。

# CHANGELOG 2026-07-05 v3

## 1. 修改概述

本次任务为 Android Logcat Studio MVP 增加后端全日志搜索结果能力，并将顶部 SearchBar 与引擎的 `set_search` WebSocket 消息打通。
实现后，渲染端搜索输入变化会保留本地 `searchQuery`，同时向当前激活设备发送搜索请求。
引擎收到搜索请求后，会在当前设备缓冲区中按可见日志消息执行大小写不敏感的子串匹配，并返回匹配日志的 `seq` 列表。
该改动解决了搜索框此前只影响前端高亮/展示、没有后端搜索结果状态的问题。
本次没有实现 E2E 测试或验收清单相关内容。

## 2. 需求来源

### 用户需求
Implement Task 12 only in `/home/noah/Codes/PC/AndroidLogcatStudio/.worktrees/als-mvp` on branch `feature/als-mvp`.

目标包括：
- `DeviceContext` 新增 `search_visible_sequences(&self, query: &str) -> Vec<u64>`。
- WebSocket 协议新增并处理 `set_search`，返回 `search_results`。
- `appStore.ts` 新增 `searchMatches` 状态并处理 `search_results`。
- `App.tsx` 中 SearchBar 的 `onChange` 发送 `set_search`，QueryBar 过滤逻辑保持不变。
- 执行 `cargo test -p als-engine` 和 `npm run build`。
- 提交 commit：`feat: add backend full-log search`。

### 目标结果
搜索框输入变化时，前端向后端发送固定搜索选项的 `set_search` 消息；后端在当前日志缓冲区中返回匹配的可见日志序号；前端 store 将匹配序号保存到 `searchMatches`，为后续 UI 使用提供状态基础。

## 3. 修改范围

| 文件 | 类型 | 修改内容 |
|------|------|----------|
| `engine/src/device.rs` | 修改 | 新增 `search_visible_sequences`，按消息文本执行可见日志搜索；新增单元测试覆盖空查询、大小写不敏感和隐藏日志排除。 |
| `engine/src/websocket.rs` | 修改 | `ClientMessage` 新增 `SetSearch`；`ServerMessage` 新增 `SearchResults`；处理 `set_search` 请求并发送匹配序号；新增协议序列化/反序列化测试。 |
| `src/renderer/state/appStore.ts` | 修改 | 新增 `searchMatches` 状态并在 `search_results` 消息到达时更新。 |
| `src/renderer/App.tsx` | 修改 | 新增 SearchBar 变更处理函数，在更新本地搜索查询后向活动设备发送 `set_search`；保留 QueryBar 的过滤消息逻辑。 |
| `docs/CHANGELOG-2026-07-05-v3.md` | 新增 | 记录本次开发变更、验证结果和回滚方案。 |

## 4. 核心实现说明

### 修改前
搜索框仅通过 `setSearchQuery` 更新前端 store 的 `searchQuery`，没有向引擎发送搜索请求。
后端 WebSocket 协议支持设备连接、过滤、统计等消息，但没有 `set_search` 输入或 `search_results` 输出。
`DeviceContext` 只有可见快照能力，没有按查询返回匹配日志序号的后端搜索函数。
前端 store 类型中已有协议层 `search_results` 类型定义，但应用状态没有保存匹配结果。

### 修改后
`DeviceContext::search_visible_sequences` 对空查询直接返回空列表；非空查询会读取当前 ring buffer 中最多 1,000,000 条日志，过滤掉 hidden 日志后，对 `message` 做大小写不敏感子串匹配，并返回匹配日志的 `seq`。
WebSocket `ClientMessage` 新增 `SetSearch { device_id, query, options }`，通过既有 serde 配置保持 `type: set_search` 和 camelCase 字段。
`handle_client_text` 在校验设备 ID 后调用搜索函数，并发送 `ServerMessage::SearchResults { device_id: mock-device, matches }`。
前端 store 新增 `searchMatches: number[]`，收到 `search_results` 后以后端返回结果覆盖当前匹配列表。
`App.tsx` 为 SearchBar 增加专用 `handleSearchChange`，保留本地查询更新，同时向活动设备发送固定选项：`regex: false`、`caseSensitive: false`、`wholeWord: false`。

### 为什么这样实现
该实现直接复用当前设备缓冲区和已有 hidden 标记，能与现有过滤结果保持一致，避免新增索引或缓存层。
WebSocket 协议沿用现有 `rename_all = "snake_case"` 和 `rename_all_fields = "camelCase"`，减少前后端字段映射风险。
前端只保存匹配序号，不改变 LogView 的现有搜索高亮和 QueryBar 的过滤行为，符合“只实现 Task 12”的范围。

### 为什么没有采用其他方案
没有实现正则、大小写敏感或整词搜索，因为本次需求明确要求后端执行大小写不敏感子串搜索，SearchBar 仅发送固定 options。
没有引入全文索引或增量搜索缓存，因为当前目标是 MVP 后端搜索结果，且搜索范围被限制为当前 buffer 最多 1,000,000 条。
没有改造 LogView 消费 `searchMatches`，因为需求只要求 store 处理 `search_results`，未要求改变展示逻辑。
没有实现 E2E 或验收清单，因为用户明确要求不实现。

## 5. 关键函数说明

### `DeviceContext::search_visible_sequences`
- 作用：在当前日志缓冲区中查找消息文本包含查询字符串的可见日志，并返回其 `seq`。
- 输入：`query: &str`，搜索关键字。
- 输出：`Vec<u64>`，匹配日志的序号列表；空查询返回空列表。
- 调用关系：由 `engine/src/websocket.rs` 中 `handle_client_text` 处理 `ClientMessage::SetSearch` 时调用。
- 注意事项：仅搜索 `message` 字段；仅包含 `hidden == false` 的日志；大小写不敏感；当前实现会对每条候选日志消息执行小写转换。

### `handle_client_text` 的 `ClientMessage::SetSearch` 分支
- 作用：接收前端搜索请求，校验 mock 设备 ID，调用设备搜索并返回 `search_results`。
- 输入：WebSocket 文本消息反序列化后的 `ClientMessage::SetSearch`。
- 输出：通过 WebSocket 发送 `ServerMessage::SearchResults`；函数返回连接是否继续。
- 调用关系：由 WebSocket 接收循环在收到文本消息时调用。
- 注意事项：`options` 当前只为协议兼容保留，未参与搜索逻辑；未知设备仍沿用现有 `ensure_mock_device` 错误处理。

### `handleSearchChange`
- 作用：处理 SearchBar 输入变化，更新本地 `searchQuery` 并发送后端搜索请求。
- 输入：`next: string`，新的搜索字符串。
- 输出：更新 Zustand store；若存在 `activeDeviceId`，向引擎发送 `set_search`。
- 调用关系：作为 `SearchBar` 的 `onChange` 回调传入。
- 注意事项：不修改 QueryBar 的过滤逻辑；没有活动设备时只更新本地搜索查询。

### `handleServerMessage` 的 `search_results` 分支
- 作用：将后端返回的匹配序号保存到 `searchMatches`。
- 输入：`ServerMessage` 中的 `{ type: 'search_results', matches }`。
- 输出：更新 Zustand store 的 `searchMatches`。
- 调用关系：由 `EngineClient` 收到服务端消息后触发。
- 注意事项：当前不会按 `deviceId` 过滤结果，沿用当前单 mock 设备上下文。

## 6. 配置变更

无配置变更。

## 7. 影响范围分析

### 直接影响
- 引擎设备上下文的日志搜索能力。
- WebSocket 客户端/服务端搜索协议。
- 渲染端应用状态中的搜索匹配结果。
- 顶部 SearchBar 的输入处理路径。

### 间接影响
- 后续 LogView 或导航功能可以消费 `searchMatches` 实现搜索结果跳转或计数。
- 后端搜索现在依赖当前 visible/hidden 状态，因此过滤条件变化后搜索结果可能需要由前端重新触发搜索才能同步。

### 风险点
- 当前搜索对每条候选日志执行 `to_lowercase()`，在最大 1,000,000 条日志时可能产生 CPU 和内存开销。
- `options` 字段已接收但未应用，如果未来 UI 开启正则/大小写敏感/整词选项，需要补充实现。
- `search_results` 更新未按 `deviceId` 做前端过滤，目前在单 mock 设备模型下可接受，多设备支持时需要调整。

### 兼容性
现有过滤、设备列表、日志推送、统计和 recorder status 消息保持不变。
新增消息类型不会改变已有消息的序列化字段命名。
QueryBar 的 `set_filter` 发送逻辑保持不变。

## 8. 验证记录

| 验证项 | 结果 | 说明 |
|--------|------|------|
| 编译通过 | 是 | `npm run build` 已通过，包含 TypeScript 检查、main 进程编译和 Vite 构建。 |
| 单元测试通过 | 是 | `cargo test -p als-engine` 已通过：20 passed，0 failed。 |
| 功能验证通过 | 否 | 未进行运行中应用的端到端交互验证；用户要求不实现 E2E 或验收清单。 |
| 回归测试通过 | 是 | 已执行现有引擎单元测试和前端构建，未发现失败；未执行额外 E2E 回归。 |

## 9. 回滚方案

如出现问题，可回滚以下文件到本次提交前状态：
- `engine/src/device.rs`
- `engine/src/websocket.rs`
- `src/renderer/state/appStore.ts`
- `src/renderer/App.tsx`
- `docs/CHANGELOG-2026-07-05-v3.md`

无配置文件需要恢复。
回滚风险较低，但回滚后前端 `set_search` 消息和后端 `search_results` 能力会消失，依赖 `searchMatches` 的后续功能也将不可用。

## 10. 后续优化建议（可选）

- 若搜索性能成为瓶颈，可考虑缓存 lowercase message 或引入增量索引。
- 多设备支持落地后，前端 `search_results` 应按 `deviceId` 与当前活动设备匹配后再更新。
- 若 UI 暴露搜索选项，需要在后端正式支持 `regex`、`caseSensitive`、`wholeWord`。

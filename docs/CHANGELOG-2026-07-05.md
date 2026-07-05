# CHANGELOG - 2026-07-05

## 1. 修改概述

本次任务为 Android Logcat Studio 实现 Task 10 的 MVP 日志查看界面。
新增了设备标签、查询过滤、搜索、虚拟化日志列表、统计面板和底部状态栏组件。
主应用从简单连接状态占位界面改为暗色调调试工作台布局。
查询和搜索输入仅更新前端 Zustand store，未向后端发送过滤或搜索消息。
保留了 Task 9 中 EngineClient 的 useRef 重复连接保护逻辑，避免 React StrictMode 下重复建立连接。

## 2. 需求来源

### 用户需求
Implement Task 10 only in /home/noah/Codes/PC/AndroidLogcatStudio/.worktrees/als-mvp. Create renderer components DeviceTabs, QueryBar, SearchBar, LogView, StatsPanel, StatusBar; modify App.tsx and styles.css; preserve EngineClient useRef duplicate guard; do not wire backend filter/search messages; run npm run build and cargo test -p als-engine; commit with message feat: add MVP logcat interface.

### 目标结果
完成 MVP logcat 前端界面组合：顶部工具栏与搜索框、设备标签、查询过滤输入、虚拟化日志输出区、统计侧栏和状态栏，并通过构建、引擎测试和运行时 UI 加载验证。

## 3. 修改范围

| 文件 | 类型 | 修改内容 |
|------|------|----------|
| src/renderer/components/DeviceTabs.tsx | 新增 | 渲染连接设备导航，支持 active class 和 Connected devices aria-label。 |
| src/renderer/components/QueryBar.tsx | 新增 | 渲染 labeled Query Filter 输入框，更新前端查询字符串。 |
| src/renderer/components/SearchBar.tsx | 新增 | 渲染 labeled Search 输入框，更新前端搜索字符串。 |
| src/renderer/components/LogView.tsx | 新增 | 使用 react-virtuoso 虚拟化展示日志，并高亮第一处大小写不敏感搜索匹配。 |
| src/renderer/components/StatsPanel.tsx | 新增 | 展示 Errors、Warnings、Logs/s、Memory、Hidden 统计信息。 |
| src/renderer/components/StatusBar.tsx | 新增 | 展示连接状态、录制路径、可见日志数量和 warning strong 文本。 |
| src/renderer/App.tsx | 修改 | 组合 Task 10 UI 组件，并保留 EngineClient useRef 连接保护。 |
| src/renderer/styles.css | 修改 | 实现暗色 debug workbench 布局、面板、边框、日志等宽字体和 level 颜色。 |
| docs/CHANGELOG-2026-07-05.md | 新增 | 记录本次任务变更、验证结果和回滚方案。 |

## 4. 核心实现说明

### 修改前
App 仅在启动时创建 EngineClient 并连接后端，然后显示一个 toolbar 和 empty-state 文本，用于展示连接中或已连接日志数量。界面没有设备区域、查询输入、搜索输入、日志列表、统计面板或底部状态栏。

### 修改后
App 继续使用 useRef 保存 EngineClient 和 hasConnectedRef，保留重复连接保护。界面改为五段式布局：toolbar、device tabs、query controls、content grid、status bar。SearchBar 和 QueryBar 通过 store setter 更新本地状态，不调用 EngineClient.send。LogView 通过 react-virtuoso 渲染 store 中的 logs，并对 message 中第一处大小写不敏感搜索匹配包裹 mark。StatsPanel 和 StatusBar 从 store 派生展示统计、录制和连接状态。

### 为什么这样实现
将界面拆分为小组件可以保持 App 只负责组合和状态接线，便于后续任务单独扩展设备切换、过滤、搜索结果和日志操作。当前任务明确要求不接线后端过滤/搜索消息，因此输入组件只触发 Zustand store setter，避免提前改变 WebSocket 协议行为。日志列表使用 react-virtuoso 以适配大量日志的渲染场景。

### 为什么没有采用其他方案
没有把所有 UI 写在 App.tsx 中，因为这样会让后续维护和测试变困难。没有在输入变化时发送 set_filter 或 set_search WebSocket 消息，因为用户明确要求本任务不要接线后端过滤/搜索消息。没有实现设备切换回调，因为当前 store 尚未提供切换 activeDeviceId 的 action，Task 10 只要求 DeviceTabs 接收 devices 和 activeDeviceId 并设置 active class。

## 5. 关键函数说明

### DeviceTabs
- 作用：展示 connected devices 导航列表，并为当前设备按钮添加 active class。
- 输入：devices、activeDeviceId。
- 输出：React 设备导航 UI。
- 调用关系：由 App 调用，读取 Zustand store 的 devices 和 activeDeviceId。
- 注意事项：当前仅展示设备状态，不触发设备切换或后端消息。

### QueryBar
- 作用：展示 Query Filter 输入框并把用户输入传给上层回调。
- 输入：value、onChange。
- 输出：React 查询输入 UI。
- 调用关系：由 App 调用，onChange 绑定 useAppStore.setFilterQuery。
- 注意事项：只更新 store，不发送 set_filter 消息。

### SearchBar
- 作用：展示 Search 输入框并把用户输入传给上层回调。
- 输入：value、onChange。
- 输出：React 搜索输入 UI。
- 调用关系：由 App 调用，onChange 绑定 useAppStore.setSearchQuery。
- 注意事项：只更新 store，不发送 set_search 消息。

### LogView
- 作用：使用 Virtuoso 虚拟化展示日志输出。
- 输入：logs、searchQuery。
- 输出：React 日志列表 UI。
- 调用关系：由 App 调用，读取 store 中 logs 和 searchQuery。
- 注意事项：日志字段展示 time、pid、tid、level 首字母大写、tag、message；仅高亮第一处大小写不敏感匹配。

### highlightFirstMatch
- 作用：在日志消息中查找第一处搜索命中并用 mark 包裹。
- 输入：message、searchQuery。
- 输出：原始字符串或包含 mark 的 React fragment。
- 调用关系：LogView 渲染每一行日志时调用。
- 注意事项：空白搜索词不高亮；当前为普通字符串匹配，不按正则处理。

### StatsPanel
- 作用：展示错误、警告、日志速率、内存和隐藏日志统计。
- 输入：stats。
- 输出：React 统计侧栏 UI。
- 调用关系：由 App 调用，读取 store 中 stats。
- 注意事项：Memory 使用 B/KB/MB 格式化。

### formatMemory
- 作用：将 bytes 转为 B、KB 或 MB 文本。
- 输入：bytes number。
- 输出：格式化内存字符串。
- 调用关系：StatsPanel 渲染 Memory 时调用。
- 注意事项：KB 和 MB 保留 1 位小数。

### StatusBar
- 作用：展示 connected/disconnected、recorder path 或 pending、visible log count 和 warning。
- 输入：connected、recorderPath、visibleLogCount、warning。
- 输出：React 底部状态栏 UI。
- 调用关系：由 App 调用，读取 store 中连接、录制和日志数量状态。
- 注意事项：warning 使用 strong 渲染。

## 6. 配置变更

无配置变更。

## 7. 影响范围分析

### 直接影响
src/renderer 前端 UI 层，包括 App 组合、组件目录和全局样式。

### 间接影响
Electron renderer 启动后会加载新的界面结构；现有 EngineClient 连接逻辑仍会执行。Zustand store 的 filterQuery 和 searchQuery 现在有可交互输入来源。

### 风险点
LogView 依赖 react-virtuoso 容器高度，父级布局必须保持 min-height: 0 和明确网格区域。当前设备标签按钮没有设备切换行为，后续如需要切换设备需增加 store action 和事件处理。运行时验证使用浏览器预览并模拟 window.als，不等同于完整 Electron + engine 真实设备流验证。

### 兼容性
Task 9 的 EngineClient useRef 连接保护逻辑保留；没有新增后端 WebSocket 消息发送，因此不会改变后端过滤/搜索协议行为。

## 8. 验证记录

| 验证项 | 结果 | 说明 |
|--------|------|------|
| 编译通过 | 是 | 已执行 npm run build，TypeScript 和 Vite build 均通过。 |
| 单元测试通过 | 是 | 已执行 cargo test -p als-engine，18 个测试通过；存在既有 dead_code warning：RingBuffer::len 未使用。 |
| 功能验证通过 | 是 | 已通过 Vite preview + Playwright/Chrome 加载构建后的 renderer，确认界面渲染、Search/Query 输入可更新、统计和状态栏可见。 |
| 回归测试通过 | 否 | 未执行完整回归测试；本次仅执行指定构建、引擎测试和 UI 加载/输入验证。 |

## 9. 回滚方案

出现问题时可回滚本次提交，或恢复以下文件：
- 删除 src/renderer/components/DeviceTabs.tsx
- 删除 src/renderer/components/QueryBar.tsx
- 删除 src/renderer/components/SearchBar.tsx
- 删除 src/renderer/components/LogView.tsx
- 删除 src/renderer/components/StatsPanel.tsx
- 删除 src/renderer/components/StatusBar.tsx
- 将 src/renderer/App.tsx 恢复为 Task 9 的连接状态占位界面
- 将 src/renderer/styles.css 恢复为 Task 9 的基础样式
- 删除 docs/CHANGELOG-2026-07-05.md

无配置需要恢复。回滚风险较低，但会移除 Task 10 MVP UI，应用将退回仅显示连接状态和日志数量的界面。

## 10. 后续优化建议（可选）

- 为 DeviceTabs 增加 activeDeviceId 切换 action 和键盘可访问性状态管理。
- 在后续任务中按需求接入后端 set_filter/set_search 消息。
- 为 LogView 增加空日志提示和更完整的日志 level 显示策略。
- 为 Electron + engine + 模拟 WebSocket 消息建立可复用的项目 verifier skill。

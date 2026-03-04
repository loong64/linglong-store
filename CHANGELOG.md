# Changelog

## [2.3.0] - 2026-03-04

### 新增
- 玲珑进程页全面重构，升级为符合桌面工具习惯的进程管理页
  - 新增顶部状态工具栏：展示运行中数量、上次刷新时间、手动刷新按钮、静默刷新状态
  - 新增行级右键上下文菜单（`@tauri-apps/api/menu` 原生菜单）：停止进程、复制进入容器命令、复制应用 ID / PID / 容器 ID、刷新列表
  - 新增行级操作 loading，停止进程时仅锁定当前行，不影响整表交互
  - 新增明确空状态，无运行中进程时展示提示
  - 新增刷新失败时保留旧数据并展示错误横幅，支持手动刷新恢复
  - "更多"按钮作为右键菜单的显式兜底入口

### 优化
- 刷新策略从固定 1s 轮询改为智能条件感知刷新（默认 3s）
  - 仅当标签页激活且页面可见时自动刷新
  - 并发保护：上一轮未完成时跳过本轮
  - 失败退避：1次失败→3s / 2次→6s / 3次以上→10s
  - 页面从不可见切回前台时立即补刷新
- Rust 进程查询从 N+1 外部命令优化为固定 2 次（`ll-cli ps` + `ll-cli list --json --type=all`），大幅降低查询延迟
- `rowKey` 改为稳定的 `containerId`，消除索引导致的多余重渲染
- `ApplicationCard` 从组件内直接订阅 `installedApps / updates / installQueue` 调整为页面级统一建索引并下发布尔状态，减少切页、滚动和安装过程中的卡片级重渲染
- 新增通用 `ApplicationCardSkeleton` 骨架卡片组件，`customCategory`、`allApps`、`searchList`、`ranking` 首屏改为骨架屏加载，分页追加继续使用底部“加载中...”

### 技术细节
- 新增 `src/hooks/useLinglongProcesses.ts`（刷新、退避、行级操作状态统一管理）
- 新增 `src/hooks/useApplicationCardModel.ts`，统一卡片安装状态索引与交互动作
- 新增 `src/pages/myApps/components/linglongProcess/ProcessToolbar.tsx`
- 新增 `src/pages/myApps/components/linglongProcess/ProcessTable.tsx`
- `src/types/api/invoke.d.ts` 新增 `RunningApp` 类型，`id` 字段作为稳定唯一键
- Rust `LinglongAppInfo` 新增 `id` 字段（序列化为 camelCase），前后端类型对齐
- `src-tauri/src/services/process.rs` 重构 `get_running_linglong_apps()`，移除 N+1 `ll-cli info` 调用



### 新增
- 应用详情页新增"创建桌面快捷方式"功能按钮
  - 仅对已安装应用显示该按钮
  - 自动从 `ll-cli content` 获取应用导出的 `.desktop` 文件
  - 复制到用户目录 `~/Desktop`，无需管理员权限
  - 同名快捷方式已存在时友好提示，不会覆盖
  - 采用 Link 风格按钮，与现有 UI 保持一致

### 技术细节
- Rust 新增 `create_desktop_shortcut` 命令
- 前端 `src/apis/invoke/index.ts` 新增 `createDesktopShortcut` API 封装
- 详情页组件新增快捷方式按钮及相关状态管理

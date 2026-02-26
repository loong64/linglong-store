# Changelog

## [2.1.2] - 2026-02-26 19:00

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



# Coppy

一个轻量的剪贴板管理器（Tauri v2 + React + TypeScript）。

## 功能

- **剪贴板监听**
  - 监听系统剪贴板变化（文本 / 图片）。
- **一键粘贴**
  - 选中条目后自动写入系统剪贴板，并模拟 `Ctrl+V` 粘贴到上一个前台应用。
- **图片支持**
  - 图片剪贴板会以缩略图形式展示，可一键粘贴。
- **收藏夹 + 文件夹**
  - 支持将条目收藏到指定文件夹。
  - 通过顶部 Tab 切换不同收藏文件夹。
- **持久化**
  - 收藏（含文件夹结构）会写入本地文件，重启后可恢复。
- **开机自启（设置项）**
  - 标题栏 `⚙` 打开设置，支持启用/关闭开机自启。

## 快捷键

- **双击 Ctrl**：显示/隐藏主窗口
- **↑ / ↓**：上下选择条目
- **Enter**：粘贴当前选中条目
- **1-9**：直接粘贴第 N 个条目
- **Esc**：隐藏窗口

## 数据存储位置

- **收藏与文件夹**：保存在 Tauri `app_data_dir` 下的 `favorites.json`
  - Windows 通常在：`%APPDATA%\com.po.tauri-app\favorites.json`
- **历史记录（仅文本）**：保存在浏览器 localStorage（用于快速恢复最近文本，不保存大图片数据）

## 开发

### 环境要求

- Node.js（建议 18+）
- Rust stable（建议最新）
- Windows：需要安装 MSVC Build Tools（含 Windows 10/11 SDK）

### 安装依赖

```bash
npm install
```

### 启动开发模式

```bash
npm run tauri dev
```

### 构建

```bash
npm run tauri build
```

## 常见问题

### 1) `Failed to resolve import "@tauri-apps/plugin-autostart"`

当前版本的开机自启通过 **Rust 命令**实现，不需要前端引入 `@tauri-apps/plugin-autostart`。

- 请确认 `src/App.tsx` 中没有 `@tauri-apps/plugin-autostart` 的 import。
- 如果你是从旧分支切换过来，建议执行一次：

```bash
npm install
```

### 2) 窗口边缘出现白边

这通常和透明窗口/圆角渲染有关。

- 当前配置使用非透明窗口，并通过 `html/body` 背景色避免圆角处漏出白底。

## 推荐 IDE 设置

- VS Code + Tauri + rust-analyzer
  - https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode
  - https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer

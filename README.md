# Coppy

一个轻量的剪贴板管理器（Tauri v2 + React + TypeScript）。

## 界面预览

<p align="center">
  <img src="docs/images/screenshot-main.png" width="400" alt="主界面 - 剪贴板历史与收藏">
  <img src="docs/images/screenshot-folder.png" width="400" alt="新建文件夹弹窗">
</p>

| 主界面 | 新建文件夹 |
|:------:|:----------:|
| 剪贴板历史列表，支持收藏与文件夹分类 | 创建自定义收藏文件夹 |

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
  - Windows 通常在：`%APPDATA%\com.coppy.app\favorites.json`
- **历史记录（仅文本）**：保存在浏览器 localStorage（用于快速恢复最近文本，不保存大图片数据）

## 更新日志

### v0.1.0 (2026-01-17)

- ✨ UI 优化：移除渐变背景，改用纯白色简洁风格
- 🔧 窗口定位优化：弹出窗口现在出现在光标上方，更贴近输入位置
- 📦 重命名项目：应用名称正式命名为 **Coppy**
- 🐛 修复圆角显示问题

## 后续优化计划

### 🔄 多端本地同步
- [ ] 局域网设备发现与配对
- [ ] 剪贴板历史实时同步（TCP/UDP）
- [ ] 端到端加密传输
- [ ] 冲突处理与合并策略

### 🎨 UI/UX 改进
- [ ] 深色模式优化
- [ ] 自定义主题颜色
- [ ] 条目搜索与过滤功能增强
- [ ] 拖拽排序收藏条目
- [ ] 右键菜单（编辑、删除、分享）

### 📱 跨平台支持
- [ ] macOS 版本适配
- [ ] Linux 版本适配
- [ ] 移动端配套 App（可选）

### 🔐 安全与隐私
- [ ] 敏感内容自动识别与隐藏
- [ ] 密码类内容自动清除
- [ ] 历史记录加密存储

### ⚡ 性能优化
- [ ] 大量历史记录时的虚拟滚动
- [ ] 图片压缩与懒加载
- [ ] 后台内存占用优化

### 🔗 集成功能
- [ ] 快捷短语模板
- [ ] OCR 图片文字识别
- [ ] 翻译集成
- [ ] 云端备份（可选）

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

# 日程管理应用 (Planning)

一个基于 **Tauri + Vue 3** 开发的桌面端日程管理应用，支持周视图日历、事件管理和数据持久化。

## 功能特性

### 📅 周视图日历
- 显示完整的周一到周日周视图
- 时间范围：8:00 - 22:00（每半小时一个时间片）
- 支持周导航：上一周/下一周/本周
- 显示当前周数和本年剩余周数
- 自动高亮今天日期
- 周六、周日标记为休息日

### ✨ 事件管理
- **拖拽创建事件**：在日历网格上拖拽即可创建新事件
- **编辑事件**：点击事件可编辑标题、日期、时间和描述
- **删除事件**：支持从弹窗或列表中删除事件
- **事件列表**：右侧边栏显示本周所有事件，按日期和时间排序
- **数据持久化**：使用 localStorage 自动保存事件数据

### 🎨 界面设计
- 现代化 UI 设计，渐变色主题
- 响应式布局，自适应窗口大小
- 支持系统深色模式
- 流畅的交互动画效果

## 技术栈

- **前端框架**: Vue 3 (Composition API + `<script setup>`)
- **UI 组件库**: Element Plus
- **桌面框架**: Tauri v2
- **构建工具**: Vite
- **后端语言**: Rust (Tauri)

## 开发环境

### 推荐的 IDE 设置

- [VS Code](https://code.visualstudio.com/)
- [Vue - Official](https://marketplace.visualstudio.com/items?itemName=Vue.volar)
- [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode)
- [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## 快速开始

### 安装依赖

```bash
npm install
```

### 开发模式

```bash
npm run tauri dev
```

### 构建应用

```bash
npm run tauri build
```

## 项目结构

```
planning/
├── index.html              # 入口 HTML
├── package.json            # 前端依赖配置
├── vite.config.js          # Vite 构建配置
├── src/
│   ├── main.js             # Vue 应用入口
│   ├── App.vue             # 根组件
│   └── views/
│       └── Calendar.vue    # 日历主组件
└── src-tauri/              # Tauri Rust 后端
    ├── src/
    │   └── lib.rs          # Rust 主代码
    └── Cargo.toml          # Rust 依赖配置
```

## 使用说明

### 创建事件
1. 在日历网格中按住鼠标左键
2. 拖拽到结束时间位置
3. 松开鼠标即可创建新事件

### 编辑事件
1. 点击日历上的事件或右侧事件列表
2. 在弹窗中修改事件信息
3. 点击"保存"保存修改

### 删除事件
- 方法一：在编辑弹窗中点击"删除"
- 方法二：在右侧事件列表中，悬停在事件上点击"×"按钮

### 周导航
- 点击"上一周"切换到上周
- 点击"本周"返回当前周
- 点击"下一周"切换到下周

## 数据存储

事件数据自动保存在浏览器的 localStorage 中，键名为 `calendar-events`。数据格式如下：

```json
[
  {
    "id": 1234567890,
    "date": "2026-02-04",
    "startTime": 9,
    "endTime": 10,
    "title": "会议",
    "description": "项目讨论"
  }
]
```

## 浏览器支持

本应用基于 Tauri，支持以下操作系统：
- Windows
- macOS
- Linux

## 许可证

MIT License

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
- **事件类型分类**：支持任务、会议、问题、提醒、其他五种类型，每种类型有独立的颜色标识
- **事件列表**：右侧边栏显示本周所有事件，按日期和时间排序
- **数据持久化**：使用 localStorage 自动保存事件数据
- **事件详情提示**：鼠标悬停在事件上显示详细信息（类型、标题、时间、描述）

### 🔔 事件提醒
- **智能提醒**：事件开始前 5 分钟自动发送提醒通知
- **系统通知**：使用原生系统通知，不依赖应用窗口状态
- **提醒开关**：可随时启用或禁用提醒功能
- **测试功能**：提供"测试"按钮验证通知功能
- **防重复提醒**：同一事件每天只提醒一次
- **权限管理**：自动请求和检查通知权限

### 🎨 界面设计
- 现代化 UI 设计，渐变色主题
- 响应式布局，自适应窗口大小
- 支持系统深色模式
- 流畅的交互动画效果

## 技术栈

- **前端框架**: Vue 3 (Composition API + `<script setup>`)
- **UI 组件库**: Element Plus
- **桌面框架**: Tauri v2
- **通知插件**: @tauri-apps/plugin-notification
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

### 事件提醒
- **启用/禁用提醒**：使用右上角的"提醒"开关控制
- **测试通知**：点击提醒开关旁的"测试"按钮验证通知功能
- **提醒时机**：事件开始前 5 分钟自动发送提醒
- **通知类型**：
  - 优先使用系统原生通知（需要授予通知权限）
  - 如果系统通知不可用，使用应用内弹窗通知
- **权限说明**：首次使用提醒功能时，系统会请求通知权限，请允许后才能接收系统通知

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
    "type": "meeting",
    "description": "项目讨论"
  }
]
```

事件类型说明：
- `task`: 任务（蓝色渐变）
- `meeting`: 会议（绿色渐变）
- `issue`: 问题（红色渐变）
- `reminder`: 提醒（橙色渐变）
- `other`: 其他（灰色渐变）

## 浏览器支持

本应用基于 Tauri，支持以下操作系统：
- Windows
- macOS
- Linux

## 许可证

MIT License

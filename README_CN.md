# Ximage 图片压缩工具

语言: **中文** | [English](README.md)

一款使用 Tauri 2.x、Vue 3、TypeScript 和 TailwindCSS 构建的跨平台桌面图片压缩应用。

## 功能特性

- ✅ 两栏式界面设计（左侧设置面板，右侧拖拽区域）
- ✅ 支持 JPG/JPEG、PNG、WEBP 图片格式
- ✅ 有损压缩（手动质量滑块）
- ✅ 无损压缩选项（PNG/WebP；JPEG 仍为有损）
- ✅ PNG 有损量化压缩（更高压缩率）
- ✅ 拖拽导入 + 文件选择
- ✅ 图片预览（缩略图）与单文件状态
- ✅ 总进度条展示
- ✅ 记住上次设置、语言、主题与保存路径
- ✅ 系统/浅色/深色主题
- ✅ 中英双语界面
- ✅ 跨平台支持（Mac、Windows）

## 技术栈

- **前端框架**: Vue 3 + TypeScript
- **UI 框架**: TailwindCSS
- **桌面应用框架**: Tauri 2.x
- **包管理器**: pnpm
- **图片处理库**: image、imagequant、oxipng、webp、png、img-parts（Rust）

## 安装与运行

```bash
# 克隆项目
cd /path/to/project

# 安装依赖
pnpm install

# 启动开发服务器
pnpm tauri dev
```

## 构建

```bash
# 构建应用
pnpm tauri build
```

## 项目结构

```
src/                 # 前端源码
├── App.vue         # 主应用组件
├── main.ts         # 应用入口
├── style.css       # 全局样式
src-tauri/          # Tauri 后端源码
├── Cargo.toml      # Rust 依赖配置
├── src/
│   └── main.rs     # Rust 主逻辑
├── tauri.conf.json # Tauri 配置
└── capabilities/   # 权限配置
```

## 功能说明

### 压缩设置

- **无损压缩**: 仅对 PNG/WebP 进行无损输出，JPEG 仍为有损编码
- **JPG 质量**: 调节 JPG 有损压缩质量（10%-100%）
- **WEBP 质量**: 调节 WebP 有损压缩质量（10%-100%）
- **PNG 质量**: 调节 PNG 有损量化压缩强度（10%-100%）

### 操作流程

1. 将图片文件拖拽到右侧区域或点击选择文件
2. 在左侧设置压缩参数
3. 点击"压缩图片"按钮开始处理
4. 查看压缩结果和节省的空间（未选择保存位置时会覆盖原文件）

### 重要说明

- **未选择保存位置**：将直接覆盖原文件
- **拖拽文件覆盖**：若无法获取原始路径，将要求选择保存位置
- **PNG 压缩**：有损模式使用调色板量化，压缩率更高但可能轻微失真
- **EXIF**：尽量保留 JPEG 的 EXIF 信息

## 许可证

MIT

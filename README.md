# MOSM - Modern OSM Editor

高性能、纯本地的 OpenStreetMap 地图编辑器。

## 技术栈

- **后端**: Rust + Tauri 2.0
- **前端**: Vue 3 + TypeScript + Canvas 2D
- **数据格式**: OSM PBF
- **投影**: Web Mercator (EPSG:3857)

## 核心特性

- 高性能 PBF 解析（并行处理）
- R-Tree 空间索引
- 零拷贝二进制 IPC 通信
- Web Mercator 投影渲染
- LOD（细节层次）节点显示策略

## 开发

### 环境要求

- Node.js 18+
- Rust 1.70+
- pnpm

### 安装依赖

```bash
pnpm install
```

### 开发模式

```bash
pnpm tauri dev
```

### 构建

```bash
pnpm tauri build
```

## 项目结构

```
mosm/
├── src/                    # 前端 Vue 代码
│   ├── components/         # Vue 组件
│   ├── composables/        # Vue 组合式函数
│   └── core/               # 核心渲染引擎
├── src-tauri/              # Rust 后端
│   └── src/
│       ├── lib.rs          # Tauri 命令入口
│       ├── osm_store.rs    # OSM 数据存储
│       ├── pbf_parser.rs   # PBF 解析器
│       ├── projection.rs   # Web Mercator 投影
│       ├── spatial_query.rs# 空间查询
│       └── binary_protocol.rs # 二进制序列化
├── docs/                   # 开发文档
└── testdata/               # 测试数据
```

## 推荐 IDE 设置

- [VS Code](https://code.visualstudio.com/)
- [Vue - Official](https://marketplace.visualstudio.com/items?itemName=Vue.volar)
- [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode)
- [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## 文档

- [开发问题与解决方案](docs/troubleshooting.md)

## 许可证

MIT

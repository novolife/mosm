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

## 功能列表

**地图交互**
- [x] 平移
- [x] 缩放
- [x] 要素选择
- [x] 选中要素高亮显示（青色高亮）
- [x] 左侧面板显示要素详情（ID、坐标、标签等）
- [x] 显示要素所属 Relation 及角色

**用户界面**
- [x] 文件选择对话框
- [x] 数据统计显示（节点/路径/关系数量）
- [x] 缩放级别显示
- [x] 渲染性能统计

**标签编辑（Tag CRUD）**
- [x] 标签查看面板
- [x] 标签添加/修改/删除
- [x] 标签修改同步到后端
- [x] 影响渲染的标签修改触发重绘

**撤销/重做（Undo/Redo）**
- [x] Command Pattern 架构
- [x] HistoryStack 实现
- [x] 标签编辑命令化（UpdateWayTagsCommand, UpdateNodeTagsCommand）
- [x] Ctrl+Z / Ctrl+Shift+Z 快捷键支持

**几何编辑（Spatial CRUD）**
- [ ] 节点拖拽移动
- [ ] 前端 Draft Layer（草稿层）
- [ ] 新增节点/路径
- [ ] 删除要素
- [ ] 拓扑关系维护

**数据导出**
- [ ] 导出为 .osm XML
- [ ] 导出为 .osc 变更集
- [ ] OSM API 上传支持

**高级功能**
- [ ] 搜索功能（按名称/标签搜索）
- [ ] 图层控制（显示/隐藏特定类型）
- [ ] 背景影像叠加
- [ ] 键盘快捷键
- [ ] 多选操作

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

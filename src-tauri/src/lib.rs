//! MOSM - Modern OSM Editor
//!
//! 高性能本地 OSM 地图编辑器的 Rust 后端
//!
//! # 模块结构
//!
//! - `osm_store`: OSM 数据存储层 (DashMap + R-Tree)
//! - `pbf_parser`: PBF 文件解析器
//! - `spatial_query`: 空间查询引擎
//! - `binary_protocol`: 高效二进制协议
//! - `polygon_assembler`: 多边形拓扑组装
//! - `render_feature`: 渲染特征系统
//! - `projection`: Web 墨卡托投影
//! - `history`: Undo/Redo 历史记录
//! - `types`: 公共类型定义
//! - `commands`: Tauri IPC 命令处理器

mod binary_protocol;
mod commands;
mod history;
mod osm_store;
mod pbf_parser;
mod polygon_assembler;
mod projection;
mod render_feature;
mod spatial_query;
mod types;

use history::HistoryManager;
use osm_store::OsmStore;
use std::sync::Arc;

/// 全局应用状态
pub struct AppState {
    pub store: Arc<OsmStore>,
    pub history: HistoryManager,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            store: Arc::new(OsmStore::new()),
            history: HistoryManager::new(),
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            // 数据命令
            commands::get_stats,
            commands::get_bounds,
            commands::load_pbf,
            // 查询命令
            commands::query_viewport_nodes,
            commands::query_viewport_coords,
            commands::query_viewport_full,
            commands::pick_feature,
            commands::get_node_details,
            commands::get_way_details,
            // 编辑命令
            commands::update_way_tags,
            commands::update_node_tags,
            commands::undo,
            commands::redo,
            commands::get_history_state,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

//! MOSM - Modern OSM Editor
//!
//! 高性能本地 OSM 地图编辑器的 Rust 后端

mod binary_protocol;
mod osm_store;
mod pbf_parser;
mod polygon_assembler;
mod projection;
mod render_feature;
mod spatial_query;

use osm_store::OsmStore;
use spatial_query::Viewport;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;


/// 全局应用状态
pub struct AppState {
    pub store: Arc<OsmStore>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            store: Arc::new(OsmStore::new()),
        }
    }
}

/// 获取存储统计信息
#[tauri::command]
fn get_stats(state: State<AppState>) -> osm_store::StoreStats {
    state.store.stats()
}

/// 获取数据边界 (用于自动定位相机)
#[tauri::command]
fn get_bounds(state: State<AppState>) -> Option<osm_store::DataBounds> {
    state.store.get_bounds()
}

/// 加载 PBF 文件 (异步命令)
#[tauri::command]
async fn load_pbf(path: String, state: State<'_, AppState>) -> Result<pbf_parser::ParseProgress, String> {
    let store = Arc::clone(&state.store);
    let path = PathBuf::from(path);

    tokio::task::spawn_blocking(move || {
        pbf_parser::parse_pbf_parallel(&path, store).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 查询视口内的节点 (返回二进制数据) - 已弃用，使用 query_viewport_full
#[tauri::command]
fn query_viewport_nodes(viewport: Viewport, state: State<AppState>) -> Vec<u8> {
    let result = spatial_query::query_viewport(&state.store, &viewport);
    binary_protocol::encode_priority_nodes(&result.nodes)
}

/// 查询视口内的坐标 (纯坐标，用于渲染) - 已弃用，使用 query_viewport_full
#[tauri::command]
fn query_viewport_coords(viewport: Viewport, state: State<AppState>) -> Vec<u8> {
    let result = spatial_query::query_viewport(&state.store, &viewport);
    // 转换为简单坐标格式
    let mut buffer = Vec::with_capacity(result.nodes.len() * 16);
    for node in &result.nodes {
        buffer.extend_from_slice(&node.lon.to_le_bytes());
        buffer.extend_from_slice(&node.lat.to_le_bytes());
    }
    buffer
}

/// 查询视口内的完整数据 (V4: 带节点优先级 + Polygon)
///
/// 返回格式:
/// - Header (16 bytes): node_count, way_count, polygon_count, truncated
/// - Nodes: node_count * 24 bytes (x, y, ref_count, padding)
/// - Way geometry: [total_ways][render_feature][point_count][coords...]...
/// - Polygon geometry: [total_polygons][render_feature][ring_count][point_count][coords...]...
#[tauri::command]
fn query_viewport_full(viewport: Viewport, state: State<AppState>) -> Vec<u8> {
    let result = spatial_query::query_viewport(&state.store, &viewport);

    binary_protocol::build_viewport_response_v4(
        &state.store,
        &result.nodes,
        &result.way_ids,
        &result.polygons,
        result.truncated,
    )
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            get_stats,
            get_bounds,
            load_pbf,
            query_viewport_nodes,
            query_viewport_coords,
            query_viewport_full,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

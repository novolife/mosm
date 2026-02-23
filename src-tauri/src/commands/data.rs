//! 数据加载和统计命令
//!
//! 处理 PBF 文件加载、统计信息和边界查询

use crate::osm_store::{DataBounds, StoreStats};
use crate::pbf_parser;
use crate::AppState;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

/// 获取存储统计信息
#[tauri::command]
pub fn get_stats(state: State<AppState>) -> StoreStats {
    state.store.stats()
}

/// 获取数据边界 (用于自动定位相机)
#[tauri::command]
pub fn get_bounds(state: State<AppState>) -> Option<DataBounds> {
    state.store.get_bounds()
}

/// 加载 PBF 文件 (异步命令)
#[tauri::command]
pub async fn load_pbf(
    path: String,
    state: State<'_, AppState>,
) -> Result<pbf_parser::ParseProgress, String> {
    let store = Arc::clone(&state.store);
    let path = PathBuf::from(path);

    tokio::task::spawn_blocking(move || {
        pbf_parser::parse_pbf_parallel(&path, store).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

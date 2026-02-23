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

/// 空间拾取：在指定坐标查找最近的要素
///
/// 参数：
/// - merc_x, merc_y: 墨卡托坐标（米）
/// - tolerance_meters: 拾取容差（米）
/// - zoom: 当前缩放级别，用于过滤不可见的节点
///
/// 返回：
/// - { type: "Node", id: 123 }
/// - { type: "Way", id: 456 }
/// - { type: "None" }
#[tauri::command]
fn pick_feature(
    merc_x: f64,
    merc_y: f64,
    tolerance_meters: f64,
    zoom: f64,
    state: State<AppState>,
) -> spatial_query::PickedFeature {
    spatial_query::pick_feature(&state.store, merc_x, merc_y, tolerance_meters, zoom)
}

/// 所属关系信息
#[derive(serde::Serialize)]
struct ParentRelation {
    id: i64,
    role: String,
    relation_type: Option<String>,
    name: Option<String>,
}

/// 节点详情
#[derive(serde::Serialize)]
struct NodeDetails {
    id: i64,
    lon: f64,
    lat: f64,
    tags: Vec<(String, String)>,
    ref_count: u16,
    parent_relations: Vec<ParentRelation>,
}

/// 路径详情
#[derive(serde::Serialize)]
struct WayDetails {
    id: i64,
    tags: Vec<(String, String)>,
    node_count: usize,
    is_area: bool,
    render_feature: u16,
    layer: i8,
    parent_relations: Vec<ParentRelation>,
}

/// 要素详情
#[derive(serde::Serialize)]
#[serde(tag = "type")]
enum FeatureDetails {
    Node(NodeDetails),
    Way(WayDetails),
    NotFound,
}

/// 查找包含指定要素的所有 Relation
fn find_parent_relations(
    store: &osm_store::OsmStore,
    member_type: osm_store::MemberType,
    member_id: i64,
) -> Vec<ParentRelation> {
    let mut result = Vec::new();

    for entry in store.relations.iter() {
        let relation = entry.value();

        // 查找该要素是否是这个 Relation 的成员
        for member in &relation.members {
            if member.member_type == member_type && member.ref_id == member_id {
                // 从 Relation 的 tags 中提取 type 和 name
                let relation_type = relation
                    .tags
                    .iter()
                    .find(|(k, _)| k == "type")
                    .map(|(_, v)| v.clone());

                let name = relation
                    .tags
                    .iter()
                    .find(|(k, _)| k == "name")
                    .map(|(_, v)| v.clone());

                result.push(ParentRelation {
                    id: relation.id,
                    role: member.role.clone(),
                    relation_type,
                    name,
                });

                break; // 一个 Relation 中同一个成员只出现一次（通常）
            }
        }
    }

    result
}

/// 获取节点详情
#[tauri::command]
fn get_node_details(node_id: i64, state: State<AppState>) -> FeatureDetails {
    if let Some(node) = state.store.nodes.get(&node_id) {
        let ref_count = state
            .store
            .node_ref_count
            .get(&node_id)
            .map(|r| *r)
            .unwrap_or(0);

        let parent_relations =
            find_parent_relations(&state.store, osm_store::MemberType::Node, node_id);

        FeatureDetails::Node(NodeDetails {
            id: node.id,
            lon: node.lon,
            lat: node.lat,
            tags: vec![], // Node 通常没有 tags，但保留接口
            ref_count,
            parent_relations,
        })
    } else {
        FeatureDetails::NotFound
    }
}

/// 获取路径详情
#[tauri::command]
fn get_way_details(way_id: i64, state: State<AppState>) -> FeatureDetails {
    if let Some(way) = state.store.ways.get(&way_id) {
        let parent_relations =
            find_parent_relations(&state.store, osm_store::MemberType::Way, way_id);

        FeatureDetails::Way(WayDetails {
            id: way.id,
            tags: way.tags.clone(),
            node_count: way.node_refs.len(),
            is_area: way.is_area,
            render_feature: way.render_feature,
            layer: way.layer,
            parent_relations,
        })
    } else {
        FeatureDetails::NotFound
    }
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
            pick_feature,
            get_node_details,
            get_way_details,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

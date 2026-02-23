//! 空间查询命令
//!
//! 处理视口查询、要素拾取和详情获取

use crate::osm_store::{MemberType, OsmStore};
use crate::spatial_query::{self, PickedFeature, Viewport};
use crate::types::{FeatureDetails, NodeDetails, ParentRelation, WayDetails};
use crate::{binary_protocol, AppState};
use tauri::State;

/// 查询视口内的节点 (返回二进制数据) - 已弃用，使用 query_viewport_full
#[tauri::command]
pub fn query_viewport_nodes(viewport: Viewport, state: State<AppState>) -> Vec<u8> {
    let result = spatial_query::query_viewport(&state.store, &viewport);
    binary_protocol::encode_priority_nodes(&result.nodes)
}

/// 查询视口内的坐标 (纯坐标，用于渲染) - 已弃用，使用 query_viewport_full
#[tauri::command]
pub fn query_viewport_coords(viewport: Viewport, state: State<AppState>) -> Vec<u8> {
    let result = spatial_query::query_viewport(&state.store, &viewport);
    let mut buffer = Vec::with_capacity(result.nodes.len() * 16);
    for node in &result.nodes {
        buffer.extend_from_slice(&node.lon.to_le_bytes());
        buffer.extend_from_slice(&node.lat.to_le_bytes());
    }
    buffer
}

/// 查询视口内的完整数据 (V4: 带节点优先级 + Polygon)
#[tauri::command]
pub fn query_viewport_full(viewport: Viewport, state: State<AppState>) -> Vec<u8> {
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
#[tauri::command]
pub fn pick_feature(
    merc_x: f64,
    merc_y: f64,
    tolerance_meters: f64,
    zoom: f64,
    state: State<AppState>,
) -> PickedFeature {
    spatial_query::pick_feature(&state.store, merc_x, merc_y, tolerance_meters, zoom)
}

/// 查找包含指定要素的所有 Relation
fn find_parent_relations(
    store: &OsmStore,
    member_type: MemberType,
    member_id: i64,
) -> Vec<ParentRelation> {
    let mut result = Vec::new();

    for entry in store.relations.iter() {
        let relation = entry.value();

        for member in &relation.members {
            if member.member_type == member_type && member.ref_id == member_id {
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

                break;
            }
        }
    }

    result
}

/// 获取节点详情
#[tauri::command]
pub fn get_node_details(node_id: i64, state: State<AppState>) -> FeatureDetails {
    if let Some(node) = state.store.nodes.get(&node_id) {
        let ref_count = state
            .store
            .node_ref_count
            .get(&node_id)
            .map(|r| *r)
            .unwrap_or(0);

        let parent_relations = find_parent_relations(&state.store, MemberType::Node, node_id);

        FeatureDetails::Node(NodeDetails {
            id: node.id,
            lon: node.lon,
            lat: node.lat,
            tags: node.tags.clone(),
            ref_count,
            parent_relations,
        })
    } else {
        FeatureDetails::NotFound
    }
}

/// 获取路径详情
#[tauri::command]
pub fn get_way_details(way_id: i64, state: State<AppState>) -> FeatureDetails {
    if let Some(way) = state.store.ways.get(&way_id) {
        let parent_relations = find_parent_relations(&state.store, MemberType::Way, way_id);

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

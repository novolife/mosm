//! 编辑命令
//!
//! 处理标签编辑、Undo/Redo 等修改操作

use crate::history::{
    AddNodeCommand, DeleteNodeCommand, DeleteWayCommand, MoveNodeCommand, UpdateNodeTagsCommand,
    UpdateWayTagsCommand,
};
use crate::osm_store::OsmNode;
use crate::polygon_assembler;
use crate::projection;
use crate::render_feature;
use crate::types::{AddNodeResult, DeleteFeatureResult, MoveNodeResult, UndoRedoResult, UpdateTagsResult};
use crate::AppState;
use tauri::State;

/// 更新路径标签（使用命令模式支持撤销）
#[tauri::command]
pub fn update_way_tags(
    way_id: i64,
    new_tags: Vec<(String, String)>,
    state: State<AppState>,
) -> UpdateTagsResult {
    let way = state.store.ways.get(&way_id);
    if way.is_none() {
        return UpdateTagsResult {
            success: false,
            render_feature: 0,
            layer: 0,
            is_area: false,
        };
    }

    let way = way.unwrap();
    let old_tags = way.tags.clone();
    let old_render_feature = way.render_feature;
    let old_layer = way.layer;
    let old_is_area = way.is_area;
    let node_refs = way.node_refs.clone();
    drop(way);

    let new_parsed = render_feature::parse_tags(&new_tags);
    let new_render_feature = new_parsed.feature;
    let new_layer = new_parsed.layer;
    let new_is_area = polygon_assembler::is_area_way(&new_tags, &node_refs);

    let command = UpdateWayTagsCommand {
        way_id,
        old_tags,
        new_tags,
        old_render_feature,
        new_render_feature,
        old_layer,
        new_layer,
        old_is_area,
        new_is_area,
    };

    let result = state.history.execute(Box::new(command), &state.store);

    UpdateTagsResult {
        success: result.success,
        render_feature: new_render_feature,
        layer: new_layer,
        is_area: new_is_area,
    }
}

/// 更新节点标签（使用命令模式支持撤销）
#[tauri::command]
pub fn update_node_tags(
    node_id: i64,
    new_tags: Vec<(String, String)>,
    state: State<AppState>,
) -> UpdateTagsResult {
    let node = state.store.nodes.get(&node_id);
    if node.is_none() {
        return UpdateTagsResult {
            success: false,
            render_feature: 0,
            layer: 0,
            is_area: false,
        };
    }

    let node = node.unwrap();
    let old_tags = node.tags.clone();
    drop(node);

    let command = UpdateNodeTagsCommand {
        node_id,
        old_tags,
        new_tags,
    };

    let result = state.history.execute(Box::new(command), &state.store);

    UpdateTagsResult {
        success: result.success,
        render_feature: 0,
        layer: 0,
        is_area: false,
    }
}

/// 撤销上一个操作
#[tauri::command]
pub fn undo(state: State<AppState>) -> UndoRedoResult {
    let result = state.history.undo(&state.store);
    UndoRedoResult {
        success: result.success,
        needs_redraw: result.needs_redraw,
        message: result.message,
        undo_count: state.history.undo_count(),
        redo_count: state.history.redo_count(),
    }
}

/// 重做上一个撤销的操作
#[tauri::command]
pub fn redo(state: State<AppState>) -> UndoRedoResult {
    let result = state.history.redo(&state.store);
    UndoRedoResult {
        success: result.success,
        needs_redraw: result.needs_redraw,
        message: result.message,
        undo_count: state.history.undo_count(),
        redo_count: state.history.redo_count(),
    }
}

/// 获取历史记录状态
#[tauri::command]
pub fn get_history_state(state: State<AppState>) -> (usize, usize) {
    (state.history.undo_count(), state.history.redo_count())
}

/// 移动节点（使用命令模式支持撤销）
///
/// 接收墨卡托坐标（米），转换为经纬度后更新节点
#[tauri::command]
pub fn move_node(
    node_id: i64,
    new_merc_x: f64,
    new_merc_y: f64,
    state: State<AppState>,
) -> MoveNodeResult {
    let node = state.store.nodes.get(&node_id);
    if node.is_none() {
        return MoveNodeResult {
            success: false,
            message: Some("Node not found".to_string()),
        };
    }

    let node = node.unwrap();
    let old_lon = node.lon;
    let old_lat = node.lat;
    drop(node);

    // 墨卡托坐标转经纬度
    let (new_lon, new_lat) = projection::mercator_to_lonlat(new_merc_x, new_merc_y);

    let command = MoveNodeCommand {
        node_id,
        old_lon,
        old_lat,
        new_lon,
        new_lat,
    };

    let result = state.history.execute(Box::new(command), &state.store);

    MoveNodeResult {
        success: result.success,
        message: result.message,
    }
}

/// 添加节点（使用命令模式支持撤销）
///
/// 接收墨卡托坐标（米），转换为经纬度后创建节点
#[tauri::command]
pub fn add_node(merc_x: f64, merc_y: f64, state: State<AppState>) -> AddNodeResult {
    // 墨卡托坐标转经纬度
    let (lon, lat) = projection::mercator_to_lonlat(merc_x, merc_y);

    // 生成负数 ID
    let node_id = state.store.generate_local_id();

    let node = OsmNode {
        id: node_id,
        lon,
        lat,
        tags: Vec::new(),
    };

    let command = AddNodeCommand { node };
    let result = state.history.execute(Box::new(command), &state.store);

    AddNodeResult {
        success: result.success,
        node_id,
        message: result.message,
    }
}

/// 删除 Way（使用命令模式支持撤销）
#[tauri::command]
pub fn delete_way(way_id: i64, state: State<AppState>) -> DeleteFeatureResult {
    let way = state.store.ways.get(&way_id);
    if way.is_none() {
        return DeleteFeatureResult {
            success: false,
            message: Some("Way not found".to_string()),
            cascaded_way_ids: Vec::new(),
        };
    }

    let way = way.unwrap().clone();
    drop(state.store.ways.get(&way_id)); // 确保释放引用

    let command = DeleteWayCommand { way };
    let result = state.history.execute(Box::new(command), &state.store);

    DeleteFeatureResult {
        success: result.success,
        message: result.message,
        cascaded_way_ids: Vec::new(),
    }
}

/// 删除节点（使用命令模式支持撤销，含级联处理）
///
/// 删除节点时：
/// 1. 从所有引用该节点的 Way 中移除
/// 2. 如果 Way 只剩 1 个节点，级联删除该 Way
#[tauri::command]
pub fn delete_node(node_id: i64, state: State<AppState>) -> DeleteFeatureResult {
    let node = state.store.nodes.get(&node_id);
    if node.is_none() {
        return DeleteFeatureResult {
            success: false,
            message: Some("Node not found".to_string()),
            cascaded_way_ids: Vec::new(),
        };
    }

    let node = node.unwrap().clone();
    drop(state.store.nodes.get(&node_id));

    // 收集所有引用该节点的 Way 及其位置
    let mut way_references: Vec<(i64, Vec<usize>)> = Vec::new();
    let mut cascaded_ways = Vec::new();

    let referencing_ways = state.store.find_ways_referencing_node(node_id);

    for way_id in referencing_ways {
        if let Some(way) = state.store.ways.get(&way_id) {
            // 记录节点在 Way 中的所有位置
            let indices: Vec<usize> = way
                .node_refs
                .iter()
                .enumerate()
                .filter(|(_, &id)| id == node_id)
                .map(|(i, _)| i)
                .collect();

            // 检查移除后 Way 是否仍有效
            let remaining_count = way.node_refs.len() - indices.len();
            if remaining_count < 2 {
                // Way 将变为无效，需要级联删除
                cascaded_ways.push(way.clone());
            }

            way_references.push((way_id, indices));
        }
    }

    let cascaded_way_ids: Vec<i64> = cascaded_ways.iter().map(|w| w.id).collect();

    let command = DeleteNodeCommand {
        node,
        way_references,
        cascaded_ways,
    };

    let result = state.history.execute(Box::new(command), &state.store);

    DeleteFeatureResult {
        success: result.success,
        message: result.message,
        cascaded_way_ids,
    }
}

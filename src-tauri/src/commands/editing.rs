//! 编辑命令
//!
//! 处理标签编辑、Undo/Redo 等修改操作

use crate::history::{UpdateNodeTagsCommand, UpdateWayTagsCommand};
use crate::polygon_assembler;
use crate::render_feature;
use crate::types::{UndoRedoResult, UpdateTagsResult};
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

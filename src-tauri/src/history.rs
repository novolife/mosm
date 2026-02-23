//! 撤销/重做历史记录系统 (Undo/Redo History)
//!
//! 使用命令模式 (Command Pattern) 实现：
//! - 每个编辑操作封装为一个 Command
//! - Command 必须实现 apply() 和 undo() 方法
//! - HistoryManager 维护 undo_stack 和 redo_stack

use crate::osm_store::{OsmNode, OsmStore, OsmWay};
use std::sync::Mutex;

/// 命令执行结果
#[derive(Debug, Clone, serde::Serialize)]
pub struct CommandResult {
    pub success: bool,
    pub needs_redraw: bool,
    pub message: Option<String>,
}

impl CommandResult {
    pub fn success(needs_redraw: bool) -> Self {
        Self {
            success: true,
            needs_redraw,
            message: None,
        }
    }

    pub fn failure(message: &str) -> Self {
        Self {
            success: false,
            needs_redraw: false,
            message: Some(message.to_string()),
        }
    }
}

/// 命令 Trait
///
/// 所有编辑操作必须实现此 trait
pub trait Command: Send + Sync {
    /// 执行命令（正向操作）
    fn apply(&self, store: &OsmStore) -> CommandResult;

    /// 撤销命令（逆向操作）
    fn undo(&self, store: &OsmStore) -> CommandResult;

    /// 命令描述（用于调试和 UI 显示）
    fn description(&self) -> String;
}

/// 更新 Way 标签命令
pub struct UpdateWayTagsCommand {
    pub way_id: i64,
    pub old_tags: Vec<(String, String)>,
    pub new_tags: Vec<(String, String)>,
    pub old_render_feature: u16,
    pub new_render_feature: u16,
    pub old_layer: i8,
    pub new_layer: i8,
    pub old_is_area: bool,
    pub new_is_area: bool,
}

impl Command for UpdateWayTagsCommand {
    fn apply(&self, store: &OsmStore) -> CommandResult {
        if let Some(mut way) = store.ways.get_mut(&self.way_id) {
            way.tags = self.new_tags.clone();
            way.render_feature = self.new_render_feature;
            way.layer = self.new_layer;
            way.is_area = self.new_is_area;
            CommandResult::success(self.old_render_feature != self.new_render_feature)
        } else {
            CommandResult::failure("Way not found")
        }
    }

    fn undo(&self, store: &OsmStore) -> CommandResult {
        if let Some(mut way) = store.ways.get_mut(&self.way_id) {
            way.tags = self.old_tags.clone();
            way.render_feature = self.old_render_feature;
            way.layer = self.old_layer;
            way.is_area = self.old_is_area;
            CommandResult::success(self.old_render_feature != self.new_render_feature)
        } else {
            CommandResult::failure("Way not found")
        }
    }

    fn description(&self) -> String {
        format!("Update tags for Way #{}", self.way_id)
    }
}

/// 更新 Node 标签命令
pub struct UpdateNodeTagsCommand {
    pub node_id: i64,
    pub old_tags: Vec<(String, String)>,
    pub new_tags: Vec<(String, String)>,
}

impl Command for UpdateNodeTagsCommand {
    fn apply(&self, store: &OsmStore) -> CommandResult {
        if let Some(mut node) = store.nodes.get_mut(&self.node_id) {
            node.tags = self.new_tags.clone();
            CommandResult::success(false)
        } else {
            CommandResult::failure("Node not found")
        }
    }

    fn undo(&self, store: &OsmStore) -> CommandResult {
        if let Some(mut node) = store.nodes.get_mut(&self.node_id) {
            node.tags = self.old_tags.clone();
            CommandResult::success(false)
        } else {
            CommandResult::failure("Node not found")
        }
    }

    fn description(&self) -> String {
        format!("Update tags for Node #{}", self.node_id)
    }
}

/// 移动节点命令
///
/// 更新节点坐标，同时维护 R-Tree 索引
pub struct MoveNodeCommand {
    pub node_id: i64,
    pub old_lon: f64,
    pub old_lat: f64,
    pub new_lon: f64,
    pub new_lat: f64,
}

impl Command for MoveNodeCommand {
    fn apply(&self, store: &OsmStore) -> CommandResult {
        if store.update_node_position(self.node_id, self.new_lon, self.new_lat) {
            CommandResult::success(true)
        } else {
            CommandResult::failure("Node not found")
        }
    }

    fn undo(&self, store: &OsmStore) -> CommandResult {
        if store.update_node_position(self.node_id, self.old_lon, self.old_lat) {
            CommandResult::success(true)
        } else {
            CommandResult::failure("Node not found")
        }
    }

    fn description(&self) -> String {
        format!(
            "Move Node #{} from ({:.6}, {:.6}) to ({:.6}, {:.6})",
            self.node_id, self.old_lon, self.old_lat, self.new_lon, self.new_lat
        )
    }
}

/// 添加节点命令
pub struct AddNodeCommand {
    pub node: OsmNode,
}

impl Command for AddNodeCommand {
    fn apply(&self, store: &OsmStore) -> CommandResult {
        store.add_node_with_index(self.node.clone());
        CommandResult::success(true)
    }

    fn undo(&self, store: &OsmStore) -> CommandResult {
        store.remove_node_with_index(self.node.id);
        CommandResult::success(true)
    }

    fn description(&self) -> String {
        format!(
            "Add Node #{} at ({:.6}, {:.6})",
            self.node.id, self.node.lon, self.node.lat
        )
    }
}

/// 删除 Way 命令
pub struct DeleteWayCommand {
    pub way: OsmWay,
}

impl Command for DeleteWayCommand {
    fn apply(&self, store: &OsmStore) -> CommandResult {
        store.remove_way_with_index(self.way.id);
        CommandResult::success(true)
    }

    fn undo(&self, store: &OsmStore) -> CommandResult {
        store.add_way_with_index(self.way.clone());
        CommandResult::success(true)
    }

    fn description(&self) -> String {
        format!("Delete Way #{}", self.way.id)
    }
}

/// 删除节点命令（含级联拓扑处理）
///
/// 删除节点时必须处理所有引用该节点的 Way：
/// 1. 从 Way 的 node_refs 中移除该节点
/// 2. 记录原始位置以便撤销时恢复
/// 3. 如果 Way 只剩 1 个节点，级联删除该 Way
pub struct DeleteNodeCommand {
    pub node: OsmNode,
    /// 节点在各个 Way 中的位置: (way_id, indices)
    pub way_references: Vec<(i64, Vec<usize>)>,
    /// 因节点删除而级联删除的 Way
    pub cascaded_ways: Vec<OsmWay>,
}

impl Command for DeleteNodeCommand {
    fn apply(&self, store: &OsmStore) -> CommandResult {
        // 1. 从所有引用的 Way 中移除该节点（但不在这里做，因为 way_references 已记录）
        for (way_id, _indices) in &self.way_references {
            store.remove_node_from_way(*way_id, self.node.id);
        }

        // 2. 级联删除无效的 Way（节点数 < 2）
        for way in &self.cascaded_ways {
            store.remove_way_with_index(way.id);
        }

        // 3. 删除节点本身
        store.remove_node_with_index(self.node.id);

        CommandResult::success(true)
    }

    fn undo(&self, store: &OsmStore) -> CommandResult {
        // 恢复顺序必须严格相反

        // 1. 恢复节点
        store.add_node_with_index(self.node.clone());

        // 2. 恢复级联删除的 Way
        for way in &self.cascaded_ways {
            store.add_way_with_index(way.clone());
        }

        // 3. 将节点恢复到各个 Way 的原始位置
        for (way_id, indices) in &self.way_references {
            store.insert_node_to_way(*way_id, self.node.id, indices);
        }

        CommandResult::success(true)
    }

    fn description(&self) -> String {
        format!(
            "Delete Node #{} (affects {} ways, cascades {} way deletions)",
            self.node.id,
            self.way_references.len(),
            self.cascaded_ways.len()
        )
    }
}

/// 历史记录管理器
pub struct HistoryManager {
    undo_stack: Mutex<Vec<Box<dyn Command>>>,
    redo_stack: Mutex<Vec<Box<dyn Command>>>,
}

impl Default for HistoryManager {
    fn default() -> Self {
        Self::new()
    }
}

impl HistoryManager {
    pub fn new() -> Self {
        Self {
            undo_stack: Mutex::new(Vec::new()),
            redo_stack: Mutex::new(Vec::new()),
        }
    }

    /// 执行命令并加入历史记录
    pub fn execute(&self, command: Box<dyn Command>, store: &OsmStore) -> CommandResult {
        let result = command.apply(store);

        if result.success {
            let mut undo_stack = self.undo_stack.lock().unwrap();
            let mut redo_stack = self.redo_stack.lock().unwrap();

            undo_stack.push(command);
            redo_stack.clear();
        }

        result
    }

    /// 撤销上一个命令
    pub fn undo(&self, store: &OsmStore) -> CommandResult {
        let command = {
            let mut undo_stack = self.undo_stack.lock().unwrap();
            undo_stack.pop()
        };

        if let Some(cmd) = command {
            let result = cmd.undo(store);

            if result.success {
                let mut redo_stack = self.redo_stack.lock().unwrap();
                redo_stack.push(cmd);
            }

            result
        } else {
            CommandResult::failure("Nothing to undo")
        }
    }

    /// 重做上一个撤销的命令
    pub fn redo(&self, store: &OsmStore) -> CommandResult {
        let command = {
            let mut redo_stack = self.redo_stack.lock().unwrap();
            redo_stack.pop()
        };

        if let Some(cmd) = command {
            let result = cmd.apply(store);

            if result.success {
                let mut undo_stack = self.undo_stack.lock().unwrap();
                undo_stack.push(cmd);
            }

            result
        } else {
            CommandResult::failure("Nothing to redo")
        }
    }

    /// 获取可撤销的命令数量
    pub fn undo_count(&self) -> usize {
        self.undo_stack.lock().unwrap().len()
    }

    /// 获取可重做的命令数量
    pub fn redo_count(&self) -> usize {
        self.redo_stack.lock().unwrap().len()
    }

    /// 清空历史记录
    pub fn clear(&self) {
        self.undo_stack.lock().unwrap().clear();
        self.redo_stack.lock().unwrap().clear();
    }
}

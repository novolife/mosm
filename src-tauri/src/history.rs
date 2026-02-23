//! 撤销/重做历史记录系统 (Undo/Redo History)
//!
//! 使用命令模式 (Command Pattern) 实现：
//! - 每个编辑操作封装为一个 Command
//! - Command 必须实现 apply() 和 undo() 方法
//! - HistoryManager 维护 undo_stack 和 redo_stack

use crate::osm_store::OsmStore;
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

//! 公共类型定义
//!
//! 集中管理跨模块共享的数据传输对象 (DTO)

use serde::Serialize;

/// 所属关系信息
#[derive(Serialize, Clone)]
pub struct ParentRelation {
    pub id: i64,
    pub role: String,
    pub relation_type: Option<String>,
    pub name: Option<String>,
}

/// 节点详情
#[derive(Serialize)]
pub struct NodeDetails {
    pub id: i64,
    pub lon: f64,
    pub lat: f64,
    pub tags: Vec<(String, String)>,
    pub ref_count: u16,
    pub parent_relations: Vec<ParentRelation>,
}

/// 路径详情
#[derive(Serialize)]
pub struct WayDetails {
    pub id: i64,
    pub tags: Vec<(String, String)>,
    pub node_count: usize,
    pub is_area: bool,
    pub render_feature: u16,
    pub layer: i8,
    pub parent_relations: Vec<ParentRelation>,
}

/// 要素详情
#[derive(Serialize)]
#[serde(tag = "type")]
pub enum FeatureDetails {
    Node(NodeDetails),
    Way(WayDetails),
    NotFound,
}

/// 标签更新结果
#[derive(Serialize)]
pub struct UpdateTagsResult {
    pub success: bool,
    pub render_feature: u16,
    pub layer: i8,
    pub is_area: bool,
}

/// Undo/Redo 操作结果
#[derive(Serialize)]
pub struct UndoRedoResult {
    pub success: bool,
    pub needs_redraw: bool,
    pub message: Option<String>,
    pub undo_count: usize,
    pub redo_count: usize,
}

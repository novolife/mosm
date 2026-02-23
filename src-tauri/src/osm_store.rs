//! OSM 内存数据存储层
//!
//! 架构设计：
//! - DashMap 存储实体本身 (O(1) 随机访问)
//! - R-Tree 存储空间索引 (O(log n) 范围查询)
//! - 两阶段加载：先收集数据，再批量构建索引

use dashmap::DashMap;
use rstar::{RTree, RTreeObject, AABB};
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};

/// OSM 节点 (Node) - 地图上的一个坐标点
#[derive(Debug, Clone, Copy)]
pub struct OsmNode {
    pub id: i64,
    pub lat: f64,
    pub lon: f64,
}

/// OSM 路径 (Way) - 由多个节点组成的线或面
#[derive(Debug, Clone)]
pub struct OsmWay {
    pub id: i64,
    pub node_refs: Vec<i64>,
    pub tags: Vec<(String, String)>,
    /// 预计算的渲染特征 (u16 位掩码)
    /// 低 8 位: BaseType, 高 8 位: Flags
    pub render_feature: u16,
    /// OSM layer 值 (-5 到 +5)，用于 Z-order 排序
    pub layer: i8,
    /// 是否是闭合面 (Area)
    pub is_area: bool,
}

/// OSM 关系 (Relation) - 复杂的逻辑组合
#[derive(Debug, Clone)]
pub struct OsmRelation {
    pub id: i64,
    pub members: Vec<RelationMember>,
    pub tags: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct RelationMember {
    pub member_type: MemberType,
    pub ref_id: i64,
    pub role: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberType {
    Node,
    Way,
    Relation,
}

/// R-Tree 中的空间索引项 (只存 ID 和包围盒)
#[derive(Debug, Clone, Copy)]
pub struct SpatialEntry {
    pub id: i64,
    pub min_lon: f64,
    pub min_lat: f64,
    pub max_lon: f64,
    pub max_lat: f64,
}

impl RTreeObject for SpatialEntry {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners([self.min_lon, self.min_lat], [self.max_lon, self.max_lat])
    }
}

/// 核心数据存储结构
pub struct OsmStore {
    pub nodes: DashMap<i64, OsmNode>,
    pub ways: DashMap<i64, OsmWay>,
    pub relations: DashMap<i64, OsmRelation>,
    /// 节点被多少条 Way 引用 (用于渲染优先级)
    pub node_ref_count: DashMap<i64, u16>,
    node_index: RwLock<RTree<SpatialEntry>>,
    way_index: RwLock<RTree<SpatialEntry>>,
    index_dirty: AtomicBool,
}

impl OsmStore {
    pub fn new() -> Self {
        Self {
            nodes: DashMap::new(),
            ways: DashMap::new(),
            relations: DashMap::new(),
            node_ref_count: DashMap::new(),
            node_index: RwLock::new(RTree::new()),
            way_index: RwLock::new(RTree::new()),
            index_dirty: AtomicBool::new(false),
        }
    }

    /// 插入节点 (不更新索引，需要后续调用 rebuild_indices)
    pub fn insert_node(&self, node: OsmNode) {
        self.nodes.insert(node.id, node);
        self.index_dirty.store(true, Ordering::Relaxed);
    }

    /// 插入路径 (不更新索引，同时更新节点引用计数)
    pub fn insert_way(&self, way: OsmWay) {
        for &node_id in &way.node_refs {
            self.node_ref_count
                .entry(node_id)
                .and_modify(|c| *c = c.saturating_add(1))
                .or_insert(1);
        }
        self.ways.insert(way.id, way);
        self.index_dirty.store(true, Ordering::Relaxed);
    }

    /// 批量重建空间索引 (O(n log n) 一次性构建，比逐条插入快 100 倍)
    pub fn rebuild_indices(&self) {
        let node_entries: Vec<SpatialEntry> = self
            .nodes
            .iter()
            .map(|entry| {
                let node = entry.value();
                SpatialEntry {
                    id: node.id,
                    min_lon: node.lon,
                    min_lat: node.lat,
                    max_lon: node.lon,
                    max_lat: node.lat,
                }
            })
            .collect();

        let way_entries: Vec<SpatialEntry> = self
            .ways
            .iter()
            .filter_map(|entry| self.compute_way_bbox(entry.value()))
            .collect();

        if let Ok(mut index) = self.node_index.write() {
            *index = RTree::bulk_load(node_entries);
        }

        if let Ok(mut index) = self.way_index.write() {
            *index = RTree::bulk_load(way_entries);
        }

        self.index_dirty.store(false, Ordering::Relaxed);
    }

    /// 计算 Way 的包围盒
    fn compute_way_bbox(&self, way: &OsmWay) -> Option<SpatialEntry> {
        let mut min_lon = f64::MAX;
        let mut min_lat = f64::MAX;
        let mut max_lon = f64::MIN;
        let mut max_lat = f64::MIN;
        let mut found = false;

        for &node_id in &way.node_refs {
            if let Some(node) = self.nodes.get(&node_id) {
                min_lon = min_lon.min(node.lon);
                min_lat = min_lat.min(node.lat);
                max_lon = max_lon.max(node.lon);
                max_lat = max_lat.max(node.lat);
                found = true;
            }
        }

        if found {
            Some(SpatialEntry {
                id: way.id,
                min_lon,
                min_lat,
                max_lon,
                max_lat,
            })
        } else {
            None
        }
    }

    /// 视口范围查询节点
    pub fn query_nodes_in_viewport(
        &self,
        min_lon: f64,
        min_lat: f64,
        max_lon: f64,
        max_lat: f64,
    ) -> Vec<OsmNode> {
        let query_box = AABB::from_corners([min_lon, min_lat], [max_lon, max_lat]);

        let index = match self.node_index.read() {
            Ok(guard) => guard,
            Err(_) => return vec![],
        };

        index
            .locate_in_envelope_intersecting(&query_box)
            .filter_map(|entry| self.nodes.get(&entry.id).map(|n| *n))
            .collect()
    }

    /// 视口范围查询路径 ID
    pub fn query_way_ids_in_viewport(
        &self,
        min_lon: f64,
        min_lat: f64,
        max_lon: f64,
        max_lat: f64,
    ) -> Vec<i64> {
        let query_box = AABB::from_corners([min_lon, min_lat], [max_lon, max_lat]);

        let index = match self.way_index.read() {
            Ok(guard) => guard,
            Err(_) => return vec![],
        };

        index
            .locate_in_envelope_intersecting(&query_box)
            .map(|entry| entry.id)
            .collect()
    }

    /// 获取存储统计信息
    pub fn stats(&self) -> StoreStats {
        StoreStats {
            node_count: self.nodes.len(),
            way_count: self.ways.len(),
            relation_count: self.relations.len(),
        }
    }

    /// 获取所有数据的边界框 (从 DashMap 直接计算，不依赖索引)
    pub fn get_bounds(&self) -> Option<DataBounds> {
        if self.nodes.is_empty() {
            return None;
        }

        let mut min_lon = f64::MAX;
        let mut min_lat = f64::MAX;
        let mut max_lon = f64::MIN;
        let mut max_lat = f64::MIN;

        for entry in self.nodes.iter() {
            let node = entry.value();
            min_lon = min_lon.min(node.lon);
            min_lat = min_lat.min(node.lat);
            max_lon = max_lon.max(node.lon);
            max_lat = max_lat.max(node.lat);
        }

        Some(DataBounds {
            min_lon,
            min_lat,
            max_lon,
            max_lat,
            center_lon: (min_lon + max_lon) / 2.0,
            center_lat: (min_lat + max_lat) / 2.0,
        })
    }

    /// 获取节点索引的只读访问
    pub fn node_index(&self) -> std::sync::RwLockReadGuard<'_, RTree<SpatialEntry>> {
        self.node_index.read().unwrap()
    }

    /// 获取路径索引的只读访问
    pub fn way_index(&self) -> std::sync::RwLockReadGuard<'_, RTree<SpatialEntry>> {
        self.way_index.read().unwrap()
    }
}

impl Default for OsmStore {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StoreStats {
    pub node_count: usize,
    pub way_count: usize,
    pub relation_count: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DataBounds {
    pub min_lon: f64,
    pub min_lat: f64,
    pub max_lon: f64,
    pub max_lat: f64,
    pub center_lon: f64,
    pub center_lat: f64,
}

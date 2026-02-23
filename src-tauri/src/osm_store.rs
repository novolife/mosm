//! OSM 内存数据存储层
//!
//! 架构设计：
//! - DashMap 存储实体本身 (O(1) 随机访问)
//! - R-Tree 存储空间索引 (O(log n) 范围查询)
//! - 两阶段加载：先收集数据，再批量构建索引

use dashmap::DashMap;
use rstar::{RTree, RTreeObject, AABB};
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

/// OSM 节点 (Node) - 地图上的一个坐标点
#[derive(Debug, Clone)]
pub struct OsmNode {
    pub id: i64,
    pub lat: f64,
    pub lon: f64,
    pub tags: Vec<(String, String)>,
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

impl PartialEq for SpatialEntry {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && (self.min_lon - other.min_lon).abs() < 1e-10
            && (self.min_lat - other.min_lat).abs() < 1e-10
            && (self.max_lon - other.max_lon).abs() < 1e-10
            && (self.max_lat - other.max_lat).abs() < 1e-10
    }
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
    /// 本地 ID 生成器（负数 ID，用于新创建的要素）
    next_local_id: AtomicI64,
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
            next_local_id: AtomicI64::new(-1),
        }
    }

    /// 生成新的本地 ID（负数，用于未提交到服务器的新要素）
    pub fn generate_local_id(&self) -> i64 {
        self.next_local_id.fetch_sub(1, Ordering::SeqCst)
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
            .filter_map(|entry| self.nodes.get(&entry.id).map(|n| n.clone()))
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

    /// 更新节点坐标并维护 R-Tree 索引
    ///
    /// 这是移动节点的核心操作，必须同时更新：
    /// 1. DashMap 中节点的坐标
    /// 2. R-Tree 中节点的索引
    /// 3. R-Tree 中所有引用该节点的 Way 的边界框
    pub fn update_node_position(&self, node_id: i64, new_lon: f64, new_lat: f64) -> bool {
        // 1. 更新 DashMap 中的节点坐标
        let old_entry = {
            let mut node = match self.nodes.get_mut(&node_id) {
                Some(n) => n,
                None => return false,
            };
            let old_lon = node.lon;
            let old_lat = node.lat;
            node.lon = new_lon;
            node.lat = new_lat;
            SpatialEntry {
                id: node_id,
                min_lon: old_lon,
                min_lat: old_lat,
                max_lon: old_lon,
                max_lat: old_lat,
            }
        };

        // 2. 更新节点 R-Tree 索引
        let new_entry = SpatialEntry {
            id: node_id,
            min_lon: new_lon,
            min_lat: new_lat,
            max_lon: new_lon,
            max_lat: new_lat,
        };

        if let Ok(mut index) = self.node_index.write() {
            index.remove(&old_entry);
            index.insert(new_entry);
        }

        // 3. 找出所有引用该节点的 Way，更新它们在 R-Tree 中的边界框
        let affected_ways: Vec<i64> = self
            .ways
            .iter()
            .filter(|entry| entry.value().node_refs.contains(&node_id))
            .map(|entry| *entry.key())
            .collect();

        if !affected_ways.is_empty() {
            if let Ok(mut way_index) = self.way_index.write() {
                for way_id in affected_ways {
                    if let Some(way) = self.ways.get(&way_id) {
                        // 计算旧的边界框（用于删除）
                        // 注意：由于节点坐标已更新，我们无法精确获取旧边界框
                        // 所以我们使用 retain 方法按 ID 删除
                        let entries_to_remove: Vec<_> = way_index
                            .iter()
                            .filter(|e| e.id == way_id)
                            .cloned()
                            .collect();
                        
                        for entry in entries_to_remove {
                            way_index.remove(&entry);
                        }

                        // 计算新的边界框并插入
                        if let Some(new_bbox) = self.compute_way_bbox(&way) {
                            way_index.insert(new_bbox);
                        }
                    }
                }
            }
        }

        true
    }

    /// 查找所有引用指定节点的 Way ID
    pub fn find_ways_referencing_node(&self, node_id: i64) -> Vec<i64> {
        self.ways
            .iter()
            .filter(|entry| entry.value().node_refs.contains(&node_id))
            .map(|entry| *entry.key())
            .collect()
    }

    /// 添加节点并更新 R-Tree 索引
    pub fn add_node_with_index(&self, node: OsmNode) {
        let entry = SpatialEntry {
            id: node.id,
            min_lon: node.lon,
            min_lat: node.lat,
            max_lon: node.lon,
            max_lat: node.lat,
        };

        self.nodes.insert(node.id, node);

        if let Ok(mut index) = self.node_index.write() {
            index.insert(entry);
        }
    }

    /// 删除节点并更新 R-Tree 索引
    pub fn remove_node_with_index(&self, node_id: i64) -> Option<OsmNode> {
        let removed = self.nodes.remove(&node_id);

        if let Some((_, ref node)) = removed {
            let entry = SpatialEntry {
                id: node_id,
                min_lon: node.lon,
                min_lat: node.lat,
                max_lon: node.lon,
                max_lat: node.lat,
            };

            if let Ok(mut index) = self.node_index.write() {
                index.remove(&entry);
            }
        }

        removed.map(|(_, n)| n)
    }

    /// 添加 Way 并更新 R-Tree 索引和节点引用计数
    pub fn add_way_with_index(&self, way: OsmWay) {
        // 更新节点引用计数
        for &node_id in &way.node_refs {
            self.node_ref_count
                .entry(node_id)
                .and_modify(|c| *c = c.saturating_add(1))
                .or_insert(1);
        }

        // 计算边界框并插入 R-Tree
        if let Some(bbox) = self.compute_way_bbox(&way) {
            if let Ok(mut index) = self.way_index.write() {
                index.insert(bbox);
            }
        }

        self.ways.insert(way.id, way);
    }

    /// 删除 Way 并更新 R-Tree 索引和节点引用计数
    pub fn remove_way_with_index(&self, way_id: i64) -> Option<OsmWay> {
        let removed = self.ways.remove(&way_id);

        if let Some((_, ref way)) = removed {
            // 减少节点引用计数
            for &node_id in &way.node_refs {
                self.node_ref_count.entry(node_id).and_modify(|c| {
                    *c = c.saturating_sub(1);
                });
            }

            // 从 R-Tree 移除
            if let Ok(mut index) = self.way_index.write() {
                let entries_to_remove: Vec<_> = index
                    .iter()
                    .filter(|e| e.id == way_id)
                    .cloned()
                    .collect();

                for entry in entries_to_remove {
                    index.remove(&entry);
                }
            }
        }

        removed.map(|(_, w)| w)
    }

    /// 从 Way 中移除指定节点引用，返回被移除的索引位置列表
    pub fn remove_node_from_way(&self, way_id: i64, node_id: i64) -> Vec<usize> {
        let mut removed_indices = Vec::new();

        if let Some(mut way) = self.ways.get_mut(&way_id) {
            // 记录所有需要移除的位置
            let indices: Vec<usize> = way
                .node_refs
                .iter()
                .enumerate()
                .filter(|(_, &id)| id == node_id)
                .map(|(i, _)| i)
                .collect();

            // 从后往前删除，避免索引位移问题
            for &idx in indices.iter().rev() {
                way.node_refs.remove(idx);
            }

            removed_indices = indices;
        }

        // 减少节点引用计数
        if !removed_indices.is_empty() {
            self.node_ref_count.entry(node_id).and_modify(|c| {
                *c = c.saturating_sub(removed_indices.len() as u16);
            });

            // 更新 Way 的 R-Tree 边界框
            self.update_way_rtree(way_id);
        }

        removed_indices
    }

    /// 在 Way 的指定位置插入节点引用
    pub fn insert_node_to_way(&self, way_id: i64, node_id: i64, indices: &[usize]) {
        if let Some(mut way) = self.ways.get_mut(&way_id) {
            // 从前往后插入，需要考虑索引位移
            for (offset, &idx) in indices.iter().enumerate() {
                let insert_pos = idx + offset;
                if insert_pos <= way.node_refs.len() {
                    way.node_refs.insert(insert_pos, node_id);
                }
            }
        }

        // 增加节点引用计数
        if !indices.is_empty() {
            self.node_ref_count
                .entry(node_id)
                .and_modify(|c| *c = c.saturating_add(indices.len() as u16))
                .or_insert(indices.len() as u16);

            // 更新 Way 的 R-Tree 边界框
            self.update_way_rtree(way_id);
        }
    }

    /// 更新 Way 的 R-Tree 边界框
    fn update_way_rtree(&self, way_id: i64) {
        if let Ok(mut index) = self.way_index.write() {
            // 移除旧的边界框
            let entries_to_remove: Vec<_> = index
                .iter()
                .filter(|e| e.id == way_id)
                .cloned()
                .collect();

            for entry in entries_to_remove {
                index.remove(&entry);
            }

            // 插入新的边界框
            if let Some(way) = self.ways.get(&way_id) {
                if let Some(bbox) = self.compute_way_bbox(&way) {
                    index.insert(bbox);
                }
            }
        }
    }

    /// 检查 Way 是否仍然有效（至少 2 个节点）
    pub fn is_way_valid(&self, way_id: i64) -> bool {
        self.ways
            .get(&way_id)
            .map(|w| w.node_refs.len() >= 2)
            .unwrap_or(false)
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

//! 空间查询与视口计算模块
//!
//! 职责：
//! - 视口坐标裁剪
//! - LOD (Level of Detail) 降级策略
//! - 瓦片分块计算
//! - Polygon 组装 (Area + Multipolygon)

use crate::osm_store::OsmStore;
use crate::polygon_assembler::{assemble_from_closed_way, AssembledPolygon};
// TODO: 后续添加 Relation 空间索引后启用
#[allow(unused_imports)]
use crate::polygon_assembler::assemble_from_relation;

/// 带有引用计数的节点数据 (用于渲染优先级)
#[derive(Debug, Clone, Copy)]
pub struct NodeWithPriority {
    pub id: i64,
    pub lon: f64,
    pub lat: f64,
    pub ref_count: u16, // 被多少条 Way 引用
}

/// 视口定义 (WGS84 坐标系)
#[derive(Debug, Clone, Copy, serde::Deserialize)]
pub struct Viewport {
    pub min_lon: f64,
    pub min_lat: f64,
    pub max_lon: f64,
    pub max_lat: f64,
    pub zoom: f32,
}

impl Viewport {
    /// 计算视口面积 (用于判断是否需要 LOD 降级)
    pub fn area(&self) -> f64 {
        (self.max_lon - self.min_lon) * (self.max_lat - self.min_lat)
    }

    /// 判断是否需要降级 (视口过大时跳过小物体)
    pub fn needs_simplification(&self) -> bool {
        self.zoom < 14.0
    }

    /// 计算最小可见物体的像素阈值
    pub fn min_feature_size_deg(&self) -> f64 {
        match self.zoom as u32 {
            0..=10 => 0.01,
            11..=13 => 0.001,
            14..=16 => 0.0001,
            _ => 0.0,
        }
    }
}

/// 视口查询结果
#[derive(Debug)]
pub struct ViewportQueryResult {
    pub nodes: Vec<NodeWithPriority>,
    pub way_ids: Vec<i64>,
    pub polygons: Vec<AssembledPolygon>,
    pub truncated: bool,
}

/// 根据缩放级别确定渲染上限
fn get_render_limits(zoom: f32) -> (usize, usize) {
    match zoom as u32 {
        0..=8 => (10_000, 5_000),
        9..=11 => (30_000, 15_000),
        12..=14 => (80_000, 40_000),
        15..=17 => (150_000, 80_000),
        18..=21 => (300_000, 150_000),
        22..=24 => (500_000, 250_000),
        _ => (800_000, 400_000), // zoom 25+
    }
}

/// 节点 LOD 配置
#[derive(Debug, Clone, Copy)]
pub struct NodeLodConfig {
    /// 是否显示节点
    pub show_nodes: bool,
    /// 最小引用计数阈值 (0 = 显示所有，2 = 只显示连接点)
    pub min_ref_count: u16,
    /// 最大节点数
    pub max_nodes: usize,
}

fn get_node_lod_config(zoom: f32) -> NodeLodConfig {
    match zoom as u32 {
        // 低缩放 (0-17): 不显示节点
        0..=17 => NodeLodConfig {
            show_nodes: false,
            min_ref_count: 0,
            max_nodes: 0,
        },
        // 中缩放 (18-19): 只显示优先节点 (ref_count >= 2)
        18..=19 => NodeLodConfig {
            show_nodes: true,
            min_ref_count: 2,
            max_nodes: 50_000,
        },
        // 高缩放 (20-21): 优先节点 + 普通节点
        20..=21 => NodeLodConfig {
            show_nodes: true,
            min_ref_count: 0,
            max_nodes: 100_000,
        },
        // 超高缩放 (22+): 显示所有节点，更多数量
        _ => NodeLodConfig {
            show_nodes: true,
            min_ref_count: 0,
            max_nodes: 300_000,
        },
    }
}

/// 执行视口查询
pub fn query_viewport(store: &OsmStore, viewport: &Viewport) -> ViewportQueryResult {
    let (_, max_ways) = get_render_limits(viewport.zoom);
    let node_lod = get_node_lod_config(viewport.zoom);

    // 查询节点 (根据 LOD 配置)
    let mut nodes: Vec<NodeWithPriority> = if node_lod.show_nodes {
        let raw_nodes = store.query_nodes_in_viewport(
            viewport.min_lon,
            viewport.min_lat,
            viewport.max_lon,
            viewport.max_lat,
        );

        // 转换为带优先级的节点，并过滤低引用计数
        let mut prioritized: Vec<NodeWithPriority> = raw_nodes
            .iter()
            .filter_map(|node| {
                let ref_count = store
                    .node_ref_count
                    .get(&node.id)
                    .map(|r| *r)
                    .unwrap_or(0);

                if ref_count >= node_lod.min_ref_count {
                    Some(NodeWithPriority {
                        id: node.id,
                        lon: node.lon,
                        lat: node.lat,
                        ref_count,
                    })
                } else {
                    None
                }
            })
            .collect();

        // 按引用计数降序排序 (连接多路径的优先)
        prioritized.sort_by(|a, b| b.ref_count.cmp(&a.ref_count));
        prioritized
    } else {
        Vec::new()
    };

    // 查询路径
    let mut way_ids = store.query_way_ids_in_viewport(
        viewport.min_lon,
        viewport.min_lat,
        viewport.max_lon,
        viewport.max_lat,
    );

    let mut truncated = false;

    if nodes.len() > node_lod.max_nodes {
        nodes.truncate(node_lod.max_nodes);
        truncated = true;
    }

    if way_ids.len() > max_ways {
        way_ids.truncate(max_ways);
        truncated = true;
    }

    // 分离 Area Way 和普通 Way
    let mut line_way_ids: Vec<i64> = Vec::with_capacity(way_ids.len());
    let mut area_way_ids: Vec<i64> = Vec::new();

    for &way_id in &way_ids {
        if let Some(way) = store.ways.get(&way_id) {
            if way.is_area {
                area_way_ids.push(way_id);
            } else {
                line_way_ids.push(way_id);
            }
        }
    }

    // 组装 Polygon
    let mut polygons: Vec<AssembledPolygon> = Vec::with_capacity(area_way_ids.len());

    // 1. 从闭合 Area Way 组装
    for way_id in area_way_ids {
        if let Some(polygon) = assemble_from_closed_way(store, way_id) {
            polygons.push(polygon);
        }
    }

    // 2. 从 Multipolygon Relation 组装 (TODO: 需要空间索引 Relation)
    // 目前暂时跳过 Relation，后续可以添加

    ViewportQueryResult {
        nodes,
        way_ids: line_way_ids,
        polygons,
        truncated,
    }
}

/// 瓦片坐标 (用于分块加载)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileCoord {
    pub x: u32,
    pub y: u32,
    pub z: u8,
}

impl TileCoord {
    /// 从经纬度计算瓦片坐标
    pub fn from_lonlat(lon: f64, lat: f64, zoom: u8) -> Self {
        let n = 2_u32.pow(zoom as u32);
        let x = ((lon + 180.0) / 360.0 * n as f64).floor() as u32;
        let lat_rad = lat.to_radians();
        let y = ((1.0 - lat_rad.tan().asinh() / std::f64::consts::PI) / 2.0 * n as f64).floor()
            as u32;
        Self {
            x: x.min(n - 1),
            y: y.min(n - 1),
            z: zoom,
        }
    }

    /// 计算瓦片的经纬度边界
    pub fn to_bbox(&self) -> (f64, f64, f64, f64) {
        let n = 2_u32.pow(self.z as u32) as f64;
        let min_lon = self.x as f64 / n * 360.0 - 180.0;
        let max_lon = (self.x + 1) as f64 / n * 360.0 - 180.0;
        let max_lat = (std::f64::consts::PI * (1.0 - 2.0 * self.y as f64 / n))
            .sinh()
            .atan()
            .to_degrees();
        let min_lat = (std::f64::consts::PI * (1.0 - 2.0 * (self.y + 1) as f64 / n))
            .sinh()
            .atan()
            .to_degrees();
        (min_lon, min_lat, max_lon, max_lat)
    }
}

/// 计算覆盖视口的所有瓦片
pub fn tiles_in_viewport(viewport: &Viewport) -> Vec<TileCoord> {
    let zoom = (viewport.zoom.floor() as u8).clamp(0, 19);
    let top_left = TileCoord::from_lonlat(viewport.min_lon, viewport.max_lat, zoom);
    let bottom_right = TileCoord::from_lonlat(viewport.max_lon, viewport.min_lat, zoom);

    let mut tiles = Vec::new();
    for x in top_left.x..=bottom_right.x {
        for y in top_left.y..=bottom_right.y {
            tiles.push(TileCoord { x, y, z: zoom });
        }
    }
    tiles
}

// ============================================================================
// 空间拾取 (Feature Picking / Hit Test)
// ============================================================================

use crate::projection::mercator_to_lonlat;

/// 拾取结果类型
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", content = "id")]
pub enum PickedFeature {
    Node(i64),
    Way(i64),
    None,
}

/// 点到线段的最短距离（平方）
fn point_to_segment_distance_sq(
    px: f64,
    py: f64,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
) -> f64 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len_sq = dx * dx + dy * dy;

    if len_sq < 1e-10 {
        // 线段退化为点
        let dx = px - x1;
        let dy = py - y1;
        return dx * dx + dy * dy;
    }

    // 计算投影点参数 t
    let t = ((px - x1) * dx + (py - y1) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);

    // 投影点坐标
    let proj_x = x1 + t * dx;
    let proj_y = y1 + t * dy;

    let dx = px - proj_x;
    let dy = py - proj_y;
    dx * dx + dy * dy
}

/// 在点击位置查找最近的要素
///
/// 算法：
/// 1. 首先查找容差范围内的 Node（优先级最高，根据 zoom 过滤）
/// 2. 如果没有 Node，查找最近的 Way
///
/// 参数：
/// - merc_x, merc_y: 点击位置的墨卡托坐标（米）
/// - tolerance_meters: 拾取容差（米）
/// - zoom: 当前缩放级别，用于过滤节点显示
///
/// 节点可见性规则（与渲染一致）：
/// - 优先节点 (ref_count >= 2): zoom >= 18 时显示
/// - 普通节点 (ref_count < 2): zoom >= 20 时显示
pub fn pick_feature(
    store: &OsmStore,
    merc_x: f64,
    merc_y: f64,
    tolerance_meters: f64,
    zoom: f64,
) -> PickedFeature {
    use crate::projection::lonlat_to_mercator;
    use rstar::AABB;

    // 转换为经纬度用于 R-Tree 查询
    let (click_lon, click_lat) = mercator_to_lonlat(merc_x, merc_y);

    // 计算容差对应的经纬度范围（近似）
    // 在赤道，1度 ≈ 111320米
    let meters_per_degree = 111320.0;
    let tolerance_deg = tolerance_meters / meters_per_degree;

    let tolerance_sq = tolerance_meters * tolerance_meters;

    // 1. 优先查找 Node
    let search_bbox = AABB::from_corners(
        [click_lon - tolerance_deg, click_lat - tolerance_deg],
        [click_lon + tolerance_deg, click_lat + tolerance_deg],
    );

    let node_index = store.node_index();
    let mut closest_node: Option<(i64, f64)> = None;

    for entry in node_index.locate_in_envelope(&search_bbox) {
        if let Some(node) = store.nodes.get(&entry.id) {
            // 根据 zoom 级别过滤节点（与渲染逻辑一致）
            let ref_count = store
                .node_ref_count
                .get(&entry.id)
                .map(|r| *r)
                .unwrap_or(0);
            let is_high_priority = ref_count >= 2;

            // 优先节点: zoom >= 18 时可见
            // 普通节点: zoom >= 20 时可见
            let is_visible = if is_high_priority {
                zoom >= 18.0
            } else {
                zoom >= 20.0
            };

            if !is_visible {
                continue;
            }

            let (node_mx, node_my) = lonlat_to_mercator(node.lon, node.lat);
            let dx = node_mx - merc_x;
            let dy = node_my - merc_y;
            let dist_sq = dx * dx + dy * dy;

            if dist_sq <= tolerance_sq {
                if closest_node.is_none() || dist_sq < closest_node.unwrap().1 {
                    closest_node = Some((entry.id, dist_sq));
                }
            }
        }
    }

    if let Some((node_id, _)) = closest_node {
        return PickedFeature::Node(node_id);
    }

    // 2. 查找 Way
    // Way 的 R-Tree 存储的是 Way 的包围盒
    // 使用一个非常小的查询框来查找"包含点击点"的所有 Way 包围盒
    let way_index = store.way_index();
    let mut closest_way: Option<(i64, f64)> = None;

    // 使用一个极小的搜索框来查找包含该点的所有 Way
    let tiny_eps = 1e-9;
    let click_box = AABB::from_corners(
        [click_lon - tiny_eps, click_lat - tiny_eps],
        [click_lon + tiny_eps, click_lat + tiny_eps],
    );

    for entry in way_index.locate_in_envelope_intersecting(&click_box) {
        if let Some(way) = store.ways.get(&entry.id) {

            // 计算点击位置到 Way 的最短距离
            let node_refs = &way.node_refs;
            if node_refs.len() < 2 {
                continue;
            }

            let mut min_dist_sq = f64::MAX;

            for i in 0..node_refs.len() - 1 {
                let n1 = store.nodes.get(&node_refs[i]);
                let n2 = store.nodes.get(&node_refs[i + 1]);

                if let (Some(n1), Some(n2)) = (n1, n2) {
                    let (mx1, my1) = lonlat_to_mercator(n1.lon, n1.lat);
                    let (mx2, my2) = lonlat_to_mercator(n2.lon, n2.lat);

                    let dist_sq = point_to_segment_distance_sq(merc_x, merc_y, mx1, my1, mx2, my2);

                    if dist_sq < min_dist_sq {
                        min_dist_sq = dist_sq;
                    }
                }
            }

            if min_dist_sq <= tolerance_sq {
                if closest_way.is_none() || min_dist_sq < closest_way.unwrap().1 {
                    closest_way = Some((entry.id, min_dist_sq));
                }
            }
        }
    }

    if let Some((way_id, _)) = closest_way {
        return PickedFeature::Way(way_id);
    }

    PickedFeature::None
}

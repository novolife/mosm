//! 空间查询与视口计算模块
//!
//! 职责：
//! - 视口坐标裁剪
//! - LOD (Level of Detail) 降级策略
//! - 瓦片分块计算

use crate::osm_store::OsmStore;

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
    pub truncated: bool,
}

/// 根据缩放级别确定渲染上限
fn get_render_limits(zoom: f32) -> (usize, usize) {
    match zoom as u32 {
        0..=8 => (10_000, 5_000),
        9..=11 => (30_000, 15_000),
        12..=14 => (80_000, 40_000),
        15..=17 => (150_000, 80_000),
        _ => (300_000, 150_000),
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
        // 低缩放 (0-16): 不显示节点
        0..=16 => NodeLodConfig {
            show_nodes: false,
            min_ref_count: 0,
            max_nodes: 0,
        },
        // 中缩放 (17-18): 只显示优先节点 (ref_count >= 2)
        17..=18 => NodeLodConfig {
            show_nodes: true,
            min_ref_count: 2,
            max_nodes: 50_000,
        },
        // 高缩放 (19-20): 显示所有节点
        19..=20 => NodeLodConfig {
            show_nodes: true,
            min_ref_count: 0,
            max_nodes: 100_000,
        },
        // 超高缩放 (21+): 显示所有节点，更多数量
        _ => NodeLodConfig {
            show_nodes: true,
            min_ref_count: 0,
            max_nodes: 200_000,
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

    ViewportQueryResult {
        nodes,
        way_ids,
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

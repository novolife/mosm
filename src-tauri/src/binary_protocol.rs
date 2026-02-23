//! 二进制协议层
//!
//! 将 Rust 数据结构高效序列化为字节流，供前端直接通过 ArrayBuffer 消费。
//! 设计原则：零拷贝、固定长度、可直接映射为 TypedArray。
//!
//! ## 节点序列化格式 (V4: 带 ID + 优先级)
//!
//! ```text
//! [node_id: i64][x: f64][y: f64][ref_count: u16][_pad: u16][_pad2: u32] = 32 bytes per node
//! ```
//!
//! ## Way 几何序列化格式 (V3: 带 ID + RenderFeature + Z-Order)
//!
//! 专为前端 Canvas 渲染设计，**后端完成几何组装和 Z-Order 排序**。
//!
//! ```text
//! [total_ways: u32]
//! [way_id: i64][render_feature: u16][point_count: u32][x1: f64][y1: f64]...
//! ...
//! ```
//!
//! - `way_id`: 用于空间拾取后的高亮渲染
//! - `render_feature`: 低 8 位 = BaseType, 高 8 位 = Flags
//! - Ways 按 z_order 升序排列，确保正确的图层遮挡
//!
//! ## Polygon 几何序列化格式 (V1: 多环面)
//!
//! 用于 Area 和 Multipolygon，支持 clip + 双倍线宽内描边效果。
//!
//! ```text
//! [total_polygons: u32]
//! [render_feature: u16][ring_count: u16][point_count_ring1: u32][x,y...]
//!   [point_count_ring2: u32][x,y...]...
//! ...
//! ```
//!
//! - 第一个 Ring 是 outer（外环），后续是 inner（洞）
//! - 所有环必须闭合（首尾点相同）

use crate::osm_store::{OsmNode, OsmStore};
use crate::polygon_assembler::AssembledPolygon;
use crate::projection::lonlat_to_mercator;
use crate::render_feature::calculate_z_order;
use crate::spatial_query::NodeWithPriority;
use bytemuck::{Pod, Zeroable};

/// 节点的二进制表示 (24 字节，内存对齐)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct NodeBinary {
    pub id: i64,   // 8 bytes
    pub lon: f64,  // 8 bytes
    pub lat: f64,  // 8 bytes
}

impl From<&OsmNode> for NodeBinary {
    fn from(node: &OsmNode) -> Self {
        Self {
            id: node.id,
            lon: node.lon,
            lat: node.lat,
        }
    }
}

/// 带优先级的节点二进制表示 (32 字节，内存对齐)
/// 格式: [node_id: i64][x: f64][y: f64][ref_count: u16][_pad: u16][_pad2: u32]
/// 注意：x, y 是 Web 墨卡托投影坐标（单位：米）
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct NodePriorityBinary {
    pub node_id: i64,   // 8 bytes
    pub x: f64,         // 8 bytes - 墨卡托 X (米)
    pub y: f64,         // 8 bytes - 墨卡托 Y (米)
    pub ref_count: u16, // 2 bytes
    pub _pad: u16,      // 2 bytes padding
    pub _pad2: u32,     // 4 bytes padding (total = 32)
}

impl From<&NodeWithPriority> for NodePriorityBinary {
    fn from(node: &NodeWithPriority) -> Self {
        // 应用 Web 墨卡托投影
        let (x, y) = lonlat_to_mercator(node.lon, node.lat);
        Self {
            node_id: node.id,
            x,
            y,
            ref_count: node.ref_count,
            _pad: 0,
            _pad2: 0,
        }
    }
}

/// 将节点数组序列化为字节流
pub fn encode_nodes(nodes: &[OsmNode]) -> Vec<u8> {
    let binary_nodes: Vec<NodeBinary> = nodes.iter().map(NodeBinary::from).collect();
    bytemuck::cast_slice(&binary_nodes).to_vec()
}

/// 将带优先级的节点数组序列化为字节流
pub fn encode_priority_nodes(nodes: &[NodeWithPriority]) -> Vec<u8> {
    let binary_nodes: Vec<NodePriorityBinary> = nodes.iter().map(NodePriorityBinary::from).collect();
    bytemuck::cast_slice(&binary_nodes).to_vec()
}

/// 批量坐标编码 (只传输坐标，不传 ID，用于渲染层)
pub fn encode_coordinates(nodes: &[OsmNode]) -> Vec<u8> {
    let mut buffer = Vec::with_capacity(nodes.len() * 16);
    for node in nodes {
        buffer.extend_from_slice(&node.lon.to_le_bytes());
        buffer.extend_from_slice(&node.lat.to_le_bytes());
    }
    buffer
}

/// 紧凑型 Way 几何序列化（Web 墨卡托投影 + Z-Order 排序）
///
/// 后端完成几何组装：查询 Way 的 node_refs，从 DashMap 获取坐标，
/// **应用 Web 墨卡托投影**，**按 Z-Order 升序排序**，然后拍平为连续字节流。
/// 缺失的 Node 会被跳过（PBF 截断场景）。
///
/// 格式: [total_ways: u32][way_id: i64][render_feature: u16][point_count: u32][x,y coords...]...
///
/// Z-Order 排序确保：隧道 < 水系 < 普通道路 < 桥梁
pub fn encode_ways_geometry(store: &OsmStore, way_ids: &[i64]) -> Vec<u8> {
    // 第一步：收集所有有效的 Way 数据
    struct WayData {
        way_id: i64,
        render_feature: u16,
        z_order: i16,
        coords: Vec<(f64, f64)>,
    }

    let mut ways_data: Vec<WayData> = Vec::with_capacity(way_ids.len());

    for &way_id in way_ids {
        let way = match store.ways.get(&way_id) {
            Some(w) => w,
            None => continue,
        };

        // 收集有效坐标并应用投影
        let coords: Vec<(f64, f64)> = way
            .node_refs
            .iter()
            .filter_map(|node_id| {
                store.nodes.get(node_id).map(|n| lonlat_to_mercator(n.lon, n.lat))
            })
            .collect();

        // 至少需要 2 个点才能画线
        if coords.len() < 2 {
            continue;
        }

        let z_order = calculate_z_order(way.render_feature, way.layer);

        ways_data.push(WayData {
            way_id,
            render_feature: way.render_feature,
            z_order,
            coords,
        });
    }

    // 第二步：按 Z-Order 升序排序（先渲染的在底层）
    ways_data.sort_by_key(|w| w.z_order);

    // 第三步：序列化
    let mut buffer = Vec::with_capacity(4 + ways_data.len() * 64);

    // 写入 way_count
    buffer.extend_from_slice(&(ways_data.len() as u32).to_le_bytes());

    for way_data in ways_data {
        // 写入 Way ID (8 字节)
        buffer.extend_from_slice(&way_data.way_id.to_le_bytes());

        // 写入 RenderFeature (2 字节)
        buffer.extend_from_slice(&way_data.render_feature.to_le_bytes());

        // 写入点数量 (4 字节)
        buffer.extend_from_slice(&(way_data.coords.len() as u32).to_le_bytes());

        // 写入投影后的坐标（每点 16 字节：x f64 + y f64）
        for (x, y) in way_data.coords {
            buffer.extend_from_slice(&x.to_le_bytes());
            buffer.extend_from_slice(&y.to_le_bytes());
        }
    }

    buffer
}

/// Polygon 几何序列化（用于 Area 和 Multipolygon）
///
/// 格式: [polygon_count: u32]
///       [way_id: i64][render_feature: u16][ring_count: u16]
///       [point_count_ring1: u32][x,y coords...]
///       [point_count_ring2: u32][x,y coords...]...
///
/// 支持 clip + 双倍线宽的内向描边效果
pub fn encode_polygons_geometry(polygons: &[AssembledPolygon]) -> Vec<u8> {
    // 按 z_order 排序
    let mut sorted: Vec<&AssembledPolygon> = polygons.iter().collect();
    sorted.sort_by_key(|p| calculate_z_order(p.render_feature, p.layer));

    // 预估容量
    let estimated_size: usize = 4
        + sorted
            .iter()
            .map(|p| {
                12 + p.rings.iter().map(|r| 4 + r.len() * 16).sum::<usize>() // 8 (way_id) + 2 (feature) + 2 (ring_count)
            })
            .sum::<usize>();

    let mut buffer = Vec::with_capacity(estimated_size);

    // 写入 polygon_count
    buffer.extend_from_slice(&(sorted.len() as u32).to_le_bytes());

    for polygon in sorted {
        // 写入 Way ID (8 字节)
        buffer.extend_from_slice(&polygon.way_id.to_le_bytes());

        // 写入 RenderFeature (2 字节)
        buffer.extend_from_slice(&polygon.render_feature.to_le_bytes());

        // 写入 ring_count (2 字节)
        buffer.extend_from_slice(&(polygon.rings.len() as u16).to_le_bytes());

        // 写入每个环
        for ring in &polygon.rings {
            // 写入点数量 (4 字节)
            buffer.extend_from_slice(&(ring.len() as u32).to_le_bytes());

            // 写入坐标
            for &(x, y) in ring {
                buffer.extend_from_slice(&x.to_le_bytes());
                buffer.extend_from_slice(&y.to_le_bytes());
            }
        }
    }

    buffer
}

/// 响应头 (元数据) - 16 字节
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ViewportResponseHeader {
    pub node_count: u32,
    pub way_count: u32,
    pub polygon_count: u32,
    pub truncated: u32,
}

/// 构建完整的视口查询响应 (V4: 带节点优先级 + Polygon)
///
/// 格式:
/// ```text
/// [Header: 16 bytes]
/// [Nodes: node_count * 32 bytes]
/// [Way geometry: variable length]
/// [Polygon geometry: variable length]
/// ```
pub fn build_viewport_response_v4(
    store: &OsmStore,
    nodes: &[NodeWithPriority],
    way_ids: &[i64],
    polygons: &[AssembledPolygon],
    truncated: bool,
) -> Vec<u8> {
    let way_data = encode_ways_geometry(store, way_ids);
    let node_data = encode_priority_nodes(nodes);
    let polygon_data = encode_polygons_geometry(polygons);

    // 解析 way_data 获取实际的 way_count
    let actual_way_count = if way_data.len() >= 4 {
        u32::from_le_bytes([way_data[0], way_data[1], way_data[2], way_data[3]])
    } else {
        0
    };

    // 解析 polygon_data 获取实际的 polygon_count
    let actual_polygon_count = if polygon_data.len() >= 4 {
        u32::from_le_bytes([
            polygon_data[0],
            polygon_data[1],
            polygon_data[2],
            polygon_data[3],
        ])
    } else {
        0
    };

    let header = ViewportResponseHeader {
        node_count: nodes.len() as u32,
        way_count: actual_way_count,
        polygon_count: actual_polygon_count,
        truncated: if truncated { 1 } else { 0 },
    };

    let header_bytes = bytemuck::bytes_of(&header);

    let mut response = Vec::with_capacity(
        header_bytes.len() + node_data.len() + way_data.len() + polygon_data.len(),
    );

    // 1. Header
    response.extend_from_slice(header_bytes);

    // 2. Node data
    response.extend_from_slice(&node_data);

    // 3. Way geometry data
    response.extend_from_slice(&way_data);

    // 4. Polygon geometry data
    response.extend_from_slice(&polygon_data);

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_binary_size() {
        assert_eq!(std::mem::size_of::<NodeBinary>(), 24);
    }

    #[test]
    fn test_header_size() {
        assert_eq!(std::mem::size_of::<ViewportResponseHeader>(), 16);
    }

    #[test]
    fn test_encode_nodes() {
        let nodes = vec![
            OsmNode { id: 1, lat: 51.5074, lon: -0.1278 },
            OsmNode { id: 2, lat: 48.8566, lon: 2.3522 },
        ];
        let bytes = encode_nodes(&nodes);
        assert_eq!(bytes.len(), 48);
    }

    #[test]
    fn test_encode_ways_geometry_empty() {
        let store = OsmStore::new();
        let result = encode_ways_geometry(&store, &[]);
        assert_eq!(result.len(), 4);
        assert_eq!(u32::from_le_bytes([result[0], result[1], result[2], result[3]]), 0);
    }
}

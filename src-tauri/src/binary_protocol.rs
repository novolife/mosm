//! 二进制协议层
//!
//! 将 Rust 数据结构高效序列化为字节流，供前端直接通过 ArrayBuffer 消费。
//! 设计原则：零拷贝、固定长度、可直接映射为 TypedArray。
//!
//! ## 节点序列化格式 (V3: 带优先级)
//!
//! ```text
//! [lon: f64][lat: f64][ref_count: u16][_pad: u16] = 20 bytes per node
//! ```
//!
//! ## Way 几何序列化格式 (紧凑型)
//!
//! 专为前端 Canvas 渲染设计，**后端完成几何组装**，前端零对象分配。
//!
//! ```text
//! [total_ways: u32]
//! [way_1_point_count: u32][x1: f64][y1: f64][x2: f64][y2: f64]...
//! [way_2_point_count: u32][x1: f64][y1: f64]...
//! ...
//! ```

use crate::osm_store::{OsmNode, OsmStore};
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

/// 带优先级的节点二进制表示 (24 字节，内存对齐)
/// 格式: [lon: f64][lat: f64][ref_count: u16][_pad: u16][_pad2: u32]
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct NodePriorityBinary {
    pub lon: f64,       // 8 bytes
    pub lat: f64,       // 8 bytes
    pub ref_count: u16, // 2 bytes
    pub _pad: u16,      // 2 bytes padding
    pub _pad2: u32,     // 4 bytes padding (total = 24)
}

impl From<&NodeWithPriority> for NodePriorityBinary {
    fn from(node: &NodeWithPriority) -> Self {
        Self {
            lon: node.lon,
            lat: node.lat,
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

/// 紧凑型 Way 几何序列化
///
/// 后端完成几何组装：查询 Way 的 node_refs，从 DashMap 获取坐标，拍平为连续字节流。
/// 缺失的 Node 会被跳过（PBF 截断场景）。
///
/// 格式: [total_ways: u32][way_1_point_count: u32][coords...][way_2_point_count: u32][coords...]...
pub fn encode_ways_geometry(store: &OsmStore, way_ids: &[i64]) -> Vec<u8> {
    let mut buffer = Vec::with_capacity(4 + way_ids.len() * 64);
    let mut valid_way_count: u32 = 0;

    // 预留 way_count 位置
    let way_count_pos = buffer.len();
    buffer.extend_from_slice(&0u32.to_le_bytes());

    for &way_id in way_ids {
        let way = match store.ways.get(&way_id) {
            Some(w) => w,
            None => continue,
        };

        // 收集有效坐标
        let coords: Vec<(f64, f64)> = way
            .node_refs
            .iter()
            .filter_map(|node_id| {
                store.nodes.get(node_id).map(|n| (n.lon, n.lat))
            })
            .collect();

        // 至少需要 2 个点才能画线
        if coords.len() < 2 {
            continue;
        }

        // 写入点数量
        buffer.extend_from_slice(&(coords.len() as u32).to_le_bytes());

        // 写入坐标
        for (lon, lat) in coords {
            buffer.extend_from_slice(&lon.to_le_bytes());
            buffer.extend_from_slice(&lat.to_le_bytes());
        }

        valid_way_count += 1;
    }

    // 回填实际的 way_count
    buffer[way_count_pos..way_count_pos + 4]
        .copy_from_slice(&valid_way_count.to_le_bytes());

    buffer
}

/// 响应头 (元数据) - 16 字节
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ViewportResponseHeader {
    pub node_count: u32,
    pub way_count: u32,
    pub truncated: u32,
    pub _reserved: u32,
}

/// 构建完整的视口查询响应 (V3: 带节点优先级)
///
/// 格式:
/// ```text
/// [Header: 16 bytes]
/// [Nodes: node_count * 24 bytes (lon, lat, ref_count, padding)]
/// [Way geometry: variable length, see encode_ways_geometry]
/// ```
pub fn build_viewport_response_v3(
    store: &OsmStore,
    nodes: &[NodeWithPriority],
    way_ids: &[i64],
    truncated: bool,
) -> Vec<u8> {
    let way_data = encode_ways_geometry(store, way_ids);
    let node_data = encode_priority_nodes(nodes);

    // 解析 way_data 获取实际的 way_count
    let actual_way_count = if way_data.len() >= 4 {
        u32::from_le_bytes([way_data[0], way_data[1], way_data[2], way_data[3]])
    } else {
        0
    };

    let header = ViewportResponseHeader {
        node_count: nodes.len() as u32,
        way_count: actual_way_count,
        truncated: if truncated { 1 } else { 0 },
        _reserved: 0,
    };

    let header_bytes = bytemuck::bytes_of(&header);

    let mut response = Vec::with_capacity(
        header_bytes.len() + node_data.len() + way_data.len()
    );

    // 1. Header
    response.extend_from_slice(header_bytes);

    // 2. Node data (lon, lat, ref_count, padding) - 24 bytes each
    response.extend_from_slice(&node_data);

    // 3. Way geometry data
    response.extend_from_slice(&way_data);

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

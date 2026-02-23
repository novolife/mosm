//! 多边形拓扑组装器
//!
//! 将 OSM 中散乱的 Way 拼接成完整的闭合环 (Rings)。
//! 处理 Area (闭合 Way) 和 Multipolygon Relation。
//!
//! ## 核心算法
//!
//! OSM Multipolygon 的 member 通常是无序的线段片段。
//! 算法需要：
//! 1. 收集所有 outer/inner member 的节点序列
//! 2. 通过端点匹配将片段拼接成闭合环
//! 3. 返回组装好的 Polygon 结构

use crate::osm_store::OsmStore;
use crate::projection::lonlat_to_mercator;
use std::collections::HashMap;

/// 组装好的多边形
#[derive(Debug, Clone)]
pub struct AssembledPolygon {
    /// 渲染特征
    pub render_feature: u16,
    /// 图层值
    pub layer: i8,
    /// 所有环（第一个是 outer，后续是 inner）
    /// 每个环是墨卡托坐标序列 [(x, y), ...]
    pub rings: Vec<Vec<(f64, f64)>>,
}

/// 从闭合 Way 创建简单多边形
pub fn assemble_from_closed_way(
    store: &OsmStore,
    way_id: i64,
) -> Option<AssembledPolygon> {
    let way = store.ways.get(&way_id)?;

    // 检查是否闭合
    if way.node_refs.len() < 4 {
        return None;
    }
    if way.node_refs.first() != way.node_refs.last() {
        return None;
    }

    // 收集坐标并投影
    let coords: Vec<(f64, f64)> = way
        .node_refs
        .iter()
        .filter_map(|node_id| {
            store.nodes.get(node_id).map(|n| lonlat_to_mercator(n.lon, n.lat))
        })
        .collect();

    // 至少需要 4 个点（包含闭合点）
    if coords.len() < 4 {
        return None;
    }

    Some(AssembledPolygon {
        render_feature: way.render_feature,
        layer: way.layer,
        rings: vec![coords],
    })
}

/// 从 Multipolygon Relation 组装多边形
///
/// 这是核心算法：将散乱的 Way 片段拼接成闭合环
pub fn assemble_from_relation(
    store: &OsmStore,
    relation_id: i64,
) -> Option<AssembledPolygon> {
    use crate::osm_store::MemberType;
    use crate::render_feature::parse_tags;

    let relation = store.relations.get(&relation_id)?;

    // 检查是否是 multipolygon 类型
    let is_multipolygon = relation
        .tags
        .iter()
        .any(|(k, v)| k == "type" && v == "multipolygon");

    if !is_multipolygon {
        return None;
    }

    // 解析 relation 的渲染特征
    let parsed = parse_tags(&relation.tags);

    // 收集 outer 和 inner 成员
    let mut outer_ways: Vec<i64> = Vec::new();
    let mut inner_ways: Vec<i64> = Vec::new();

    for member in &relation.members {
        if member.member_type != MemberType::Way {
            continue;
        }
        match member.role.as_str() {
            "outer" | "" => outer_ways.push(member.ref_id),
            "inner" => inner_ways.push(member.ref_id),
            _ => {}
        }
    }

    // 组装 outer 环
    let outer_rings = stitch_ways_to_rings(store, &outer_ways);
    if outer_rings.is_empty() {
        return None;
    }

    // 组装 inner 环
    let inner_rings = stitch_ways_to_rings(store, &inner_ways);

    // 合并：outer 在前，inner 在后
    let mut rings = outer_rings;
    rings.extend(inner_rings);

    Some(AssembledPolygon {
        render_feature: parsed.feature,
        layer: parsed.layer,
        rings,
    })
}

/// 核心拓扑拼接算法
///
/// 将多条可能首尾相连的 Way 拼接成闭合环
fn stitch_ways_to_rings(store: &OsmStore, way_ids: &[i64]) -> Vec<Vec<(f64, f64)>> {
    if way_ids.is_empty() {
        return Vec::new();
    }

    // 收集所有 Way 的节点序列
    let mut segments: Vec<Vec<i64>> = Vec::new();
    for &way_id in way_ids {
        if let Some(way) = store.ways.get(&way_id) {
            if way.node_refs.len() >= 2 {
                segments.push(way.node_refs.clone());
            }
        }
    }

    if segments.is_empty() {
        return Vec::new();
    }

    // 构建端点索引: node_id -> [(segment_idx, is_start)]
    let mut endpoint_index: HashMap<i64, Vec<(usize, bool)>> = HashMap::new();
    for (idx, seg) in segments.iter().enumerate() {
        let start = *seg.first().unwrap();
        let end = *seg.last().unwrap();
        endpoint_index.entry(start).or_default().push((idx, true));
        endpoint_index.entry(end).or_default().push((idx, false));
    }

    // 标记已使用的片段
    let mut used: Vec<bool> = vec![false; segments.len()];
    let mut rings: Vec<Vec<(f64, f64)>> = Vec::new();

    // 贪婪拼接
    for start_idx in 0..segments.len() {
        if used[start_idx] {
            continue;
        }

        let mut current_ring: Vec<i64> = Vec::new();
        let mut current_idx = start_idx;
        let mut forward = true; // 当前片段的遍历方向

        loop {
            used[current_idx] = true;
            let seg = &segments[current_idx];

            // 添加节点（根据方向）
            if forward {
                if current_ring.is_empty() {
                    current_ring.extend(seg.iter().cloned());
                } else {
                    // 跳过第一个节点（与上一段末尾重复）
                    current_ring.extend(seg.iter().skip(1).cloned());
                }
            } else {
                if current_ring.is_empty() {
                    current_ring.extend(seg.iter().rev().cloned());
                } else {
                    current_ring.extend(seg.iter().rev().skip(1).cloned());
                }
            }

            // 检查是否闭合
            if current_ring.len() >= 4 && current_ring.first() == current_ring.last() {
                break;
            }

            // 查找下一个片段
            let tail = *current_ring.last().unwrap();
            let mut found_next = false;

            if let Some(candidates) = endpoint_index.get(&tail) {
                for &(seg_idx, is_start) in candidates {
                    if !used[seg_idx] {
                        current_idx = seg_idx;
                        forward = is_start; // 如果匹配的是起点，正向遍历
                        found_next = true;
                        break;
                    }
                }
            }

            if !found_next {
                // 无法继续拼接，放弃这个环
                break;
            }
        }

        // 检查是否成功闭合
        if current_ring.len() >= 4 && current_ring.first() == current_ring.last() {
            // 转换为墨卡托坐标
            let coords: Vec<(f64, f64)> = current_ring
                .iter()
                .filter_map(|node_id| {
                    store
                        .nodes
                        .get(node_id)
                        .map(|n| lonlat_to_mercator(n.lon, n.lat))
                })
                .collect();

            if coords.len() >= 4 {
                rings.push(coords);
            }
        }
    }

    rings
}

/// 判断一个 Way 是否应该被视为 Area（闭合多边形）
pub fn is_area_way(tags: &[(String, String)], node_refs: &[i64]) -> bool {
    // 首先检查是否闭合
    if node_refs.len() < 4 {
        return false;
    }
    if node_refs.first() != node_refs.last() {
        return false;
    }

    // 检查标签是否表明这是一个 Area
    for (key, value) in tags {
        match key.as_str() {
            // 明确是 area
            "area" => return value == "yes",
            // 隐含 area 的标签
            "building" | "landuse" | "leisure" | "amenity" | "shop" | "tourism" | "man_made" => {
                if value != "no" {
                    return true;
                }
            }
            "natural" => {
                // natural=coastline 不是 area
                if value != "no" && value != "coastline" && value != "tree_row" {
                    return true;
                }
            }
            "waterway" => {
                // waterway=riverbank 是 area
                if value == "riverbank" || value == "dock" || value == "boatyard" {
                    return true;
                }
            }
            _ => {}
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_area_building() {
        let tags = vec![("building".to_string(), "yes".to_string())];
        let refs = vec![1, 2, 3, 4, 1];
        assert!(is_area_way(&tags, &refs));
    }

    #[test]
    fn test_is_area_not_closed() {
        let tags = vec![("building".to_string(), "yes".to_string())];
        let refs = vec![1, 2, 3, 4, 5]; // 不闭合
        assert!(!is_area_way(&tags, &refs));
    }

    #[test]
    fn test_is_area_highway_not_area() {
        let tags = vec![("highway".to_string(), "primary".to_string())];
        let refs = vec![1, 2, 3, 4, 1];
        assert!(!is_area_way(&tags, &refs)); // 道路不是 area
    }

    #[test]
    fn test_is_area_explicit() {
        let tags = vec![
            ("highway".to_string(), "pedestrian".to_string()),
            ("area".to_string(), "yes".to_string()),
        ];
        let refs = vec![1, 2, 3, 4, 1];
        assert!(is_area_way(&tags, &refs)); // 明确标记为 area
    }
}

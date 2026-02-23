//! PBF 流式解析器
//!
//! 基于 osmpbf crate 实现流式解析，避免一次性加载整个文件到内存。
//! 支持多线程并行解析。

use crate::osm_store::{MemberType, OsmNode, OsmRelation, OsmStore, OsmWay, RelationMember};
use crate::polygon_assembler::is_area_way;
use crate::render_feature::parse_tags;
use anyhow::{Context, Result};
use osmpbf::{Element, ElementReader, RelMemberType};
use std::path::Path;
use std::sync::Arc;

/// 解析进度回调
pub type ProgressCallback = Box<dyn Fn(ParseProgress) + Send + Sync>;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ParseProgress {
    pub nodes_parsed: u64,
    pub ways_parsed: u64,
    pub relations_parsed: u64,
    pub bytes_read: u64,
    pub total_bytes: u64,
}

/// 转换 osmpbf 的 MemberType 到我们的 MemberType
fn convert_member_type(mt: RelMemberType) -> MemberType {
    match mt {
        RelMemberType::Node => MemberType::Node,
        RelMemberType::Way => MemberType::Way,
        RelMemberType::Relation => MemberType::Relation,
    }
}

/// 流式解析 PBF 文件
pub fn parse_pbf_file(path: &Path, store: Arc<OsmStore>) -> Result<ParseProgress> {
    let reader =
        ElementReader::from_path(path).with_context(|| format!("无法打开 PBF 文件: {:?}", path))?;

    let mut nodes_parsed: u64 = 0;
    let mut ways_parsed: u64 = 0;
    let mut relations_parsed: u64 = 0;

    reader
        .for_each(|element| match element {
            Element::Node(node) => {
                let tags: Vec<(String, String)> = node
                    .tags()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();
                let osm_node = OsmNode {
                    id: node.id(),
                    lat: node.lat(),
                    lon: node.lon(),
                    tags,
                };
                store.insert_node(osm_node);
                nodes_parsed += 1;
            }
            Element::DenseNode(node) => {
                let tags: Vec<(String, String)> = node
                    .tags()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();
                let osm_node = OsmNode {
                    id: node.id(),
                    lat: node.lat(),
                    lon: node.lon(),
                    tags,
                };
                store.insert_node(osm_node);
                nodes_parsed += 1;
            }
            Element::Way(way) => {
                let tags: Vec<(String, String)> = way
                    .tags()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();
                let node_refs: Vec<i64> = way.refs().collect();
                let parsed = parse_tags(&tags);
                let is_area = is_area_way(&tags, &node_refs);
                let osm_way = OsmWay {
                    id: way.id(),
                    node_refs,
                    tags,
                    render_feature: parsed.feature,
                    layer: parsed.layer,
                    is_area,
                };
                store.insert_way(osm_way);
                ways_parsed += 1;
            }
            Element::Relation(rel) => {
                let members = rel
                    .members()
                    .map(|m| {
                        let role = m.role().unwrap_or_default().to_string();
                        let ref_id = m.member_id;
                        RelationMember {
                            member_type: convert_member_type(m.member_type),
                            ref_id,
                            role,
                        }
                    })
                    .collect();
                let osm_relation = OsmRelation {
                    id: rel.id(),
                    members,
                    tags: rel
                        .tags()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect(),
                };
                store.relations.insert(osm_relation.id, osm_relation);
                relations_parsed += 1;
            }
        })
        .with_context(|| "PBF 解析过程中发生错误")?;

    Ok(ParseProgress {
        nodes_parsed,
        ways_parsed,
        relations_parsed,
        bytes_read: 0,
        total_bytes: 0,
    })
}

/// 并行解析 PBF 文件 (利用多核 CPU)
pub fn parse_pbf_parallel(path: &Path, store: Arc<OsmStore>) -> Result<ParseProgress> {
    let reader =
        ElementReader::from_path(path).with_context(|| format!("无法打开 PBF 文件: {:?}", path))?;

    let store_ref = &store;

    let (nodes, ways, relations) = reader
        .par_map_reduce(
            |element| {
                match element {
                    Element::Node(node) => {
                        let tags: Vec<(String, String)> = node
                            .tags()
                            .map(|(k, v)| (k.to_string(), v.to_string()))
                            .collect();
                        let osm_node = OsmNode {
                            id: node.id(),
                            lat: node.lat(),
                            lon: node.lon(),
                            tags,
                        };
                        store_ref.insert_node(osm_node);
                        (1u64, 0u64, 0u64)
                    }
                    Element::DenseNode(node) => {
                        let tags: Vec<(String, String)> = node
                            .tags()
                            .map(|(k, v)| (k.to_string(), v.to_string()))
                            .collect();
                        let osm_node = OsmNode {
                            id: node.id(),
                            lat: node.lat(),
                            lon: node.lon(),
                            tags,
                        };
                        store_ref.insert_node(osm_node);
                        (1, 0, 0)
                    }
                    Element::Way(way) => {
                        let tags: Vec<(String, String)> = way
                            .tags()
                            .map(|(k, v)| (k.to_string(), v.to_string()))
                            .collect();
                        let node_refs: Vec<i64> = way.refs().collect();
                        let parsed = parse_tags(&tags);
                        let is_area = is_area_way(&tags, &node_refs);
                        let osm_way = OsmWay {
                            id: way.id(),
                            node_refs,
                            tags,
                            render_feature: parsed.feature,
                            layer: parsed.layer,
                            is_area,
                        };
                        store_ref.insert_way(osm_way);
                        (0, 1, 0)
                    }
                    Element::Relation(rel) => {
                        let members = rel
                            .members()
                            .map(|m| {
                                let role = m.role().unwrap_or_default().to_string();
                                let ref_id = m.member_id;
                                RelationMember {
                                    member_type: convert_member_type(m.member_type),
                                    ref_id,
                                    role,
                                }
                            })
                            .collect();
                        let osm_relation = OsmRelation {
                            id: rel.id(),
                            members,
                            tags: rel
                                .tags()
                                .map(|(k, v)| (k.to_string(), v.to_string()))
                                .collect(),
                        };
                        store_ref.relations.insert(osm_relation.id, osm_relation);
                        (0, 0, 1)
                    }
                }
            },
            || (0u64, 0u64, 0u64),
            |a, b| (a.0 + b.0, a.1 + b.1, a.2 + b.2),
        )
        .with_context(|| "并行解析 PBF 时发生错误")?;

    // 批量重建空间索引 (比逐条插入快 100 倍)
    store.rebuild_indices();

    Ok(ParseProgress {
        nodes_parsed: nodes,
        ways_parsed: ways,
        relations_parsed: relations,
        bytes_read: 0,
        total_bytes: 0,
    })
}

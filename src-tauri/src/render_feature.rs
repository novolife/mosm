//! 渲染特征系统 (RenderFeature)
//!
//! 使用 u16 位掩码编码：
//! - 低 8 位 (0-7): BaseType - 基础地物类型
//! - 高 8 位 (8-15): Flags - 渲染修饰符
//!
//! 设计目标：
//! 1. 避免在渲染循环中传递字符串
//! 2. 支持图层排序 (Z-ordering)
//! 3. 支持特殊渲染效果 (桥梁边框、隧道虚线等)

/// RenderFeature 类型 (u16 位掩码)
pub type RenderFeature = u16;

// ============================================================================
// BaseType 常量 (低 8 位: 0x00 - 0xFF)
// ============================================================================

pub mod base_type {
    use super::RenderFeature;

    /// 默认/未分类
    pub const DEFAULT: RenderFeature = 0;

    // 道路系统 (1-19)
    /// 主要道路 (motorway, trunk, primary)
    pub const HIGHWAY_MAJOR: RenderFeature = 1;
    /// 次要道路 (secondary, tertiary)
    pub const HIGHWAY_MINOR: RenderFeature = 2;
    /// 普通道路 (residential, unclassified, service)
    pub const HIGHWAY_ROAD: RenderFeature = 3;
    /// 人行道路 (footway, path, pedestrian, cycleway)
    pub const HIGHWAY_PATH: RenderFeature = 4;
    /// 台阶 (steps)
    pub const HIGHWAY_STEPS: RenderFeature = 5;

    // 铁路系统 (20-29)
    /// 铁路干线
    pub const RAILWAY_MAIN: RenderFeature = 20;
    /// 轻轨/地铁
    pub const RAILWAY_LIGHT: RenderFeature = 21;

    // 水系 (30-39)
    /// 河流
    pub const WATERWAY_RIVER: RenderFeature = 30;
    /// 溪流
    pub const WATERWAY_STREAM: RenderFeature = 31;
    /// 运河/水渠
    pub const WATERWAY_CANAL: RenderFeature = 32;

    // 建筑 (40-49)
    /// 普通建筑
    pub const BUILDING: RenderFeature = 40;

    // 自然/土地利用 (50-69)
    /// 森林/树木
    pub const NATURAL_WOOD: RenderFeature = 50;
    /// 水域 (湖泊、海洋)
    pub const NATURAL_WATER: RenderFeature = 51;
    /// 草地
    pub const NATURAL_GRASS: RenderFeature = 52;
    /// 土地利用
    pub const LANDUSE: RenderFeature = 60;

    // 边界 (70-79)
    /// 行政边界
    pub const BOUNDARY: RenderFeature = 70;

    /// 从 RenderFeature 提取 BaseType
    #[inline]
    pub const fn extract(feature: RenderFeature) -> RenderFeature {
        feature & 0xFF
    }
}

// ============================================================================
// Flags 常量 (高 8 位: 0x0100 - 0x8000)
// ============================================================================

pub mod flags {
    use super::RenderFeature;

    /// 桥梁 (bridge=yes)
    pub const BRIDGE: RenderFeature = 0x0100;
    /// 隧道 (tunnel=yes)
    pub const TUNNEL: RenderFeature = 0x0200;
    /// 间歇性 (intermittent=yes，用于季节性河流)
    pub const INTERMITTENT: RenderFeature = 0x0400;
    /// 正在建设中 (construction=yes)
    pub const CONSTRUCTION: RenderFeature = 0x0800;
    /// 单行道 (oneway=yes)
    pub const ONEWAY: RenderFeature = 0x1000;

    /// 检查是否设置了指定 flag
    #[inline]
    pub const fn has(feature: RenderFeature, flag: RenderFeature) -> bool {
        (feature & flag) != 0
    }
}

// ============================================================================
// Z-Order 计算
// ============================================================================

/// 默认图层值 (OSM 中 layer 标签缺失时)
pub const DEFAULT_LAYER: i8 = 0;

/// 计算 Z-Order 值，用于渲染排序
///
/// 排序优先级（升序渲染，先画的在下面）:
/// 1. layer 值（-5 到 +5，隧道通常是负值）
/// 2. 隧道在同层中最先渲染
/// 3. 水系在道路下面
/// 4. 桥梁在同层中最后渲染
///
/// 返回值范围：约 -1000 到 +1000
pub fn calculate_z_order(feature: RenderFeature, layer: i8) -> i16 {
    let base_type = base_type::extract(feature);

    // layer 贡献 (每层 100 个单位的空间)
    let layer_z = (layer as i16) * 100;

    // 基础类型优先级 (同一 layer 内)
    let type_z: i16 = match base_type {
        // 水系最底层
        base_type::WATERWAY_RIVER | base_type::WATERWAY_STREAM | base_type::WATERWAY_CANAL => -30,
        base_type::NATURAL_WATER => -35,

        // 土地利用
        base_type::LANDUSE | base_type::NATURAL_GRASS => -20,
        base_type::NATURAL_WOOD => -15,

        // 建筑
        base_type::BUILDING => -10,

        // 道路系统
        base_type::HIGHWAY_PATH | base_type::HIGHWAY_STEPS => 0,
        base_type::HIGHWAY_ROAD => 5,
        base_type::HIGHWAY_MINOR => 10,
        base_type::HIGHWAY_MAJOR => 15,

        // 铁路
        base_type::RAILWAY_MAIN | base_type::RAILWAY_LIGHT => 20,

        // 边界在顶层
        base_type::BOUNDARY => 50,

        _ => 0,
    };

    // Flag 修饰符
    let flag_z: i16 = if flags::has(feature, flags::TUNNEL) {
        -40 // 隧道往下沉
    } else if flags::has(feature, flags::BRIDGE) {
        40 // 桥梁往上浮
    } else {
        0
    };

    layer_z + type_z + flag_z
}

// ============================================================================
// OSM Tags -> RenderFeature 映射
// ============================================================================

/// 解析结果：RenderFeature + layer 值
#[derive(Debug, Clone, Copy)]
pub struct ParsedFeature {
    pub feature: RenderFeature,
    pub layer: i8,
}

impl ParsedFeature {
    /// 计算 Z-Order
    pub fn z_order(&self) -> i16 {
        calculate_z_order(self.feature, self.layer)
    }
}

/// 从 OSM Tags 解析 RenderFeature
///
/// 返回 (RenderFeature, layer) 元组
pub fn parse_tags(tags: &[(String, String)]) -> ParsedFeature {
    if tags.is_empty() {
        return ParsedFeature {
            feature: base_type::DEFAULT,
            layer: DEFAULT_LAYER,
        };
    }

    let mut feature: RenderFeature = base_type::DEFAULT;
    let mut layer: i8 = DEFAULT_LAYER;

    // 一次遍历提取所有需要的信息
    let mut highway: Option<&str> = None;
    let mut railway: Option<&str> = None;
    let mut waterway: Option<&str> = None;
    let mut natural: Option<&str> = None;
    let mut building = false;
    let mut landuse = false;
    let mut boundary = false;

    let mut is_bridge = false;
    let mut is_tunnel = false;
    let mut is_intermittent = false;
    let mut is_construction = false;
    let mut is_oneway = false;

    for (key, value) in tags {
        match key.as_str() {
            "highway" => highway = Some(value.as_str()),
            "railway" => railway = Some(value.as_str()),
            "waterway" => waterway = Some(value.as_str()),
            "natural" => natural = Some(value.as_str()),
            "building" => building = value != "no",
            "landuse" => landuse = true,
            "boundary" => boundary = value != "no",
            "bridge" => is_bridge = value == "yes" || value == "viaduct" || value == "aqueduct",
            "tunnel" => is_tunnel = value == "yes" || value == "building_passage",
            "intermittent" => is_intermittent = value == "yes",
            "construction" => is_construction = !value.is_empty() && value != "no",
            "oneway" => is_oneway = value == "yes" || value == "-1",
            "layer" => {
                layer = value.parse().unwrap_or(DEFAULT_LAYER).clamp(-5, 5);
            }
            _ => {}
        }
    }

    // 按优先级确定 BaseType
    if let Some(ww) = waterway {
        feature = match ww {
            "river" => base_type::WATERWAY_RIVER,
            "stream" | "brook" => base_type::WATERWAY_STREAM,
            "canal" | "drain" | "ditch" => base_type::WATERWAY_CANAL,
            _ => base_type::WATERWAY_STREAM,
        };
    } else if let Some(nat) = natural {
        feature = match nat {
            "water" | "coastline" | "bay" => base_type::NATURAL_WATER,
            "wood" | "tree_row" | "scrub" => base_type::NATURAL_WOOD,
            "grassland" | "heath" => base_type::NATURAL_GRASS,
            _ => base_type::DEFAULT,
        };
    } else if let Some(rw) = railway {
        feature = match rw {
            "rail" | "preserved" => base_type::RAILWAY_MAIN,
            "light_rail" | "subway" | "tram" | "monorail" => base_type::RAILWAY_LIGHT,
            _ => base_type::RAILWAY_MAIN,
        };
    } else if let Some(hw) = highway {
        feature = match hw {
            "motorway" | "motorway_link" | "trunk" | "trunk_link" | "primary" | "primary_link" => {
                base_type::HIGHWAY_MAJOR
            }
            "secondary" | "secondary_link" | "tertiary" | "tertiary_link" => {
                base_type::HIGHWAY_MINOR
            }
            "residential" | "unclassified" | "service" | "living_street" | "road" => {
                base_type::HIGHWAY_ROAD
            }
            "footway" | "path" | "pedestrian" | "cycleway" | "bridleway" | "track" => {
                base_type::HIGHWAY_PATH
            }
            "steps" => base_type::HIGHWAY_STEPS,
            _ => base_type::HIGHWAY_ROAD,
        };
    } else if building {
        feature = base_type::BUILDING;
    } else if landuse {
        feature = base_type::LANDUSE;
    } else if boundary {
        feature = base_type::BOUNDARY;
    }

    // 设置 Flags
    if is_bridge {
        feature |= flags::BRIDGE;
    }
    if is_tunnel {
        feature |= flags::TUNNEL;
    }
    if is_intermittent {
        feature |= flags::INTERMITTENT;
    }
    if is_construction {
        feature |= flags::CONSTRUCTION;
    }
    if is_oneway {
        feature |= flags::ONEWAY;
    }

    ParsedFeature { feature, layer }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tags(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn test_highway_primary() {
        let tags = make_tags(&[("highway", "primary")]);
        let parsed = parse_tags(&tags);
        assert_eq!(base_type::extract(parsed.feature), base_type::HIGHWAY_MAJOR);
        assert!(!flags::has(parsed.feature, flags::BRIDGE));
    }

    #[test]
    fn test_bridge_flag() {
        let tags = make_tags(&[("highway", "secondary"), ("bridge", "yes")]);
        let parsed = parse_tags(&tags);
        assert_eq!(base_type::extract(parsed.feature), base_type::HIGHWAY_MINOR);
        assert!(flags::has(parsed.feature, flags::BRIDGE));
    }

    #[test]
    fn test_tunnel_flag() {
        let tags = make_tags(&[("highway", "primary"), ("tunnel", "yes"), ("layer", "-1")]);
        let parsed = parse_tags(&tags);
        assert!(flags::has(parsed.feature, flags::TUNNEL));
        assert_eq!(parsed.layer, -1);
    }

    #[test]
    fn test_z_order_tunnel_below_bridge() {
        let tunnel = parse_tags(&make_tags(&[("highway", "primary"), ("tunnel", "yes")]));
        let bridge = parse_tags(&make_tags(&[("highway", "primary"), ("bridge", "yes")]));
        let normal = parse_tags(&make_tags(&[("highway", "primary")]));

        assert!(tunnel.z_order() < normal.z_order());
        assert!(normal.z_order() < bridge.z_order());
    }

    #[test]
    fn test_z_order_water_below_road() {
        let water = parse_tags(&make_tags(&[("waterway", "river")]));
        let road = parse_tags(&make_tags(&[("highway", "residential")]));

        assert!(water.z_order() < road.z_order());
    }

    #[test]
    fn test_layer_effect() {
        let layer_neg1 = parse_tags(&make_tags(&[("highway", "primary"), ("layer", "-1")]));
        let layer_0 = parse_tags(&make_tags(&[("highway", "primary")]));
        let layer_1 = parse_tags(&make_tags(&[("highway", "primary"), ("layer", "1")]));

        assert!(layer_neg1.z_order() < layer_0.z_order());
        assert!(layer_0.z_order() < layer_1.z_order());
    }

    #[test]
    fn test_intermittent_stream() {
        let tags = make_tags(&[("waterway", "stream"), ("intermittent", "yes")]);
        let parsed = parse_tags(&tags);
        assert_eq!(
            base_type::extract(parsed.feature),
            base_type::WATERWAY_STREAM
        );
        assert!(flags::has(parsed.feature, flags::INTERMITTENT));
    }
}

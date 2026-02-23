/**
 * 渲染样式配置
 *
 * 此文件定义 OSM 地物类型到 Canvas 渲染样式的映射。
 * 与 Rust 端 render_feature.rs 中的常量保持同步。
 *
 * ## RenderFeature 结构 (u16 位掩码)
 * - 低 8 位 (0x00FF): BaseType - 基础地物类型
 * - 高 8 位 (0xFF00): Flags - 渲染修饰符
 */

// ============================================================================
// BaseType 常量 (与 Rust render_feature.rs 同步)
// ============================================================================

export const BaseType = {
  DEFAULT: 0,

  // 道路系统 (1-19)
  HIGHWAY_MAJOR: 1, // motorway, trunk, primary
  HIGHWAY_MINOR: 2, // secondary, tertiary
  HIGHWAY_ROAD: 3, // residential, unclassified, service
  HIGHWAY_PATH: 4, // footway, path, pedestrian, cycleway
  HIGHWAY_STEPS: 5, // steps

  // 铁路系统 (20-29)
  RAILWAY_MAIN: 20, // rail, preserved
  RAILWAY_LIGHT: 21, // light_rail, subway, tram

  // 水系 (30-39)
  WATERWAY_RIVER: 30, // river
  WATERWAY_STREAM: 31, // stream, brook
  WATERWAY_CANAL: 32, // canal, drain, ditch

  // 建筑 (40-49)
  BUILDING: 40,

  // 自然/土地利用 (50-69)
  NATURAL_WOOD: 50,
  NATURAL_WATER: 51,
  NATURAL_GRASS: 52,
  LANDUSE: 60,

  // 边界 (70-79)
  BOUNDARY: 70,
} as const

// ============================================================================
// Flags 常量 (与 Rust render_feature.rs 同步)
// ============================================================================

export const Flags = {
  BRIDGE: 0x0100,
  TUNNEL: 0x0200,
  INTERMITTENT: 0x0400,
  CONSTRUCTION: 0x0800,
  ONEWAY: 0x1000,
} as const

// ============================================================================
// 样式定义
// ============================================================================

/** Way 基础样式 */
export interface WayStyle {
  /** 线条颜色 */
  color: string
  /** 线条宽度 (会根据 zoom 缩放) */
  width: number
  /** 虚线模式 [实线长度, 间隙长度]，undefined 表示实线 */
  lineDash?: number[]
  /** 线端样式 */
  lineCap?: CanvasLineCap
  /** 连接样式 */
  lineJoin?: CanvasLineJoin
}

/** 带修饰符的完整样式 */
export interface ResolvedStyle extends WayStyle {
  /** 是否绘制边框（用于桥梁效果） */
  drawCasing?: boolean
  /** 边框颜色 */
  casingColor?: string
  /** 边框额外宽度（单侧） */
  casingWidth?: number
}

// ============================================================================
// BaseType -> 基础样式映射
// ============================================================================

const BASE_STYLES: Record<number, WayStyle> = {
  // 默认
  [BaseType.DEFAULT]: {
    color: '#666666',
    width: 1,
  },

  // === 道路系统 ===
  [BaseType.HIGHWAY_MAJOR]: {
    color: '#ffa726',
    width: 4,
    lineCap: 'round',
    lineJoin: 'round',
  },
  [BaseType.HIGHWAY_MINOR]: {
    color: '#ffcc80',
    width: 3,
    lineCap: 'round',
    lineJoin: 'round',
  },
  [BaseType.HIGHWAY_ROAD]: {
    color: '#ffffff',
    width: 2,
    lineCap: 'round',
    lineJoin: 'round',
  },
  [BaseType.HIGHWAY_PATH]: {
    color: '#b0bec5',
    width: 1.5,
    lineDash: [4, 2],
    lineCap: 'round',
  },
  [BaseType.HIGHWAY_STEPS]: {
    color: '#90a4ae',
    width: 2,
    lineDash: [2, 2],
    lineCap: 'butt',
  },

  // === 铁路系统 ===
  [BaseType.RAILWAY_MAIN]: {
    color: '#424242',
    width: 2.5,
    lineDash: [8, 4],
    lineCap: 'butt',
  },
  [BaseType.RAILWAY_LIGHT]: {
    color: '#616161',
    width: 2,
    lineDash: [6, 3],
    lineCap: 'butt',
  },

  // === 水系 ===
  [BaseType.WATERWAY_RIVER]: {
    color: '#42a5f5',
    width: 3,
    lineCap: 'round',
    lineJoin: 'round',
  },
  [BaseType.WATERWAY_STREAM]: {
    color: '#64b5f6',
    width: 1.5,
    lineCap: 'round',
  },
  [BaseType.WATERWAY_CANAL]: {
    color: '#4fc3f7',
    width: 2,
    lineCap: 'butt',
  },

  // === 建筑 ===
  [BaseType.BUILDING]: {
    color: '#d4a373',
    width: 1,
    lineJoin: 'miter',
  },

  // === 自然/土地 ===
  [BaseType.NATURAL_WOOD]: {
    color: '#66bb6a',
    width: 1,
  },
  [BaseType.NATURAL_WATER]: {
    color: '#29b6f6',
    width: 1,
  },
  [BaseType.NATURAL_GRASS]: {
    color: '#9ccc65',
    width: 1,
  },
  [BaseType.LANDUSE]: {
    color: '#c5e1a5',
    width: 1,
  },

  // === 边界 ===
  [BaseType.BOUNDARY]: {
    color: '#ef5350',
    width: 1.5,
    lineDash: [10, 5, 2, 5],
    lineCap: 'butt',
  },
}

// ============================================================================
// 样式解析函数
// ============================================================================

/** 从 RenderFeature 提取 BaseType */
export function extractBaseType(feature: number): number {
  return feature & 0xff
}

/** 检查是否设置了指定 Flag */
export function hasFlag(feature: number, flag: number): boolean {
  return (feature & flag) !== 0
}

/**
 * 解析 RenderFeature 为完整渲染样式
 *
 * 根据 BaseType 获取基础样式，然后应用 Flags 修饰符
 */
export function resolveStyle(feature: number): ResolvedStyle {
  const baseType = extractBaseType(feature)
  const baseStyle = BASE_STYLES[baseType] || BASE_STYLES[BaseType.DEFAULT]

  // 复制基础样式
  const style: ResolvedStyle = { ...baseStyle }

  // 应用 Flags 修饰符
  if (hasFlag(feature, Flags.TUNNEL)) {
    // 隧道：使用虚线表示
    style.lineDash = [6, 4]
    // 颜色变暗
    style.color = darkenColor(style.color, 0.3)
  }

  if (hasFlag(feature, Flags.BRIDGE)) {
    // 桥梁：添加边框效果
    style.drawCasing = true
    style.casingColor = '#37474f'
    style.casingWidth = 2
  }

  if (hasFlag(feature, Flags.INTERMITTENT)) {
    // 间歇性（季节性河流等）：细虚线
    style.lineDash = [4, 4]
  }

  if (hasFlag(feature, Flags.CONSTRUCTION)) {
    // 建设中：长虚线 + 颜色变淡
    style.lineDash = [10, 5]
    style.color = lightenColor(style.color, 0.3)
  }

  return style
}

// ============================================================================
// 颜色工具函数
// ============================================================================

/** 将十六进制颜色变暗 */
function darkenColor(hex: string, amount: number): string {
  const rgb = hexToRgb(hex)
  if (!rgb) return hex
  const r = Math.max(0, Math.floor(rgb.r * (1 - amount)))
  const g = Math.max(0, Math.floor(rgb.g * (1 - amount)))
  const b = Math.max(0, Math.floor(rgb.b * (1 - amount)))
  return rgbToHex(r, g, b)
}

/** 将十六进制颜色变亮 */
function lightenColor(hex: string, amount: number): string {
  const rgb = hexToRgb(hex)
  if (!rgb) return hex
  const r = Math.min(255, Math.floor(rgb.r + (255 - rgb.r) * amount))
  const g = Math.min(255, Math.floor(rgb.g + (255 - rgb.g) * amount))
  const b = Math.min(255, Math.floor(rgb.b + (255 - rgb.b) * amount))
  return rgbToHex(r, g, b)
}

function hexToRgb(hex: string): { r: number; g: number; b: number } | null {
  const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex)
  return result
    ? {
        r: parseInt(result[1], 16),
        g: parseInt(result[2], 16),
        b: parseInt(result[3], 16),
      }
    : null
}

function rgbToHex(r: number, g: number, b: number): string {
  return '#' + [r, g, b].map((x) => x.toString(16).padStart(2, '0')).join('')
}

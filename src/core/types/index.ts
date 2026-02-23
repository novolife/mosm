/**
 * 公共类型定义
 *
 * 集中管理跨模块共享的 TypeScript 类型
 */

// ============================================================================
// 视口与坐标
// ============================================================================

/** 视口定义 */
export interface Viewport {
  min_lon: number
  min_lat: number
  max_lon: number
  max_lat: number
  zoom: number
}

/** 数据边界 */
export interface DataBounds {
  min_lon: number
  min_lat: number
  max_lon: number
  max_lat: number
  center_lon: number
  center_lat: number
}

// ============================================================================
// 存储统计
// ============================================================================

/** 存储统计信息 */
export interface StoreStats {
  node_count: number
  way_count: number
  relation_count: number
}

/** 解析进度 */
export interface ParseProgress {
  nodes_parsed: number
  ways_parsed: number
  relations_parsed: number
  bytes_read: number
  total_bytes: number
}

// ============================================================================
// 视口响应数据
// ============================================================================

/** 响应头结构 (16 字节) */
export interface ResponseHeader {
  nodeCount: number
  wayCount: number
  polygonCount: number
  truncated: boolean
}

/** 节点数据 (带 ID 和优先级, 墨卡托坐标) */
export interface NodeData {
  nodeId: number
  x: number
  y: number
  refCount: number
}

/** V4 视口响应解码结果 */
export interface ViewportData {
  header: ResponseHeader
  nodes: NodeData[]
  wayGeometry: ArrayBuffer
  polygonGeometry: ArrayBuffer
}

// ============================================================================
// 拾取与详情
// ============================================================================

/** 拾取结果类型 */
export interface PickedFeature {
  type: 'Node' | 'Way' | 'None'
  id?: number
}

/** 所属关系信息 */
export interface ParentRelation {
  id: number
  role: string
  relation_type: string | null
  name: string | null
}

/** 节点详情 */
export interface NodeDetails {
  type: 'Node'
  id: number
  lon: number
  lat: number
  tags: [string, string][]
  ref_count: number
  parent_relations: ParentRelation[]
}

/** 路径详情 */
export interface WayDetails {
  type: 'Way'
  id: number
  tags: [string, string][]
  node_count: number
  is_area: boolean
  render_feature: number
  layer: number
  parent_relations: ParentRelation[]
}

/** 未找到 */
export interface NotFound {
  type: 'NotFound'
}

export type FeatureDetails = NodeDetails | WayDetails | NotFound

// ============================================================================
// 编辑操作
// ============================================================================

/** 标签更新结果 */
export interface UpdateTagsResult {
  success: boolean
  render_feature: number
  layer: number
  is_area: boolean
}

/** Undo/Redo 操作结果 */
export interface UndoRedoResult {
  success: boolean
  needs_redraw: boolean
  message: string | null
  undo_count: number
  redo_count: number
}

/** 移动节点结果 */
export interface MoveNodeResult {
  success: boolean
  message: string | null
}

/** 添加节点结果 */
export interface AddNodeResult {
  success: boolean
  node_id: number
  message: string | null
}

/** 删除要素结果 */
export interface DeleteFeatureResult {
  success: boolean
  message: string | null
  cascaded_way_ids: number[]
}

/**
 * IPC 通信桥接层
 *
 * 封装与 Tauri Rust 后端的二进制通信。
 * 设计原则：
 * - 使用 ArrayBuffer 而非 JSON 传输海量数据
 * - 提供类型安全的解码器
 * - 零拷贝语义
 */

import { invoke } from '@tauri-apps/api/core'

/** 视口定义 */
export interface Viewport {
  min_lon: number
  min_lat: number
  max_lon: number
  max_lat: number
  zoom: number
}

/** 存储统计信息 */
export interface StoreStats {
  node_count: number
  way_count: number
  relation_count: number
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

/** 解析进度 */
export interface ParseProgress {
  nodes_parsed: number
  ways_parsed: number
  relations_parsed: number
  bytes_read: number
  total_bytes: number
}

/** 响应头结构 (16 字节) */
export interface ResponseHeader {
  nodeCount: number
  wayCount: number
  polygonCount: number
  truncated: boolean
}

/** 解码后的节点数据 */
export interface DecodedNode {
  id: bigint
  lon: number
  lat: number
}

// ============================================================================
// IPC 命令封装
// ============================================================================

/** 获取存储统计 */
export async function getStats(): Promise<StoreStats> {
  return invoke<StoreStats>('get_stats')
}

/** 获取数据边界 */
export async function getBounds(): Promise<DataBounds | null> {
  return invoke<DataBounds | null>('get_bounds')
}

/** 加载 PBF 文件 */
export async function loadPbf(path: string): Promise<ParseProgress> {
  return invoke<ParseProgress>('load_pbf', { path })
}

/** 查询视口节点 (返回二进制数据) */
export async function queryViewportNodes(viewport: Viewport): Promise<Uint8Array> {
  const result = await invoke<number[]>('query_viewport_nodes', { viewport })
  return new Uint8Array(result)
}

/** 查询视口坐标 (纯坐标，用于渲染) */
export async function queryViewportCoords(viewport: Viewport): Promise<Float64Array> {
  const result = await invoke<number[]>('query_viewport_coords', { viewport })
  return new Float64Array(new Uint8Array(result).buffer)
}

/** 查询视口完整数据 */
export async function queryViewportFull(viewport: Viewport): Promise<Uint8Array> {
  const result = await invoke<number[]>('query_viewport_full', { viewport })
  return new Uint8Array(result)
}

// ============================================================================
// 二进制解码器 (V2: 紧凑型格式)
// ============================================================================

/**
 * V4 响应格式:
 * - Header (16 bytes): node_count, way_count, polygon_count, truncated
 * - Nodes: node_count * 32 bytes (node_id: i64, x: f64, y: f64, ref_count: u16, padding: 6 bytes)
 * - Way geometry: [total_ways: u32][way_id: i64][render_feature: u16][point_count: u32][coords...]...
 * - Polygon geometry: [total_polygons: u32][render_feature: u16][ring_count: u16][point_count: u32][coords...]...
 */

const HEADER_SIZE = 16
const NODE_SIZE = 32 // node_id(8) + x(8) + y(8) + ref_count(2) + pad(2) + pad2(4)

/** 解码响应头 */
export function decodeHeader(buffer: ArrayBuffer): ResponseHeader {
  const view = new DataView(buffer)
  return {
    nodeCount: view.getUint32(0, true),
    wayCount: view.getUint32(4, true),
    polygonCount: view.getUint32(8, true),
    truncated: view.getUint32(12, true) === 1,
  }
}

/** 节点数据 (带 ID 和优先级, 墨卡托坐标) */
export interface NodeData {
  nodeId: number // 节点 ID（用于拾取后高亮）
  x: number      // 墨卡托 X 坐标（米）
  y: number      // 墨卡托 Y 坐标（米）
  refCount: number
}

/** V4 视口响应解码结果 */
export interface ViewportData {
  header: ResponseHeader
  nodes: NodeData[]
  wayGeometry: ArrayBuffer
  polygonGeometry: ArrayBuffer
}

/**
 * 解码完整视口响应 (V4: 带节点优先级 + Polygon)
 *
 * 返回:
 * - header: 元数据
 * - nodes: 节点数组 (已按 ref_count 降序排列)
 * - wayGeometry: 紧凑型 Way 几何数据
 * - polygonGeometry: Polygon 几何数据
 */
export function decodeViewportResponseV2(buffer: ArrayBuffer): ViewportData {
  const header = decodeHeader(buffer)
  const view = new DataView(buffer)

  const nodes: NodeData[] = []
  let offset = HEADER_SIZE

  // Node 格式: [node_id: i64][x: f64][y: f64][ref_count: u16][padding: 6 bytes]
  for (let i = 0; i < header.nodeCount; i++) {
    const nodeIdLow = view.getUint32(offset, true)
    const nodeIdHigh = view.getInt32(offset + 4, true)
    const nodeId = nodeIdLow + nodeIdHigh * 0x100000000

    nodes.push({
      nodeId,
      x: view.getFloat64(offset + 8, true), // 墨卡托 X
      y: view.getFloat64(offset + 16, true), // 墨卡托 Y
      refCount: view.getUint16(offset + 24, true),
    })
    offset += NODE_SIZE
  }

  // 解析 wayGeometry 长度
  const wayDataStart = offset
  const wayCount = view.getUint32(offset, true)
  offset += 4

  // 跳过所有 Way 数据找到 Polygon 数据起始位置
  // Way 格式: [way_id: i64][render_feature: u16][point_count: u32][x,y coords...]
  for (let w = 0; w < wayCount; w++) {
    offset += 8 // way_id (i64)
    offset += 2 // render_feature (u16)
    const pointCount = view.getUint32(offset, true)
    offset += 4
    offset += pointCount * 16 // 每点 16 字节 (x: f64 + y: f64)
  }

  const wayDataEnd = offset
  const wayGeometry = buffer.slice(wayDataStart, wayDataEnd)
  const polygonGeometry = buffer.slice(wayDataEnd)

  return {
    header,
    nodes,
    wayGeometry,
    polygonGeometry,
  }
}

/** 投影转换：WGS84 经纬度 -> Web Mercator 像素坐标 */
export function lonLatToMercator(
  lon: number,
  lat: number,
  zoom: number
): { x: number; y: number } {
  const scale = 256 * Math.pow(2, zoom)
  const x = ((lon + 180) / 360) * scale
  const latRad = (lat * Math.PI) / 180
  const y = ((1 - Math.log(Math.tan(latRad) + 1 / Math.cos(latRad)) / Math.PI) / 2) * scale
  return { x, y }
}

/** 批量投影转换 (适合渲染层) */
export function projectCoordinates(
  coords: Float64Array,
  zoom: number,
  centerX: number,
  centerY: number,
): Float32Array {
  const count = coords.length / 2
  const projected = new Float32Array(count * 2)
  const scale = 256 * Math.pow(2, zoom)

  for (let i = 0; i < count; i++) {
    const lon = coords[i * 2]
    const lat = coords[i * 2 + 1]
    const latRad = (lat * Math.PI) / 180

    projected[i * 2] = ((lon + 180) / 360) * scale - centerX
    projected[i * 2 + 1] =
      ((1 - Math.log(Math.tan(latRad) + 1 / Math.cos(latRad)) / Math.PI) / 2) * scale - centerY
  }

  return projected
}

// ============================================================================
// 空间拾取 (Feature Picking)
// ============================================================================

/** 拾取结果类型 */
export interface PickedFeature {
  type: 'Node' | 'Way' | 'None'
  id?: number
}

/**
 * 在指定墨卡托坐标位置拾取最近的要素
 *
 * @param mercX 墨卡托 X 坐标（米）
 * @param mercY 墨卡托 Y 坐标（米）
 * @param toleranceMeters 拾取容差（米）
 * @param zoom 当前缩放级别，用于过滤不可见的节点
 */
export async function pickFeature(
  mercX: number,
  mercY: number,
  toleranceMeters: number,
  zoom: number,
): Promise<PickedFeature> {
  return await invoke<PickedFeature>('pick_feature', {
    mercX,
    mercY,
    toleranceMeters,
    zoom,
  })
}

// ============================================================================
// 要素详情 (Feature Details)
// ============================================================================

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

/** 获取节点详情 */
export async function getNodeDetails(nodeId: number): Promise<FeatureDetails> {
  return await invoke<FeatureDetails>('get_node_details', { nodeId })
}

/** 获取路径详情 */
export async function getWayDetails(wayId: number): Promise<FeatureDetails> {
  return await invoke<FeatureDetails>('get_way_details', { wayId })
}

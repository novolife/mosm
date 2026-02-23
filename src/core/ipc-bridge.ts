/**
 * IPC 通信桥接层
 *
 * 封装与 Tauri Rust 后端的通信
 */

import { invoke } from '@tauri-apps/api/core'

// 重导出类型
export type {
  DataBounds,
  FeatureDetails,
  NodeData,
  NodeDetails,
  NotFound,
  ParentRelation,
  ParseProgress,
  PickedFeature,
  ResponseHeader,
  StoreStats,
  UndoRedoResult,
  UpdateTagsResult,
  Viewport,
  ViewportData,
  WayDetails,
} from './types'

// 重导出二进制解码器
export { decodeHeader, decodeViewportResponse } from './binary-decoder'

// 重导出投影转换
export { lonLatToMercator, projectCoordinates } from './projection'

// 为向后兼容保留旧名称
export { decodeViewportResponse as decodeViewportResponseV2 } from './binary-decoder'

import type {
  DataBounds,
  FeatureDetails,
  ParseProgress,
  PickedFeature,
  StoreStats,
  UndoRedoResult,
  UpdateTagsResult,
  Viewport,
} from './types'

// ============================================================================
// 数据命令
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

// ============================================================================
// 查询命令
// ============================================================================

/** 查询视口节点 (返回二进制数据) - 已弃用 */
export async function queryViewportNodes(viewport: Viewport): Promise<Uint8Array> {
  const result = await invoke<number[]>('query_viewport_nodes', { viewport })
  return new Uint8Array(result)
}

/** 查询视口坐标 (纯坐标，用于渲染) - 已弃用 */
export async function queryViewportCoords(viewport: Viewport): Promise<Float64Array> {
  const result = await invoke<number[]>('query_viewport_coords', { viewport })
  return new Float64Array(new Uint8Array(result).buffer)
}

/** 查询视口完整数据 */
export async function queryViewportFull(viewport: Viewport): Promise<Uint8Array> {
  const result = await invoke<number[]>('query_viewport_full', { viewport })
  return new Uint8Array(result)
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
// 详情命令
// ============================================================================

/** 获取节点详情 */
export async function getNodeDetails(nodeId: number): Promise<FeatureDetails> {
  return await invoke<FeatureDetails>('get_node_details', { nodeId })
}

/** 获取路径详情 */
export async function getWayDetails(wayId: number): Promise<FeatureDetails> {
  return await invoke<FeatureDetails>('get_way_details', { wayId })
}

// ============================================================================
// 编辑命令
// ============================================================================

/**
 * 更新路径标签
 *
 * @param wayId 路径 ID
 * @param newTags 新的标签数组
 * @returns 更新结果，包含新的 render_feature 用于判断是否需要重绘
 */
export async function updateWayTags(
  wayId: number,
  newTags: [string, string][],
): Promise<UpdateTagsResult> {
  return await invoke<UpdateTagsResult>('update_way_tags', { wayId, newTags })
}

/**
 * 更新节点标签
 *
 * @param nodeId 节点 ID
 * @param newTags 新的标签数组
 * @returns 更新结果
 */
export async function updateNodeTags(
  nodeId: number,
  newTags: [string, string][],
): Promise<UpdateTagsResult> {
  return await invoke<UpdateTagsResult>('update_node_tags', { nodeId, newTags })
}

// ============================================================================
// 历史命令
// ============================================================================

/**
 * 撤销上一个操作
 * @returns 操作结果，包含是否需要重绘和历史栈状态
 */
export async function undo(): Promise<UndoRedoResult> {
  return await invoke<UndoRedoResult>('undo')
}

/**
 * 重做上一个撤销的操作
 * @returns 操作结果，包含是否需要重绘和历史栈状态
 */
export async function redo(): Promise<UndoRedoResult> {
  return await invoke<UndoRedoResult>('redo')
}

/**
 * 获取历史记录状态
 * @returns [undo_count, redo_count]
 */
export async function getHistoryState(): Promise<[number, number]> {
  return await invoke<[number, number]>('get_history_state')
}

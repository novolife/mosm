/**
 * 二进制协议解码器
 *
 * 解析 Rust 后端返回的紧凑型二进制数据格式
 */

import type { NodeData, ResponseHeader, ViewportData } from './types'

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

/**
 * 解码完整视口响应 (V4: 带节点优先级 + Polygon)
 *
 * V4 响应格式:
 * - Header (16 bytes): node_count, way_count, polygon_count, truncated
 * - Nodes: node_count * 32 bytes (node_id: i64, x: f64, y: f64, ref_count: u16, padding: 6 bytes)
 * - Way geometry: [total_ways: u32][way_id: i64][render_feature: u16][point_count: u32][coords...]...
 * - Polygon geometry: [total_polygons: u32][render_feature: u16][ring_count: u16][point_count: u32][coords...]...
 */
export function decodeViewportResponse(buffer: ArrayBuffer): ViewportData {
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
      x: view.getFloat64(offset + 8, true),
      y: view.getFloat64(offset + 16, true),
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

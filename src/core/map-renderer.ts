/**
 * 地图渲染引擎
 *
 * 基于 Canvas 2D 的高性能渲染器。
 * 使用 Web Mercator 投影 (EPSG:3857)
 */

import type { Viewport, NodeData } from './ipc-bridge'

/** 渲染器配置 */
export interface RendererOptions {
  canvas: HTMLCanvasElement
  devicePixelRatio?: number
}

/** 相机状态 */
export interface CameraState {
  centerLon: number
  centerLat: number
  zoom: number
}

/** 渲染统计 */
export interface RenderStats {
  fps: number
  nodeCount: number
  wayCount: number
  renderTime: number
}

/** 选中的要素 */
export interface SelectedFeature {
  type: 'node' | 'way'
  id: number
}

/** 样式配置 */
export interface RenderStyle {
  nodeColor: string
  nodeRadius: number
  wayColor: string
  wayWidth: number
  backgroundColor: string
}

const DEFAULT_STYLE: RenderStyle = {
  nodeColor: '#4fc3f7',
  nodeRadius: 3,
  wayColor: '#81c784',
  wayWidth: 1.5,
  backgroundColor: '#1e1e1e',
}

import { resolveStyle, type ResolvedStyle } from './render-styles'

const DEG_TO_RAD = Math.PI / 180

/** 地球赤道半周长（米），与 Rust 端一致 */
const EARTH_HALF_CIRCUMFERENCE = 20037508.342789244

/**
 * 经纬度 -> Web 墨卡托坐标（米）
 * 与 Rust 端 projection.rs 中的 lonlat_to_mercator 一致
 */
function lonLatToMercator(lon: number, lat: number): { x: number; y: number } {
  const x = lon * EARTH_HALF_CIRCUMFERENCE / 180
  const latClamped = Math.max(-85.051129, Math.min(85.051129, lat))
  const latRad = (90 + latClamped) * DEG_TO_RAD / 2
  const y = Math.log(Math.tan(latRad)) * EARTH_HALF_CIRCUMFERENCE / Math.PI
  return { x, y }
}

/**
 * Web 墨卡托坐标（米） -> 经纬度
 */
function mercatorToLonLat(x: number, y: number): { lon: number; lat: number } {
  const lon = x * 180 / EARTH_HALF_CIRCUMFERENCE
  const lat = (2 * Math.atan(Math.exp(y * Math.PI / EARTH_HALF_CIRCUMFERENCE)) - Math.PI / 2) * 180 / Math.PI
  return { lon, lat }
}

/**
 * 计算给定 zoom 级别下，每像素对应多少米
 * 在 zoom=0 时，整个世界 (40075016.686 米) 映射到 256 像素
 */
function getMetersPerPixel(zoom: number): number {
  return (2 * EARTH_HALF_CIRCUMFERENCE) / (256 * Math.pow(2, zoom))
}

export class MapRenderer {
  private canvas: HTMLCanvasElement
  private ctx: CanvasRenderingContext2D
  private dpr: number
  private width = 0
  private height = 0

  private camera: CameraState = {
    centerLon: 0,
    centerLat: 0,
    zoom: 2,
  }

  // 缓存的相机中心墨卡托坐标（米）
  private centerMercatorX = 0
  private centerMercatorY = 0

  private nodes: NodeData[] = []
  private wayBuffer: ArrayBuffer | null = null
  private polygonBuffer: ArrayBuffer | null = null

  private style: RenderStyle = { ...DEFAULT_STYLE }
  private animationId: number | null = null
  private needsRender = true

  private stats: RenderStats = {
    fps: 0,
    nodeCount: 0,
    wayCount: 0,
    renderTime: 0,
  }

  private lastFrameTime = 0
  private frameCount = 0
  private fpsUpdateTime = 0

  // 相机变化回调
  private onCameraChange: (() => void) | null = null

  // 选中状态
  private selectedFeature: SelectedFeature | null = null
  private onFeatureClick:
    | ((mercX: number, mercY: number, toleranceMeters: number, zoom: number) => void)
    | null = null

  constructor(options: RendererOptions) {
    this.canvas = options.canvas
    this.dpr = options.devicePixelRatio ?? window.devicePixelRatio ?? 1

    const ctx = this.canvas.getContext('2d', { alpha: false })
    if (!ctx) {
      throw new Error('无法获取 Canvas 2D 上下文')
    }
    this.ctx = ctx

    this.resize()
    this.setupEventListeners()
    this.updateCenterMercator()
  }

  /** 启动渲染循环 */
  start(): void {
    if (this.animationId !== null) return
    this.lastFrameTime = performance.now()
    this.fpsUpdateTime = this.lastFrameTime
    this.loop()
  }

  /** 停止渲染循环 */
  stop(): void {
    if (this.animationId !== null) {
      cancelAnimationFrame(this.animationId)
      this.animationId = null
    }
  }

  /** 调整画布大小 */
  resize(): void {
    const rect = this.canvas.getBoundingClientRect()
    this.width = rect.width
    this.height = rect.height

    this.canvas.width = this.width * this.dpr
    this.canvas.height = this.height * this.dpr

    // 重置变换矩阵，避免 DPR 累积
    this.ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0)
    this.requestRender()
  }

  /** 设置相机位置 */
  setCamera(camera: Partial<CameraState>): void {
    Object.assign(this.camera, camera)
    this.updateCenterMercator()
    this.requestRender()
  }

  /** 获取当前视口 (WGS84 坐标) */
  getViewport(): Viewport {
    const metersPerPixel = getMetersPerPixel(this.camera.zoom)
    const halfWidthMeters = (this.width / 2) * metersPerPixel
    const halfHeightMeters = (this.height / 2) * metersPerPixel

    // 左上角和右下角的墨卡托坐标
    const topLeftMerc = {
      x: this.centerMercatorX - halfWidthMeters,
      y: this.centerMercatorY + halfHeightMeters, // Y 轴向上为正
    }
    const bottomRightMerc = {
      x: this.centerMercatorX + halfWidthMeters,
      y: this.centerMercatorY - halfHeightMeters,
    }

    // 转换回经纬度
    const topLeft = mercatorToLonLat(topLeftMerc.x, topLeftMerc.y)
    const bottomRight = mercatorToLonLat(bottomRightMerc.x, bottomRightMerc.y)

    return {
      min_lon: topLeft.lon,
      max_lat: topLeft.lat,
      max_lon: bottomRight.lon,
      min_lat: bottomRight.lat,
      zoom: this.camera.zoom,
    }
  }

  /** 设置节点数据 (带优先级) */
  setNodeData(nodes: NodeData[]): void {
    this.nodes = nodes
    this.stats.nodeCount = nodes.length
    this.requestRender()
  }

  /** 设置路径数据 */
  setWayData(data: ArrayBuffer): void {
    this.wayBuffer = data
    this.requestRender()
  }

  /** 设置多边形数据 */
  setPolygonData(data: ArrayBuffer): void {
    this.polygonBuffer = data
    this.requestRender()
  }

  /** 清除所有数据 */
  clearData(): void {
    this.nodes = []
    this.wayBuffer = null
    this.polygonBuffer = null
    this.stats.nodeCount = 0
    this.stats.wayCount = 0
    this.requestRender()
  }

  /** 获取渲染统计 */
  getStats(): RenderStats {
    return { ...this.stats }
  }

  /** 获取当前相机状态 */
  getCamera(): CameraState {
    return { ...this.camera }
  }

  /** 设置相机变化回调 */
  setOnCameraChange(callback: (() => void) | null): void {
    this.onCameraChange = callback
  }

  /** 触发相机变化回调 */
  private notifyCameraChange(): void {
    if (this.onCameraChange) {
      this.onCameraChange()
    }
  }

  /** 设置要素点击回调 */
  setOnFeatureClick(
    callback: ((mercX: number, mercY: number, toleranceMeters: number, zoom: number) => void) | null,
  ): void {
    this.onFeatureClick = callback
  }

  /** 获取当前选中的要素 */
  getSelectedFeature(): SelectedFeature | null {
    return this.selectedFeature
  }

  /** 设置选中的要素 */
  setSelectedFeature(feature: SelectedFeature | null): void {
    this.selectedFeature = feature
    this.requestRender()
  }

  /** 清除选中状态 */
  clearSelection(): void {
    this.selectedFeature = null
    this.requestRender()
  }

  /** 设置样式 */
  setStyle(style: Partial<RenderStyle>): void {
    Object.assign(this.style, style)
    this.requestRender()
  }

  /** 平移 (屏幕像素) */
  pan(dx: number, dy: number): void {
    const metersPerPixel = getMetersPerPixel(this.camera.zoom)
    
    // 屏幕像素移动转换为墨卡托坐标移动
    this.centerMercatorX -= dx * metersPerPixel
    this.centerMercatorY += dy * metersPerPixel // Y 轴向上为正，屏幕向下为正

    // 更新经纬度中心
    const newCenter = mercatorToLonLat(this.centerMercatorX, this.centerMercatorY)
    this.camera.centerLon = newCenter.lon
    this.camera.centerLat = newCenter.lat

    this.requestRender()
    this.notifyCameraChange()
  }

  /** 缩放 */
  zoomAt(delta: number, screenX: number, screenY: number): void {
    const oldZoom = this.camera.zoom
    const newZoom = Math.max(1, Math.min(26, oldZoom + delta))

    if (newZoom === oldZoom) return

    const oldMetersPerPixel = getMetersPerPixel(oldZoom)
    const newMetersPerPixel = getMetersPerPixel(newZoom)

    // 计算缩放中心的墨卡托坐标
    const offsetX = (screenX - this.width / 2) * oldMetersPerPixel
    const offsetY = (this.height / 2 - screenY) * oldMetersPerPixel // 屏幕 Y 向下，墨卡托 Y 向上
    const zoomPointMercX = this.centerMercatorX + offsetX
    const zoomPointMercY = this.centerMercatorY + offsetY

    // 更新缩放级别
    this.camera.zoom = newZoom

    // 保持缩放中心在屏幕同一位置：计算新的中心墨卡托坐标
    const newOffsetX = (screenX - this.width / 2) * newMetersPerPixel
    const newOffsetY = (this.height / 2 - screenY) * newMetersPerPixel
    this.centerMercatorX = zoomPointMercX - newOffsetX
    this.centerMercatorY = zoomPointMercY - newOffsetY

    // 更新相机中心经纬度
    const newCenter = mercatorToLonLat(this.centerMercatorX, this.centerMercatorY)
    this.camera.centerLon = newCenter.lon
    this.camera.centerLat = newCenter.lat

    this.requestRender()
    this.notifyCameraChange()
  }

  /** 销毁 */
  destroy(): void {
    this.stop()
    this.clearData()
  }

  // ===========================================================================
  // 私有方法
  // ===========================================================================

  private updateCenterMercator(): void {
    const center = lonLatToMercator(this.camera.centerLon, this.camera.centerLat)
    this.centerMercatorX = center.x
    this.centerMercatorY = center.y
  }

  private loop = (): void => {
    const now = performance.now()

    this.frameCount++
    if (now - this.fpsUpdateTime >= 1000) {
      this.stats.fps = Math.round((this.frameCount * 1000) / (now - this.fpsUpdateTime))
      this.frameCount = 0
      this.fpsUpdateTime = now
    }

    if (this.needsRender) {
      const startTime = performance.now()
      this.render()
      this.stats.renderTime = performance.now() - startTime
      this.needsRender = false
    }

    this.lastFrameTime = now
    this.animationId = requestAnimationFrame(this.loop)
  }

  private requestRender(): void {
    this.needsRender = true
  }

  private render(): void {
    const { ctx, width, height } = this

    // 重置变换矩阵，确保每帧渲染前状态正确
    ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0)

    ctx.fillStyle = this.style.backgroundColor
    ctx.fillRect(0, 0, width, height)

    ctx.save()
    ctx.translate(width / 2, height / 2)

    // 渲染顺序：先底层（多边形），再线条，最后节点
    this.renderPolygons()
    this.renderWays()
    this.renderNodes()

    ctx.restore()
  }

  /** 墨卡托坐标 -> 屏幕坐标 (相对于画布中心) */
  private mercatorToScreen(mercX: number, mercY: number): { x: number; y: number } {
    const metersPerPixel = getMetersPerPixel(this.camera.zoom)
    return {
      x: (mercX - this.centerMercatorX) / metersPerPixel,
      y: (this.centerMercatorY - mercY) / metersPerPixel, // Y 轴翻转
    }
  }

  /**
   * 屏幕坐标 -> 墨卡托坐标 (逆变换)
   *
   * 用于点击拾取：将鼠标点击位置转换为地理坐标
   *
   * @param screenX 屏幕 X 坐标（相对于 Canvas 左上角）
   * @param screenY 屏幕 Y 坐标（相对于 Canvas 左上角）
   */
  screenToMercator(screenX: number, screenY: number): { mercX: number; mercY: number } {
    const metersPerPixel = getMetersPerPixel(this.camera.zoom)

    // 转换为相对于画布中心的坐标
    const relX = screenX - this.width / 2
    const relY = screenY - this.height / 2

    // 逆变换：屏幕坐标 -> 墨卡托坐标
    const mercX = this.centerMercatorX + relX * metersPerPixel
    const mercY = this.centerMercatorY - relY * metersPerPixel // Y 轴翻转

    return { mercX, mercY }
  }

  /**
   * 获取当前缩放级别下，指定像素数对应的米数
   * 用于计算拾取容差
   */
  getToleranceInMeters(pixelTolerance: number): number {
    return pixelTolerance * getMetersPerPixel(this.camera.zoom)
  }

  /**
   * 渲染多边形 (Area + Multipolygon)
   *
   * 使用 clip + 双倍线宽魔法实现完美的内向描边效果：
   * - 外环向内描边
   * - 内环（洞）向外描边
   *
   * 二进制格式:
   * [polygon_count: u32]
   * [way_id: i64][render_feature: u16][ring_count: u16]
   * [point_count_ring1: u32][x,y coords...]
   * [point_count_ring2: u32][x,y coords...]...
   */
  private renderPolygons(): void {
    if (!this.polygonBuffer || this.polygonBuffer.byteLength < 4) return

    const { ctx } = this
    const view = new DataView(this.polygonBuffer)
    const zoomFactor = Math.min(1, this.camera.zoom / 14)

    // 收集所有 Polygon 数据用于后续高亮
    interface PolygonData {
      wayId: number
      renderFeature: number
      rings: Array<Array<{ x: number; y: number }>>
    }
    const polygons: PolygonData[] = []

    let offset = 0
    const polygonCount = view.getUint32(offset, true)
    offset += 4

    for (let p = 0; p < polygonCount; p++) {
      // 读取 Way ID (8 字节)
      const wayIdLow = view.getUint32(offset, true)
      const wayIdHigh = view.getInt32(offset + 4, true)
      const wayId = wayIdLow + wayIdHigh * 0x100000000
      offset += 8

      // 读取 RenderFeature (2 字节)
      const renderFeature = view.getUint16(offset, true)
      offset += 2

      // 读取 ring_count (2 字节)
      const ringCount = view.getUint16(offset, true)
      offset += 2

      if (ringCount === 0) continue

      // 获取样式
      const style = resolveStyle(renderFeature)

      // 收集所有环的屏幕坐标
      const rings: Array<Array<{ x: number; y: number }>> = []

      for (let r = 0; r < ringCount; r++) {
        const pointCount = view.getUint32(offset, true)
        offset += 4

        const ring: Array<{ x: number; y: number }> = []
        for (let i = 0; i < pointCount; i++) {
          const mercX = view.getFloat64(offset, true)
          offset += 8
          const mercY = view.getFloat64(offset, true)
          offset += 8
          ring.push(this.mercatorToScreen(mercX, mercY))
        }
        rings.push(ring)
      }

      // 保存 Polygon 数据
      polygons.push({ wayId, renderFeature, rings })

      // === Clip 魔法渲染 ===
      ctx.save()
      ctx.beginPath()

      // 1. 将所有环加入同一个 Path
      for (const ring of rings) {
        for (let i = 0; i < ring.length; i++) {
          if (i === 0) {
            ctx.moveTo(ring[i].x, ring[i].y)
          } else {
            ctx.lineTo(ring[i].x, ring[i].y)
          }
        }
        ctx.closePath()
      }

      // 2. 使用 evenodd 规则裁剪
      ctx.clip('evenodd')

      // 3. 先填充一层极淡的底色
      ctx.fillStyle = this.getPolygonFillColor(renderFeature)
      ctx.fill('evenodd')

      // 4. 双倍线宽描边（一半会被裁掉）
      const strokeWidth = Math.max(1, style.width * zoomFactor)
      ctx.lineWidth = strokeWidth * 2
      ctx.strokeStyle = style.color
      ctx.lineCap = 'round'
      ctx.lineJoin = 'round'

      // 重新绘制路径用于描边
      ctx.beginPath()
      for (const ring of rings) {
        for (let i = 0; i < ring.length; i++) {
          if (i === 0) {
            ctx.moveTo(ring[i].x, ring[i].y)
          } else {
            ctx.lineTo(ring[i].x, ring[i].y)
          }
        }
        ctx.closePath()
      }
      ctx.stroke()

      ctx.restore() // 清除裁剪区域
    }

    // 高亮选中的 Polygon
    if (this.selectedFeature?.type === 'way') {
      const selectedPolygon = polygons.find((p) => p.wayId === this.selectedFeature!.id)
      if (selectedPolygon) {
        ctx.save()

        // 绘制高亮边框（不使用 clip，直接描边）
        ctx.beginPath()
        for (const ring of selectedPolygon.rings) {
          for (let i = 0; i < ring.length; i++) {
            if (i === 0) {
              ctx.moveTo(ring[i].x, ring[i].y)
            } else {
              ctx.lineTo(ring[i].x, ring[i].y)
            }
          }
          ctx.closePath()
        }

        // 外层光晕
        ctx.strokeStyle = 'rgba(0, 255, 255, 0.5)'
        ctx.lineWidth = Math.max(6, 4 * zoomFactor)
        ctx.lineCap = 'round'
        ctx.lineJoin = 'round'
        ctx.stroke()

        // 内层实线
        ctx.strokeStyle = '#00ffff'
        ctx.lineWidth = Math.max(2, 2 * zoomFactor)
        ctx.stroke()

        ctx.restore()
      }
    }
  }

  /** 根据 RenderFeature 获取多边形填充颜色 */
  private getPolygonFillColor(renderFeature: number): string {
    const baseType = renderFeature & 0xff

    // 建筑
    if (baseType === 40) {
      return 'rgba(212, 163, 115, 0.15)'
    }
    // 水域
    if (baseType >= 30 && baseType < 40) {
      return 'rgba(66, 165, 245, 0.2)'
    }
    // 森林/自然
    if (baseType >= 50 && baseType < 60) {
      return 'rgba(102, 187, 106, 0.15)'
    }
    // 土地利用
    if (baseType >= 60 && baseType < 70) {
      return 'rgba(197, 225, 165, 0.1)'
    }
    // 默认
    return 'rgba(128, 128, 128, 0.1)'
  }

  /**
   * 渲染节点 (LOD 策略)
   *
   * 优先节点 (ref_count >= 2):
   * - zoom < 18: 不显示
   * - zoom 18-19: 红点
   * - zoom >= 20: 红色方框
   *
   * 普通节点:
   * - zoom < 20: 不显示
   * - zoom 20-21: 红点
   * - zoom >= 22: 红色圆框
   */
  private renderNodes(): void {
    if (this.nodes.length === 0) {
      return
    }

    const { ctx } = this
    const zoom = this.camera.zoom

    const highPriorityColor = '#f44336' // 红色
    const normalColor = '#ef9a9a' // 浅红色
    const selectedColor = '#00ffff' // 亮青色（选中高亮）
    const size = Math.max(2, 3 * Math.min(1.5, zoom / 20))

    // 选中节点的屏幕坐标（如果有的话，最后绘制）
    let selectedNodeScreen: { x: number; y: number } | null = null

    for (const node of this.nodes) {
      const { x, y } = this.mercatorToScreen(node.x, node.y)
      const isHighPriority = node.refCount >= 2

      // 检查是否为选中节点
      const isSelected =
        this.selectedFeature?.type === 'node' && this.selectedFeature.id === node.nodeId

      if (isSelected) {
        selectedNodeScreen = { x, y }
        continue // 选中节点最后绘制
      }

      if (isHighPriority) {
        // 优先节点
        if (zoom >= 20) {
          // zoom >= 20: 红色方框
          const halfSize = size * 0.8
          ctx.strokeStyle = highPriorityColor
          ctx.lineWidth = 1.5
          ctx.strokeRect(x - halfSize, y - halfSize, halfSize * 2, halfSize * 2)
        } else {
          // zoom 18-19: 红点
          ctx.beginPath()
          ctx.arc(x, y, size * 0.5, 0, Math.PI * 2)
          ctx.fillStyle = highPriorityColor
          ctx.fill()
        }
      } else {
        // 普通节点 (只有 zoom >= 20 时 Rust 才会返回)
        if (zoom >= 22) {
          // zoom >= 22: 红色圆框
          ctx.beginPath()
          ctx.arc(x, y, size * 0.6, 0, Math.PI * 2)
          ctx.strokeStyle = normalColor
          ctx.lineWidth = 1
          ctx.stroke()
        } else {
          // zoom 20-21: 红点
          ctx.beginPath()
          ctx.arc(x, y, size * 0.4, 0, Math.PI * 2)
          ctx.fillStyle = normalColor
          ctx.fill()
        }
      }
    }

    // 绘制选中的节点（高亮效果）
    if (selectedNodeScreen) {
      const { x, y } = selectedNodeScreen
      const highlightSize = size * 1.5

      // 绘制外圈光晕
      ctx.beginPath()
      ctx.arc(x, y, highlightSize + 4, 0, Math.PI * 2)
      ctx.fillStyle = 'rgba(0, 255, 255, 0.3)'
      ctx.fill()

      // 绘制青色圆点
      ctx.beginPath()
      ctx.arc(x, y, highlightSize, 0, Math.PI * 2)
      ctx.fillStyle = selectedColor
      ctx.fill()

      // 绘制白色边框
      ctx.strokeStyle = '#ffffff'
      ctx.lineWidth = 2
      ctx.stroke()
    }
  }

  /**
   * 渲染 Way 几何数据 (Z-Order + 样式修饰符)
   *
   * 二进制格式: [wayCount: u32][renderFeature: u16][pointCount: u32][x,y coords...]...
   *
   * 渲染顺序：数据已按 z_order 升序排列（隧道 -> 水系 -> 普通道路 -> 桥梁）
   * 特殊效果：
   * - 桥梁 (BRIDGE flag): 先画深色边框，再画主色
   * - 隧道 (TUNNEL flag): 虚线 + 颜色变暗
   * - 间歇性 (INTERMITTENT flag): 细虚线
   */
  private renderWays(): void {
    if (!this.wayBuffer || this.wayBuffer.byteLength < 4) return

    const { ctx } = this
    const view = new DataView(this.wayBuffer)
    const zoomFactor = Math.min(1, this.camera.zoom / 14)

    let offset = 0
    const wayCount = view.getUint32(offset, true)
    offset += 4

    this.stats.wayCount = wayCount

    // 第一遍：收集所有路径数据（避免重复读取 buffer）
    interface WayPath {
      wayId: number
      feature: number
      style: ResolvedStyle
      points: Array<{ x: number; y: number }>
    }

    const ways: WayPath[] = []

    // Way 格式: [way_id: i64][render_feature: u16][point_count: u32][x,y coords...]
    for (let w = 0; w < wayCount; w++) {
      // 读取 Way ID (8 字节)
      const wayIdLow = view.getUint32(offset, true)
      const wayIdHigh = view.getInt32(offset + 4, true)
      const wayId = wayIdLow + wayIdHigh * 0x100000000
      offset += 8

      // 读取 RenderFeature (2 字节)
      const feature = view.getUint16(offset, true)
      offset += 2

      // 读取点数量 (4 字节)
      const pointCount = view.getUint32(offset, true)
      offset += 4

      if (pointCount < 2) {
        offset += pointCount * 16
        continue
      }

      const style = resolveStyle(feature)
      const points: Array<{ x: number; y: number }> = []

      for (let p = 0; p < pointCount; p++) {
        const mercX = view.getFloat64(offset, true)
        offset += 8
        const mercY = view.getFloat64(offset, true)
        offset += 8
        points.push(this.mercatorToScreen(mercX, mercY))
      }

      ways.push({ feature, style, points, wayId })
    }

    // 第二遍：先画所有桥梁的边框（casing）
    for (const way of ways) {
      if (!way.style.drawCasing) continue

      ctx.beginPath()
      ctx.strokeStyle = way.style.casingColor || '#37474f'
      ctx.lineWidth = Math.max(1, (way.style.width + (way.style.casingWidth || 2) * 2) * zoomFactor)
      ctx.lineCap = way.style.lineCap || 'round'
      ctx.lineJoin = way.style.lineJoin || 'round'
      ctx.setLineDash([])

      for (let i = 0; i < way.points.length; i++) {
        const { x, y } = way.points[i]
        if (i === 0) ctx.moveTo(x, y)
        else ctx.lineTo(x, y)
      }
      ctx.stroke()
    }

    // 第三遍：画主线条（批处理优化）
    let currentFeature = -1
    let currentStyle: ResolvedStyle | null = null
    let batchStarted = false

    for (const way of ways) {
      // 检查是否需要切换样式
      if (way.feature !== currentFeature) {
        // 结束上一批
        if (batchStarted) {
          ctx.stroke()
        }

        // 设置新样式
        currentStyle = way.style
        ctx.strokeStyle = currentStyle.color
        ctx.lineWidth = Math.max(1, currentStyle.width * zoomFactor)
        ctx.lineCap = currentStyle.lineCap || 'round'
        ctx.lineJoin = currentStyle.lineJoin || 'round'

        // 设置虚线
        if (currentStyle.lineDash) {
          ctx.setLineDash(currentStyle.lineDash.map((d) => d * zoomFactor))
        } else {
          ctx.setLineDash([])
        }

        ctx.beginPath()
        currentFeature = way.feature
        batchStarted = true
      }

      // 绘制路径
      for (let i = 0; i < way.points.length; i++) {
        const { x, y } = way.points[i]
        if (i === 0) ctx.moveTo(x, y)
        else ctx.lineTo(x, y)
      }
    }

    // 结束最后一批
    if (batchStarted) {
      ctx.stroke()
    }

    // 第四遍：高亮选中的 Way（最后绘制，确保在顶层）
    if (this.selectedFeature?.type === 'way') {
      const selectedWay = ways.find((w) => w.wayId === this.selectedFeature!.id)
      if (selectedWay) {
        // 绘制高亮边框
        ctx.beginPath()
        ctx.strokeStyle = '#00ffff' // 亮青色
        ctx.lineWidth = Math.max(3, (selectedWay.style.width + 4) * zoomFactor)
        ctx.lineCap = 'round'
        ctx.lineJoin = 'round'
        ctx.setLineDash([])
        ctx.globalAlpha = 0.7

        for (let i = 0; i < selectedWay.points.length; i++) {
          const { x, y } = selectedWay.points[i]
          if (i === 0) ctx.moveTo(x, y)
          else ctx.lineTo(x, y)
        }
        ctx.stroke()

        // 绘制内线（保持原始样式）
        ctx.beginPath()
        ctx.strokeStyle = selectedWay.style.color
        ctx.lineWidth = Math.max(1, selectedWay.style.width * zoomFactor)
        ctx.globalAlpha = 1.0

        for (let i = 0; i < selectedWay.points.length; i++) {
          const { x, y } = selectedWay.points[i]
          if (i === 0) ctx.moveTo(x, y)
          else ctx.lineTo(x, y)
        }
        ctx.stroke()
      }
    }

    // 重置状态
    ctx.globalAlpha = 1.0
    ctx.setLineDash([])
  }

  private setupEventListeners(): void {
    let isDragging = false
    let dragMoved = false
    let lastX = 0
    let lastY = 0

    this.canvas.addEventListener('mousedown', (e) => {
      if (e.button === 0) {
        // 左键
        isDragging = true
        dragMoved = false
        lastX = e.clientX
        lastY = e.clientY
        this.canvas.style.cursor = 'grabbing'
      }
    })

    window.addEventListener('mousemove', (e) => {
      if (!isDragging) return
      const dx = e.clientX - lastX
      const dy = e.clientY - lastY

      // 只有移动超过阈值才算真正拖拽
      if (Math.abs(dx) > 2 || Math.abs(dy) > 2) {
        dragMoved = true
      }

      this.pan(dx, dy)
      lastX = e.clientX
      lastY = e.clientY
    })

    window.addEventListener('mouseup', (e) => {
      if (e.button === 0) {
        isDragging = false
        this.canvas.style.cursor = 'grab'
      }
    })

    // 点击拾取（只在没有拖拽时触发）
    this.canvas.addEventListener('click', (e) => {
      if (dragMoved) return // 拖拽后不触发点击

      const rect = this.canvas.getBoundingClientRect()
      const screenX = e.clientX - rect.left
      const screenY = e.clientY - rect.top

      // 转换为墨卡托坐标
      const { mercX, mercY } = this.screenToMercator(screenX, screenY)

      // 计算拾取容差（屏幕上 8 像素对应的米数）
      const toleranceMeters = this.getToleranceInMeters(8)

      // 触发回调，传递当前缩放级别用于节点可见性过滤
      if (this.onFeatureClick) {
        this.onFeatureClick(mercX, mercY, toleranceMeters, this.camera.zoom)
      }
    })

    this.canvas.addEventListener(
      'wheel',
      (e) => {
        e.preventDefault()
        const rect = this.canvas.getBoundingClientRect()
        const x = e.clientX - rect.left
        const y = e.clientY - rect.top
        const delta = -e.deltaY * 0.002
        this.zoomAt(delta, x, y)
      },
      { passive: false },
    )

    this.canvas.style.cursor = 'grab'
  }
}

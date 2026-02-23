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

  /** 清除所有数据 */
  clearData(): void {
    this.nodes = []
    this.wayBuffer = null
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
    const newZoom = Math.max(1, Math.min(22, oldZoom + delta))

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
   * 渲染节点 (LOD 策略)
   *
   * - zoom < 17: 不显示节点 (Rust 侧已过滤)
   * - zoom 17-18: 优先节点 (ref_count >= 2) 显示为红色小方框
   * - zoom 19-20: 优先节点红色方框 + 普通节点红点
   * - zoom >= 21: 优先节点红色方框 + 普通节点红色小圆圈
   */
  private renderNodes(): void {
    if (this.nodes.length === 0) {
      return
    }

    const { ctx } = this
    const zoom = this.camera.zoom

    const highPriorityColor = '#f44336' // 红色
    const normalColor = '#ef9a9a' // 浅红色
    const size = Math.max(2, 3 * Math.min(1.5, zoom / 18))

    for (const node of this.nodes) {
      // 节点数据已经是墨卡托坐标，直接转换为屏幕坐标
      const { x, y } = this.mercatorToScreen(node.x, node.y)
      const isHighPriority = node.refCount >= 2

      if (isHighPriority) {
        // 优先节点: 红色小方框 (所有缩放级别)
        const halfSize = size * 0.8
        ctx.strokeStyle = highPriorityColor
        ctx.lineWidth = 1.5
        ctx.strokeRect(x - halfSize, y - halfSize, halfSize * 2, halfSize * 2)
      } else if (zoom >= 21) {
        // zoom >= 21: 普通节点显示为红色小圆圈
        ctx.beginPath()
        ctx.arc(x, y, size * 0.6, 0, Math.PI * 2)
        ctx.strokeStyle = normalColor
        ctx.lineWidth = 1
        ctx.stroke()
      } else if (zoom >= 19) {
        // zoom 19-20: 普通节点显示为红点
        ctx.beginPath()
        ctx.arc(x, y, size * 0.5, 0, Math.PI * 2)
        ctx.fillStyle = normalColor
        ctx.fill()
      }
      // zoom 17-18: 只有优先节点会被 Rust 端返回
    }
  }

  /**
   * 渲染 Way 几何数据 (零对象分配)
   */
  private renderWays(): void {
    if (!this.wayBuffer || this.wayBuffer.byteLength < 4) return

    const { ctx } = this
    const view = new DataView(this.wayBuffer)

    ctx.strokeStyle = this.style.wayColor
    ctx.lineWidth = Math.max(1, this.style.wayWidth * Math.min(1, this.camera.zoom / 14))
    ctx.lineCap = 'round'
    ctx.lineJoin = 'round'

    let offset = 0
    const wayCount = view.getUint32(offset, true)
    offset += 4

    this.stats.wayCount = wayCount

    for (let w = 0; w < wayCount; w++) {
      const pointCount = view.getUint32(offset, true)
      offset += 4

      if (pointCount < 2) {
        offset += pointCount * 16
        continue
      }

      ctx.beginPath()

      for (let p = 0; p < pointCount; p++) {
        // 数据已经是墨卡托坐标（米）
        const mercX = view.getFloat64(offset, true)
        offset += 8
        const mercY = view.getFloat64(offset, true)
        offset += 8

        const { x, y } = this.mercatorToScreen(mercX, mercY)

        if (p === 0) {
          ctx.moveTo(x, y)
        } else {
          ctx.lineTo(x, y)
        }
      }

      ctx.stroke()
    }
  }

  private setupEventListeners(): void {
    let isDragging = false
    let lastX = 0
    let lastY = 0

    this.canvas.addEventListener('mousedown', (e) => {
      isDragging = true
      lastX = e.clientX
      lastY = e.clientY
      this.canvas.style.cursor = 'grabbing'
    })

    window.addEventListener('mousemove', (e) => {
      if (!isDragging) return
      const dx = e.clientX - lastX
      const dy = e.clientY - lastY
      this.pan(dx, dy)
      lastX = e.clientX
      lastY = e.clientY
    })

    window.addEventListener('mouseup', () => {
      isDragging = false
      this.canvas.style.cursor = 'grab'
    })

    this.canvas.addEventListener('wheel', (e) => {
      e.preventDefault()
      const rect = this.canvas.getBoundingClientRect()
      const x = e.clientX - rect.left
      const y = e.clientY - rect.top
      const delta = -e.deltaY * 0.002
      this.zoomAt(delta, x, y)
    }, { passive: false })

    this.canvas.style.cursor = 'grab'
  }
}

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
const TILE_SIZE = 256

/** Web Mercator 投影：经纬度 -> 世界像素坐标 */
function lonLatToPixel(lon: number, lat: number, zoom: number): { x: number; y: number } {
  const scale = TILE_SIZE * Math.pow(2, zoom)
  const x = ((lon + 180) / 360) * scale
  const latRad = lat * DEG_TO_RAD
  const y = ((1 - Math.log(Math.tan(latRad) + 1 / Math.cos(latRad)) / Math.PI) / 2) * scale
  return { x, y }
}

/** Web Mercator 逆投影：世界像素坐标 -> 经纬度 */
function pixelToLonLat(x: number, y: number, zoom: number): { lon: number; lat: number } {
  const scale = TILE_SIZE * Math.pow(2, zoom)
  const lon = (x / scale) * 360 - 180
  const n = Math.PI - (2 * Math.PI * y) / scale
  const lat = (180 / Math.PI) * Math.atan(0.5 * (Math.exp(n) - Math.exp(-n)))
  return { lon, lat }
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

  // 缓存的相机中心像素坐标
  private centerPixelX = 0
  private centerPixelY = 0

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
    this.updateCenterPixel()
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

    this.ctx.scale(this.dpr, this.dpr)
    this.requestRender()
  }

  /** 设置相机位置 */
  setCamera(camera: Partial<CameraState>): void {
    Object.assign(this.camera, camera)
    this.updateCenterPixel()
    this.requestRender()
  }

  /** 获取当前视口 (WGS84 坐标) */
  getViewport(): Viewport {
    const halfWidth = this.width / 2
    const halfHeight = this.height / 2

    const topLeft = pixelToLonLat(
      this.centerPixelX - halfWidth,
      this.centerPixelY - halfHeight,
      this.camera.zoom
    )
    const bottomRight = pixelToLonLat(
      this.centerPixelX + halfWidth,
      this.centerPixelY + halfHeight,
      this.camera.zoom
    )

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
    console.log(`MapRenderer.setNodeData: 接收 ${nodes.length} 个节点`)
    if (nodes.length > 0) {
      console.log(`  首个: lon=${nodes[0].lon}, lat=${nodes[0].lat}, ref=${nodes[0].refCount}`)
    }
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

  /** 设置样式 */
  setStyle(style: Partial<RenderStyle>): void {
    Object.assign(this.style, style)
    this.requestRender()
  }

  /** 平移 (屏幕像素) */
  pan(dx: number, dy: number): void {
    this.centerPixelX -= dx
    this.centerPixelY -= dy

    // 反向计算新的经纬度中心
    const newCenter = pixelToLonLat(this.centerPixelX, this.centerPixelY, this.camera.zoom)
    this.camera.centerLon = newCenter.lon
    this.camera.centerLat = newCenter.lat

    this.requestRender()
  }

  /** 缩放 */
  zoomAt(delta: number, screenX: number, screenY: number): void {
    const oldZoom = this.camera.zoom
    const newZoom = Math.max(1, Math.min(22, oldZoom + delta))

    if (newZoom === oldZoom) return

    // 计算缩放中心的世界坐标
    const worldX = this.centerPixelX + (screenX - this.width / 2)
    const worldY = this.centerPixelY + (screenY - this.height / 2)
    const zoomPoint = pixelToLonLat(worldX, worldY, oldZoom)

    // 更新缩放级别
    this.camera.zoom = newZoom

    // 计算新的缩放中心像素位置
    const newZoomPointPixel = lonLatToPixel(zoomPoint.lon, zoomPoint.lat, newZoom)

    // 保持缩放中心在屏幕同一位置
    this.centerPixelX = newZoomPointPixel.x - (screenX - this.width / 2)
    this.centerPixelY = newZoomPointPixel.y - (screenY - this.height / 2)

    // 更新相机中心经纬度
    const newCenter = pixelToLonLat(this.centerPixelX, this.centerPixelY, newZoom)
    this.camera.centerLon = newCenter.lon
    this.camera.centerLat = newCenter.lat

    this.requestRender()
  }

  /** 销毁 */
  destroy(): void {
    this.stop()
    this.clearData()
  }

  // ===========================================================================
  // 私有方法
  // ===========================================================================

  private updateCenterPixel(): void {
    const center = lonLatToPixel(this.camera.centerLon, this.camera.centerLat, this.camera.zoom)
    this.centerPixelX = center.x
    this.centerPixelY = center.y
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

    ctx.fillStyle = this.style.backgroundColor
    ctx.fillRect(0, 0, width, height)

    ctx.save()
    ctx.translate(width / 2, height / 2)

    this.renderWays()
    this.renderNodes()

    ctx.restore()
  }

  /** 经纬度 -> 屏幕坐标 (相对于画布中心) */
  private lonLatToScreen(lon: number, lat: number): { x: number; y: number } {
    const pixel = lonLatToPixel(lon, lat, this.camera.zoom)
    return {
      x: pixel.x - this.centerPixelX,
      y: pixel.y - this.centerPixelY,
    }
  }

  /**
   * 渲染节点 (LOD 策略)
   *
   * - zoom < 14: 不显示节点 (Rust 侧已过滤)
   * - zoom 14-16: 显示红点 (只有连接多路径的节点)
   * - zoom >= 17: 显示红色圆圈 (所有节点)
   */
  private renderNodes(): void {
    if (this.nodes.length === 0) {
      // console.log('renderNodes: 无节点数据')
      return
    }
    console.log(`renderNodes: 渲染 ${this.nodes.length} 个节点, zoom=${this.camera.zoom.toFixed(1)}`)

    const { ctx } = this
    const zoom = this.camera.zoom

    // LOD 样式配置
    const isHighZoom = zoom >= 17
    const baseRadius = isHighZoom ? 4 : 2
    const radius = Math.max(1, baseRadius * Math.min(1, zoom / 16))

    // 节点颜色: 红色系，高优先级 (连接多路径) 更深
    const highPriorityColor = '#f44336' // 红色
    const normalColor = '#ef9a9a' // 浅红色

    for (const node of this.nodes) {
      const { x, y } = this.lonLatToScreen(node.lon, node.lat)

      // 根据引用计数决定样式
      const isHighPriority = node.refCount >= 2
      const color = isHighPriority ? highPriorityColor : normalColor
      const nodeRadius = isHighPriority ? radius * 1.5 : radius

      ctx.beginPath()
      ctx.arc(x, y, nodeRadius, 0, Math.PI * 2)

      if (isHighZoom) {
        // 高缩放: 红色圆圈 (描边)
        ctx.strokeStyle = color
        ctx.lineWidth = isHighPriority ? 2 : 1
        ctx.stroke()
      } else {
        // 中缩放: 红点 (填充)
        ctx.fillStyle = color
        ctx.fill()
      }
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
        const lon = view.getFloat64(offset, true)
        offset += 8
        const lat = view.getFloat64(offset, true)
        offset += 8

        const { x, y } = this.lonLatToScreen(lon, lat)

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

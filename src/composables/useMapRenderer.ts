/**
 * 地图渲染器组合式函数
 *
 * 连接 Vue 组件与底层渲染引擎。
 * 使用 shallowRef 避免对大型数据结构的深度响应式代理。
 */

import { ref, shallowRef, onMounted, onUnmounted, watch } from 'vue'
import { MapRenderer, type CameraState, type RenderStats } from '../core/map-renderer'
import {
  queryViewportFull,
  decodeViewportResponseV2,
  type Viewport,
} from '../core/ipc-bridge'

export function useMapRenderer(canvasRef: () => HTMLCanvasElement | null) {
  const renderer = shallowRef<MapRenderer | null>(null)
  const camera = ref<CameraState>({
    centerLon: 116.4074,
    centerLat: 39.9042,
    zoom: 12,
  })
  const stats = ref<RenderStats>({
    fps: 0,
    nodeCount: 0,
    wayCount: 0,
    renderTime: 0,
  })
  const isLoading = ref(false)
  const viewport = shallowRef<Viewport | null>(null)

  let statsInterval: ReturnType<typeof setInterval> | null = null
  let debounceTimer: ReturnType<typeof setTimeout> | null = null

  const initialize = () => {
    const canvas = canvasRef()
    if (!canvas) return

    renderer.value = new MapRenderer({ canvas })
    renderer.value.setCamera(camera.value)
    renderer.value.start()

    // 设置相机变化回调，触发数据重新请求
    renderer.value.setOnCameraChange(() => {
      debouncedFetchData()
    })

    statsInterval = setInterval(() => {
      if (renderer.value) {
        stats.value = renderer.value.getStats()
        // 同步相机状态（用户可能通过鼠标操作改变了缩放/位置）
        const currentCamera = renderer.value.getCamera()
        camera.value.centerLon = currentCamera.centerLon
        camera.value.centerLat = currentCamera.centerLat
        camera.value.zoom = currentCamera.zoom
      }
    }, 200)
  }

  const fetchData = async () => {
    if (!renderer.value || isLoading.value) return

    const vp = renderer.value.getViewport()
    viewport.value = vp
    isLoading.value = true

    try {
      const rawData = await queryViewportFull(vp)

      if (rawData.byteLength > 16 && renderer.value) {
        const { nodes, wayGeometry } = decodeViewportResponseV2(rawData.buffer)
        renderer.value.setNodeData(nodes)
        renderer.value.setWayData(wayGeometry)
      }
    } catch (error) {
      console.error('获取视口数据失败:', error)
    } finally {
      isLoading.value = false
    }
  }

  const debouncedFetchData = () => {
    if (debounceTimer) {
      clearTimeout(debounceTimer)
    }
    debounceTimer = setTimeout(fetchData, 300)
  }

  const setCamera = (newCamera: Partial<CameraState>) => {
    Object.assign(camera.value, newCamera)
    renderer.value?.setCamera(newCamera)
    debouncedFetchData()
  }

  const resize = () => {
    renderer.value?.resize()
  }

  watch(camera, (newCamera) => {
    renderer.value?.setCamera(newCamera)
  }, { deep: true })

  onMounted(() => {
    // 使用 requestAnimationFrame 确保 DOM 布局已完成
    // 双重 RAF 确保浏览器已完成布局和绘制
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        initialize()
      })
    })
    window.addEventListener('resize', resize)
  })

  onUnmounted(() => {
    if (statsInterval) {
      clearInterval(statsInterval)
    }
    if (debounceTimer) {
      clearTimeout(debounceTimer)
    }
    renderer.value?.destroy()
    window.removeEventListener('resize', resize)
  })

  return {
    renderer,
    camera,
    stats,
    isLoading,
    viewport,
    setCamera,
    fetchData,
    resize,
  }
}

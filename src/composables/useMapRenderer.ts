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
      console.log(`原始数据: ${rawData.byteLength} 字节`)

      if (rawData.byteLength > 16 && renderer.value) {
        const { nodes, wayGeometry, header } = decodeViewportResponseV2(rawData.buffer)

        console.log(`视口数据: ${header.nodeCount} 节点 (header), ${nodes.length} 节点 (解码), ${header.wayCount} 路径`)
        if (nodes.length > 0) {
          console.log(`首个节点: lon=${nodes[0].lon}, lat=${nodes[0].lat}, refCount=${nodes[0].refCount}`)
        }

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
    initialize()
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

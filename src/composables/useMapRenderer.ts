/**
 * 地图渲染器组合式函数
 *
 * 连接 Vue 组件与底层渲染引擎。
 * 使用 shallowRef 避免对大型数据结构的深度响应式代理。
 */

import { ref, shallowRef, onMounted, onUnmounted, watch } from 'vue'
import {
  MapRenderer,
  type CameraState,
  type RenderStats,
  type SelectedFeature,
} from '../core/map-renderer'
import {
  queryViewportFull,
  decodeViewportResponseV2,
  pickFeature,
  type Viewport,
  type PickedFeature,
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

  // 选中状态
  const selectedFeature = ref<SelectedFeature | null>(null)
  const isPicking = ref(false)

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

    // 设置要素点击回调
    renderer.value.setOnFeatureClick(async (mercX, mercY, toleranceMeters, zoom) => {
      if (isPicking.value) return

      isPicking.value = true
      try {
        const result = await pickFeature(mercX, mercY, toleranceMeters, zoom)
        handlePickResult(result)
      } catch (error) {
        console.error('拾取要素失败:', error)
      } finally {
        isPicking.value = false
      }
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

  const handlePickResult = (result: PickedFeature) => {
    if (result.type === 'None') {
      selectedFeature.value = null
      renderer.value?.clearSelection()
      console.log('未选中任何要素')
    } else {
      const feature: SelectedFeature = {
        type: result.type === 'Node' ? 'node' : 'way',
        id: result.id!,
      }
      selectedFeature.value = feature
      renderer.value?.setSelectedFeature(feature)
      console.log(`选中 ${feature.type}: ${feature.id}`)
    }
  }

  const clearSelection = () => {
    selectedFeature.value = null
    renderer.value?.clearSelection()
  }

  const fetchData = async () => {
    if (!renderer.value || isLoading.value) return

    const vp = renderer.value.getViewport()
    viewport.value = vp
    isLoading.value = true

    try {
      const rawData = await queryViewportFull(vp)

      if (rawData.byteLength > 16 && renderer.value) {
        const { nodes, wayGeometry, polygonGeometry } = decodeViewportResponseV2(rawData.buffer)
        renderer.value.setNodeData(nodes)
        renderer.value.setWayData(wayGeometry)
        renderer.value.setPolygonData(polygonGeometry)
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
    selectedFeature,
    isPicking,
    setCamera,
    fetchData,
    resize,
    clearSelection,
  }
}

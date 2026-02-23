<script setup lang="ts">
/**
 * 地图画布组件
 *
 * 包装底层渲染引擎，提供 Vue 组件接口。
 */

import { ref, defineExpose } from 'vue'
import { useMapRenderer } from '../composables/useMapRenderer'

const canvasRef = ref<HTMLCanvasElement | null>(null)

const {
  renderer,
  camera,
  stats,
  isLoading,
  selectedFeature,
  setCamera,
  fetchData,
  resize,
  clearSelection,
} = useMapRenderer(() => canvasRef.value)

defineExpose({
  renderer,
  camera,
  stats,
  isLoading,
  selectedFeature,
  setCamera,
  fetchData,
  resize,
  clearSelection,
})
</script>

<template>
  <div class="map-canvas-container">
    <canvas ref="canvasRef" class="map-canvas" />

    <div class="map-overlay map-stats">
      <span>FPS: {{ stats.fps }}</span>
      <span>Zoom: {{ Math.floor(camera.zoom) }}</span>
      <span>{{ camera.centerLon.toFixed(4) }}°E, {{ camera.centerLat.toFixed(4) }}°N</span>
      <span>Nodes: {{ stats.nodeCount.toLocaleString() }}</span>
      <span>Ways: {{ stats.wayCount.toLocaleString() }}</span>
      <span>Render: {{ stats.renderTime.toFixed(1) }}ms</span>
    </div>

    <div v-if="isLoading" class="map-overlay map-loading">
      加载中...
    </div>
  </div>
</template>

<style scoped>
.map-canvas-container {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
  background: var(--color-bg-primary);
}

.map-canvas {
  width: 100%;
  height: 100%;
  display: block;
}

.map-overlay {
  position: absolute;
  padding: 8px 12px;
  background: var(--color-bg-overlay);
  border-radius: 6px;
  font-size: 12px;
  font-family: var(--font-mono);
  color: var(--color-text-secondary);
  pointer-events: none;
}

.map-stats {
  top: 12px;
  left: 12px;
  display: flex;
  gap: 16px;
}

.map-loading {
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  font-size: 14px;
  color: var(--color-text-primary);
}
</style>

<script setup lang="ts">
/**
 * 根组件 - IDE 风格布局
 *
 * 使用 Splitpanes 实现可拖拽的面板布局。
 */

import { Splitpanes, Pane } from 'splitpanes'
import 'splitpanes/dist/splitpanes.css'

import Sidebar from './components/Sidebar.vue'
import Toolbar from './components/Toolbar.vue'
import MapCanvas from './components/MapCanvas.vue'
import { ref } from 'vue'

const mapRef = ref<InstanceType<typeof MapCanvas> | null>(null)

const handleZoomIn = () => {
  const cam = mapRef.value?.camera
  if (cam) {
    mapRef.value?.setCamera({ zoom: cam.zoom + 1 })
  }
}

const handleZoomOut = () => {
  const cam = mapRef.value?.camera
  if (cam) {
    mapRef.value?.setCamera({ zoom: Math.max(1, cam.zoom - 1) })
  }
}

const handleZoomFit = () => {
  mapRef.value?.setCamera({ zoom: 2, centerLon: 0, centerLat: 0 })
}

import type { DataBounds } from './core/ipc-bridge'

const handleDataLoaded = (bounds: DataBounds | null) => {
  console.log('PBF 数据加载完成，边界:', bounds)

  if (bounds && mapRef.value) {
    const lonSpan = bounds.max_lon - bounds.min_lon
    const latSpan = bounds.max_lat - bounds.min_lat
    
    // #region agent log - 记录边界和计算过程
    const logData = {
      loc: 'handleDataLoaded',
      lonSpan: lonSpan.toFixed(4),
      latSpan: latSpan.toFixed(4),
      ratio: (lonSpan / latSpan).toFixed(4),
      center: `${bounds.center_lon.toFixed(4)}, ${bounds.center_lat.toFixed(4)}`
    }
    const existingLog = localStorage.getItem('mosm_debug_log') || ''
    localStorage.setItem('mosm_debug_log', existingLog + '\n' + JSON.stringify(logData))
    // #endregion
    
    const maxSpan = Math.max(lonSpan, latSpan)
    const zoom = Math.max(1, Math.min(18, Math.floor(Math.log2(360 / maxSpan))))

    mapRef.value.setCamera({
      centerLon: bounds.center_lon,
      centerLat: bounds.center_lat,
      zoom: zoom,
    })
  }

  mapRef.value?.fetchData()
}
</script>

<template>
  <div class="app-container">
    <div class="app-toolbar">
      <Toolbar
        @zoom-in="handleZoomIn"
        @zoom-out="handleZoomOut"
        @zoom-fit="handleZoomFit"
      />
    </div>

    <div class="app-main">
      <Splitpanes class="default-theme">
        <Pane :size="20" :min-size="15" :max-size="35">
          <Sidebar @data-loaded="handleDataLoaded" />
        </Pane>
        <Pane :size="80">
          <MapCanvas ref="mapRef" />
        </Pane>
      </Splitpanes>
    </div>

    <div class="app-statusbar">
      <span>就绪</span>
      <span class="statusbar-spacer" />
      <span>MOSM Editor</span>
    </div>
  </div>
</template>

<style>
/* ============================================================================
   CSS Variables - 暗黑主题设计系统
   ============================================================================ */

:root {
  /* 背景色 */
  --color-bg-primary: #1e1e1e;
  --color-bg-secondary: #252526;
  --color-bg-tertiary: #2d2d30;
  --color-bg-hover: #3c3c3c;
  --color-bg-overlay: rgba(30, 30, 30, 0.9);

  /* 文字色 */
  --color-text-primary: #e0e0e0;
  --color-text-secondary: #a0a0a0;
  --color-text-muted: #6e6e6e;
  --color-text-inverse: #ffffff;

  /* 强调色 */
  --color-accent: #0078d4;
  --color-accent-hover: #106ebe;
  --color-accent-subtle: rgba(0, 120, 212, 0.2);

  /* 边框 */
  --color-border: #3c3c3c;
  --color-border-focus: #0078d4;

  /* 状态色 */
  --color-error: #f14c4c;
  --color-error-bg: rgba(241, 76, 76, 0.1);
  --color-warning: #cca700;
  --color-success: #89d185;

  /* 字体 */
  --font-sans: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen,
    Ubuntu, Cantarell, sans-serif;
  --font-mono: 'JetBrains Mono', 'Fira Code', 'SF Mono', Consolas, monospace;

  /* 尺寸 */
  --toolbar-height: 48px;
  --statusbar-height: 24px;
}

/* ============================================================================
   全局重置
   ============================================================================ */

*,
*::before,
*::after {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

html,
body,
#app {
  width: 100%;
  height: 100%;
  overflow: hidden;
  font-family: var(--font-sans);
  font-size: 13px;
  line-height: 1.5;
  color: var(--color-text-primary);
  background: var(--color-bg-primary);
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

/* ============================================================================
   Splitpanes 主题覆盖
   ============================================================================ */

.splitpanes.default-theme .splitpanes__pane {
  background: var(--color-bg-primary);
}

.splitpanes.default-theme .splitpanes__splitter {
  background: var(--color-border);
  min-width: 4px;
  min-height: 4px;
}

.splitpanes.default-theme .splitpanes__splitter:hover {
  background: var(--color-accent);
}

.splitpanes.default-theme .splitpanes__splitter::before,
.splitpanes.default-theme .splitpanes__splitter::after {
  display: none;
}

/* ============================================================================
   滚动条样式
   ============================================================================ */

::-webkit-scrollbar {
  width: 10px;
  height: 10px;
}

::-webkit-scrollbar-track {
  background: var(--color-bg-secondary);
}

::-webkit-scrollbar-thumb {
  background: var(--color-bg-hover);
  border-radius: 5px;
}

::-webkit-scrollbar-thumb:hover {
  background: #555;
}
</style>

<style scoped>
.app-container {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
}

.app-toolbar {
  height: var(--toolbar-height);
  flex-shrink: 0;
}

.app-main {
  flex: 1;
  overflow: hidden;
}

.app-statusbar {
  height: var(--statusbar-height);
  flex-shrink: 0;
  display: flex;
  align-items: center;
  padding: 0 12px;
  background: var(--color-bg-secondary);
  border-top: 1px solid var(--color-border);
  font-size: 11px;
  color: var(--color-text-muted);
}

.statusbar-spacer {
  flex: 1;
}
</style>

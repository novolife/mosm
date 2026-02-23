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
import ContextMenu from './components/ContextMenu.vue'
import type { MenuItem } from './components/ContextMenu.vue'
import { ref, watch } from 'vue'
import { getNodeDetails, getWayDetails, type FeatureDetails } from './core/ipc-bridge'
import { useHistory } from './composables/useHistory'

const mapRef = ref<InstanceType<typeof MapCanvas> | null>(null)
const contextMenuRef = ref<InstanceType<typeof ContextMenu> | null>(null)

// 选中要素的详细信息
const selectedFeatureDetails = ref<FeatureDetails | null>(null)

// 监听 MapCanvas 的选中状态变化
watch(
  () => mapRef.value?.selectedFeature,
  async (newFeature) => {
    if (!newFeature) {
      selectedFeatureDetails.value = null
      return
    }

    try {
      if (newFeature.type === 'node') {
        selectedFeatureDetails.value = await getNodeDetails(newFeature.id)
      } else if (newFeature.type === 'way') {
        selectedFeatureDetails.value = await getWayDetails(newFeature.id)
      }
    } catch (error) {
      console.error('获取要素详情失败:', error)
      selectedFeatureDetails.value = null
    }
  },
  { deep: true },
)

// 清除选中状态
const handleClearSelection = () => {
  mapRef.value?.clearSelection()
  selectedFeatureDetails.value = null
}

// 处理标签更新
const handleTagsUpdated = async (renderFeatureChanged: boolean) => {
  // 刷新要素详情
  const feature = mapRef.value?.selectedFeature
  if (feature) {
    try {
      if (feature.type === 'node') {
        selectedFeatureDetails.value = await getNodeDetails(feature.id)
      } else if (feature.type === 'way') {
        selectedFeatureDetails.value = await getWayDetails(feature.id)
      }
    } catch (error) {
      console.error('刷新要素详情失败:', error)
    }
  }

  // 如果渲染特征改变，触发重绘
  if (renderFeatureChanged) {
    console.log('渲染特征已改变，触发重绘')
    mapRef.value?.fetchData()
  }
}

// ============================================================================
// Undo/Redo 历史记录
// ============================================================================

const refreshFeatureDetails = async () => {
  const feature = mapRef.value?.selectedFeature
  if (feature) {
    if (feature.type === 'node') {
      selectedFeatureDetails.value = await getNodeDetails(feature.id)
    } else if (feature.type === 'way') {
      selectedFeatureDetails.value = await getWayDetails(feature.id)
    }
  }
}

useHistory({
  onUndoRedo: async (needsRedraw) => {
    await refreshFeatureDetails()
    if (needsRedraw) {
      mapRef.value?.fetchData()
    }
  },
})

// 右键菜单项配置
const mapContextMenuItems: MenuItem[] = [
  { id: 'add-node', label: '添加节点', shortcut: 'N' },
  { id: 'add-way', label: '添加路径', shortcut: 'W' },
  { id: 'separator-1', label: '', separator: true },
  { id: 'select-all', label: '全选', shortcut: 'Ctrl+A' },
  { id: 'separator-2', label: '', separator: true },
  { id: 'properties', label: '属性', shortcut: 'Alt+Enter' },
]

// 处理地图区域右键
const handleMapContextMenu = (e: MouseEvent) => {
  contextMenuRef.value?.show(e.clientX, e.clientY)
}

// 处理菜单项选择
const handleMenuSelect = (id: string) => {
  console.log('菜单选择:', id)
  // TODO: 实现具体的菜单操作
  switch (id) {
    case 'add-node':
      console.log('添加节点功能待实现')
      break
    case 'add-way':
      console.log('添加路径功能待实现')
      break
    case 'properties':
      console.log('属性面板功能待实现')
      break
  }
}

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
          <Sidebar
            :selected-feature="selectedFeatureDetails"
            @data-loaded="handleDataLoaded"
            @clear-selection="handleClearSelection"
            @tags-updated="handleTagsUpdated"
          />
        </Pane>
        <Pane :size="80">
          <div class="map-wrapper" @contextmenu="handleMapContextMenu">
            <MapCanvas ref="mapRef" />
          </div>
        </Pane>
      </Splitpanes>
    </div>

    <div class="app-statusbar">
      <span>就绪</span>
      <span class="statusbar-spacer" />
      <span>MOSM Editor</span>
    </div>

    <!-- 自定义右键菜单 -->
    <ContextMenu
      ref="contextMenuRef"
      :items="mapContextMenuItems"
      @select="handleMenuSelect"
    />
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

/* ============================================================================
   原生化改造 - 消除"网页感"
   ============================================================================ */

/* 全局禁用文本选中 */
* {
  user-select: none;
  -webkit-user-select: none;
  -webkit-tap-highlight-color: transparent;
}

/* 允许输入框内选中文字 */
input,
textarea,
[contenteditable='true'] {
  user-select: text;
  -webkit-user-select: text;
}

/* 禁用图片拖拽 */
img {
  -webkit-user-drag: none;
  pointer-events: none;
}

/* 禁用 Canvas 的图片保存提示 */
canvas {
  -webkit-touch-callout: none;
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

.map-wrapper {
  width: 100%;
  height: 100%;
}

.statusbar-spacer {
  flex: 1;
}
</style>

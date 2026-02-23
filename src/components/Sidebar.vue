<script setup lang="ts">
/**
 * 侧边栏组件
 *
 * 显示数据统计、工具列表等。
 * 当有选中要素时，显示要素详情面板。
 */

import { useOsmStore } from '../composables/useOsmStore'
import { open } from '@tauri-apps/plugin-dialog'
import { onMounted } from 'vue'
import FeaturePanel from './FeaturePanel.vue'

import type { DataBounds, FeatureDetails } from '../core/ipc-bridge'

const { selectedFeature } = defineProps<{
  selectedFeature: FeatureDetails | null
}>()

const emit = defineEmits<{
  (e: 'data-loaded', bounds: DataBounds | null): void
  (e: 'clear-selection'): void
}>()

const { stats, isLoading, loadProgress, error, refreshStats, openPbfFile } = useOsmStore()

const handleOpenFile = async () => {
  try {
    const selected = await open({
      multiple: false,
      filters: [
        { name: 'OSM PBF', extensions: ['pbf', 'osm.pbf'] },
        { name: '所有文件', extensions: ['*'] },
      ],
    })
    if (selected && typeof selected === 'string') {
      const bounds = await openPbfFile(selected)
      emit('data-loaded', bounds)
    }
  } catch (e) {
    console.error('打开文件失败:', e)
  }
}

const handleCloseFeaturePanel = () => {
  emit('clear-selection')
}

onMounted(() => {
  refreshStats()
})
</script>

<template>
  <aside class="sidebar">
    <!-- 选中要素时显示要素详情面板 -->
    <template v-if="selectedFeature && selectedFeature.type !== 'NotFound'">
      <FeaturePanel :feature="selectedFeature" @close="handleCloseFeaturePanel" />
    </template>

    <!-- 未选中要素时显示默认界面 -->
    <template v-else>
      <div class="sidebar-header">
        <h2>MOSM</h2>
        <span class="version">v0.1.0</span>
      </div>

      <section class="sidebar-section">
        <h3>文件</h3>
        <button class="btn btn-primary" :disabled="isLoading" @click="handleOpenFile">
          {{ isLoading ? '加载中...' : '打开 PBF 文件' }}
        </button>
      </section>

      <section class="sidebar-section">
        <h3>数据统计</h3>
        <div class="stats-grid">
          <div class="stat-item">
            <span class="stat-label">节点</span>
            <span class="stat-value">{{ stats.node_count.toLocaleString() }}</span>
          </div>
          <div class="stat-item">
            <span class="stat-label">路径</span>
            <span class="stat-value">{{ stats.way_count.toLocaleString() }}</span>
          </div>
          <div class="stat-item">
            <span class="stat-label">关系</span>
            <span class="stat-value">{{ stats.relation_count.toLocaleString() }}</span>
          </div>
        </div>
      </section>

      <section v-if="loadProgress" class="sidebar-section">
        <h3>加载详情</h3>
        <div class="progress-info">
          <p>节点: {{ loadProgress.nodes_parsed.toLocaleString() }}</p>
          <p>路径: {{ loadProgress.ways_parsed.toLocaleString() }}</p>
          <p>关系: {{ loadProgress.relations_parsed.toLocaleString() }}</p>
        </div>
      </section>

      <section v-if="error" class="sidebar-section error-section">
        <h3>错误</h3>
        <p class="error-message">{{ error }}</p>
      </section>

      <div class="sidebar-footer">
        <span>Modern OSM Editor</span>
      </div>
    </template>
  </aside>
</template>

<style scoped>
.sidebar {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--color-bg-secondary);
  border-right: 1px solid var(--color-border);
}

.sidebar-header {
  padding: 16px;
  display: flex;
  align-items: baseline;
  gap: 8px;
  border-bottom: 1px solid var(--color-border);
}

.sidebar-header h2 {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
  color: var(--color-text-primary);
}

.version {
  font-size: 11px;
  color: var(--color-text-muted);
  font-family: var(--font-mono);
}

.sidebar-section {
  padding: 16px;
  border-bottom: 1px solid var(--color-border);
}

.sidebar-section h3 {
  margin: 0 0 12px 0;
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: var(--color-text-muted);
}

.btn {
  width: 100%;
  padding: 10px 16px;
  border: none;
  border-radius: 6px;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s ease;
}

.btn-primary {
  background: var(--color-accent);
  color: var(--color-text-inverse);
}

.btn-primary:hover:not(:disabled) {
  background: var(--color-accent-hover);
}

.btn-primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.stats-grid {
  display: grid;
  gap: 12px;
}

.stat-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.stat-label {
  font-size: 13px;
  color: var(--color-text-secondary);
}

.stat-value {
  font-size: 14px;
  font-weight: 600;
  font-family: var(--font-mono);
  color: var(--color-text-primary);
}

.progress-info {
  font-size: 12px;
  font-family: var(--font-mono);
  color: var(--color-text-secondary);
}

.progress-info p {
  margin: 4px 0;
}

.error-section {
  background: var(--color-error-bg);
}

.error-message {
  margin: 0;
  font-size: 12px;
  color: var(--color-error);
  word-break: break-word;
}

.sidebar-footer {
  margin-top: auto;
  padding: 12px 16px;
  font-size: 11px;
  color: var(--color-text-muted);
  text-align: center;
  border-top: 1px solid var(--color-border);
}
</style>

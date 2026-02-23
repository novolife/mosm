<script setup lang="ts">
import { computed } from 'vue'
import type { FeatureDetails } from '../core/ipc-bridge'

const props = defineProps<{
  feature: FeatureDetails | null
}>()

const emit = defineEmits<{
  close: []
}>()

const featureType = computed(() => {
  if (!props.feature || props.feature.type === 'NotFound') return null
  return props.feature.type
})

const featureId = computed(() => {
  if (!props.feature || props.feature.type === 'NotFound') return null
  return props.feature.id
})

const tags = computed(() => {
  if (!props.feature || props.feature.type === 'NotFound') return []
  return props.feature.tags || []
})

const nodeInfo = computed(() => {
  if (props.feature?.type !== 'Node') return null
  return {
    lon: props.feature.lon.toFixed(6),
    lat: props.feature.lat.toFixed(6),
    refCount: props.feature.ref_count,
  }
})

const wayInfo = computed(() => {
  if (props.feature?.type !== 'Way') return null
  return {
    nodeCount: props.feature.node_count,
    isArea: props.feature.is_area,
    layer: props.feature.layer,
  }
})

const parentRelations = computed(() => {
  if (!props.feature || props.feature.type === 'NotFound') return []
  return props.feature.parent_relations || []
})

function formatTagKey(key: string): string {
  return key.replace(/_/g, ' ').replace(/:/g, ': ')
}

function formatRelationType(type: string | null): string {
  if (!type) return '未知类型'
  const typeMap: Record<string, string> = {
    multipolygon: '多边形',
    route: '路线',
    boundary: '边界',
    restriction: '限制',
    building: '建筑',
    waterway: '水系',
    public_transport: '公共交通',
    associatedStreet: '关联街道',
    site: '站点',
    enforcement: '执法',
  }
  return typeMap[type] || type
}
</script>

<template>
  <div class="feature-panel">
    <div class="panel-header">
      <div class="feature-type">
        <span v-if="featureType === 'Node'" class="type-icon node-icon">●</span>
        <span v-else-if="featureType === 'Way'" class="type-icon way-icon">━</span>
        <span class="type-label">{{ featureType === 'Node' ? '节点' : '路径' }}</span>
      </div>
      <div class="feature-id">#{{ featureId }}</div>
      <button class="close-btn" @click="emit('close')" title="取消选择">×</button>
    </div>

    <div class="panel-content">
      <!-- 节点信息 -->
      <div v-if="nodeInfo" class="info-section">
        <div class="section-title">坐标</div>
        <div class="info-grid">
          <div class="info-item">
            <span class="info-label">经度</span>
            <span class="info-value">{{ nodeInfo.lon }}°</span>
          </div>
          <div class="info-item">
            <span class="info-label">纬度</span>
            <span class="info-value">{{ nodeInfo.lat }}°</span>
          </div>
          <div class="info-item">
            <span class="info-label">引用次数</span>
            <span class="info-value">{{ nodeInfo.refCount }}</span>
          </div>
        </div>
      </div>

      <!-- 路径信息 -->
      <div v-if="wayInfo" class="info-section">
        <div class="section-title">几何信息</div>
        <div class="info-grid">
          <div class="info-item">
            <span class="info-label">节点数</span>
            <span class="info-value">{{ wayInfo.nodeCount }}</span>
          </div>
          <div class="info-item">
            <span class="info-label">类型</span>
            <span class="info-value">{{ wayInfo.isArea ? '闭合面' : '线段' }}</span>
          </div>
          <div class="info-item">
            <span class="info-label">图层</span>
            <span class="info-value">{{ wayInfo.layer }}</span>
          </div>
        </div>
      </div>

      <!-- 标签列表 -->
      <div v-if="tags.length > 0" class="info-section tags-section">
        <div class="section-title">标签 ({{ tags.length }})</div>
        <div class="tags-list">
          <div v-for="([key, value], index) in tags" :key="index" class="tag-item">
            <span class="tag-key">{{ formatTagKey(key) }}</span>
            <span class="tag-value">{{ value }}</span>
          </div>
        </div>
      </div>

      <div v-else-if="featureType === 'Node'" class="empty-tags">
        此节点没有标签
      </div>

      <div v-else-if="featureType === 'Way' && tags.length === 0" class="empty-tags">
        此路径没有标签
      </div>

      <!-- 所属关系 -->
      <div v-if="parentRelations.length > 0" class="info-section relations-section">
        <div class="section-title">所属关系 ({{ parentRelations.length }})</div>
        <div class="relations-list">
          <div v-for="relation in parentRelations" :key="relation.id" class="relation-item">
            <div class="relation-header">
              <span class="relation-type-badge">{{ formatRelationType(relation.relation_type) }}</span>
              <span class="relation-id">#{{ relation.id }}</span>
            </div>
            <div v-if="relation.name" class="relation-name">{{ relation.name }}</div>
            <div class="relation-role">
              <span class="role-label">角色:</span>
              <span class="role-value">{{ relation.role || '(无)' }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <div class="panel-footer">
      <button class="action-btn" disabled title="编辑标签（开发中）">
        编辑标签
      </button>
    </div>
  </div>
</template>

<style scoped>
.feature-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--color-bg-secondary);
}

.panel-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px;
  border-bottom: 1px solid var(--color-border);
  background: var(--color-bg-tertiary);
}

.feature-type {
  display: flex;
  align-items: center;
  gap: 6px;
}

.type-icon {
  font-size: 14px;
}

.node-icon {
  color: #f44336;
}

.way-icon {
  color: #ff9800;
}

.type-label {
  font-weight: 600;
  color: var(--color-text-primary);
}

.feature-id {
  flex: 1;
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--color-text-secondary);
}

.close-btn {
  width: 24px;
  height: 24px;
  border: none;
  background: transparent;
  color: var(--color-text-secondary);
  font-size: 18px;
  cursor: pointer;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.close-btn:hover {
  background: var(--color-bg-hover);
  color: var(--color-text-primary);
}

.panel-content {
  flex: 1;
  overflow-y: auto;
  padding: 12px;
}

.info-section {
  margin-bottom: 16px;
}

.section-title {
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  color: var(--color-text-muted);
  margin-bottom: 8px;
  letter-spacing: 0.5px;
}

.info-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}

.info-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.info-label {
  font-size: 11px;
  color: var(--color-text-muted);
}

.info-value {
  font-size: 13px;
  color: var(--color-text-primary);
  font-family: var(--font-mono);
}

.tags-section {
  flex: 1;
}

.tags-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.tag-item {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  padding: 6px 8px;
  background: var(--color-bg-tertiary);
  border-radius: 4px;
  gap: 8px;
}

.tag-key {
  font-size: 12px;
  color: var(--color-accent);
  font-weight: 500;
  flex-shrink: 0;
}

.tag-value {
  font-size: 12px;
  color: var(--color-text-primary);
  text-align: right;
  word-break: break-word;
}

.empty-tags {
  font-size: 12px;
  color: var(--color-text-muted);
  font-style: italic;
  text-align: center;
  padding: 16px;
}

/* 所属关系样式 */
.relations-section {
  border-top: 1px solid var(--color-border);
  padding-top: 12px;
  margin-top: 8px;
}

.relations-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.relation-item {
  padding: 8px;
  background: var(--color-bg-tertiary);
  border-radius: 6px;
  border-left: 3px solid var(--color-accent);
}

.relation-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 4px;
}

.relation-type-badge {
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  padding: 2px 6px;
  background: var(--color-accent-subtle);
  color: var(--color-accent);
  border-radius: 3px;
}

.relation-id {
  font-size: 11px;
  font-family: var(--font-mono);
  color: var(--color-text-muted);
}

.relation-name {
  font-size: 12px;
  color: var(--color-text-primary);
  margin-bottom: 4px;
  font-weight: 500;
}

.relation-role {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
}

.role-label {
  color: var(--color-text-muted);
}

.role-value {
  color: var(--color-text-secondary);
  font-family: var(--font-mono);
  background: var(--color-bg-secondary);
  padding: 1px 4px;
  border-radius: 2px;
}

.panel-footer {
  padding: 12px;
  border-top: 1px solid var(--color-border);
}

.action-btn {
  width: 100%;
  padding: 8px 12px;
  border: 1px solid var(--color-border);
  background: var(--color-bg-tertiary);
  color: var(--color-text-secondary);
  border-radius: 4px;
  font-size: 12px;
  cursor: not-allowed;
}

.action-btn:not(:disabled):hover {
  background: var(--color-bg-hover);
  cursor: pointer;
}
</style>

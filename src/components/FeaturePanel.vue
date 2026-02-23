<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { FeatureDetails } from '../core/ipc-bridge'
import { updateWayTags, updateNodeTags } from '../core/ipc-bridge'

const props = defineProps<{
  feature: FeatureDetails | null
}>()

const emit = defineEmits<{
  close: []
  tagsUpdated: [renderFeatureChanged: boolean]
}>()

// 保存状态
const isSaving = ref(false)

// 可编辑的标签副本
const editableTags = ref<{ key: string; value: string }[]>([])

// 记录原始渲染特征，用于判断是否需要重绘
let originalRenderFeature = 0

// 同步原始标签到可编辑副本
watch(
  () => props.feature,
  (newFeature) => {
    if (newFeature && newFeature.type !== 'NotFound') {
      editableTags.value = (newFeature.tags || []).map(([key, value]) => ({
        key,
        value,
      }))
      if (newFeature.type === 'Way') {
        originalRenderFeature = newFeature.render_feature
      }
    } else {
      editableTags.value = []
    }
  },
  { immediate: true },
)

const featureType = computed(() => {
  if (!props.feature || props.feature.type === 'NotFound') return null
  return props.feature.type
})

const featureId = computed(() => {
  if (!props.feature || props.feature.type === 'NotFound') return null
  return props.feature.id
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

// 是否可编辑（Node 和 Way 都支持）
const canEdit = computed(() => {
  return props.feature?.type === 'Way' || props.feature?.type === 'Node'
})

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

function addTag() {
  editableTags.value.push({ key: '', value: '' })
}

async function removeTag(index: number) {
  editableTags.value.splice(index, 1)
  // 删除后立即保存
  await saveTagsQuietly()
}

// 静默保存标签（失去焦点或删除时调用）
async function saveTagsQuietly() {
  if (!props.feature || props.feature.type === 'NotFound') return
  if (isSaving.value) return

  // 检查是否有正在编辑的不完整标签（key有值但value为空，或反之）
  const hasIncompleteTag = editableTags.value.some(
    (tag) => (tag.key.trim() !== '' && tag.value.trim() === '') ||
             (tag.key.trim() === '' && tag.value.trim() !== '')
  )

  // 如果有不完整的标签，跳过保存（用户可能还在编辑）
  if (hasIncompleteTag) {
    return
  }

  // 过滤掉空的标签
  const validTags = editableTags.value.filter(
    (tag) => tag.key.trim() !== '' && tag.value.trim() !== '',
  )

  isSaving.value = true

  try {
    let result
    if (props.feature.type === 'Way') {
      result = await updateWayTags(
        props.feature.id,
        validTags.map((tag) => [tag.key.trim(), tag.value.trim()]),
      )

      if (result.success) {
        // Way: 判断渲染特征是否改变
        const renderFeatureChanged = result.render_feature !== originalRenderFeature
        originalRenderFeature = result.render_feature
        emit('tagsUpdated', renderFeatureChanged)
      }
    } else if (props.feature.type === 'Node') {
      result = await updateNodeTags(
        props.feature.id,
        validTags.map((tag) => [tag.key.trim(), tag.value.trim()]),
      )

      if (result.success) {
        // Node: 标签不影响渲染，不需要重绘
        emit('tagsUpdated', false)
      }
    }
  } catch (error) {
    console.error('保存标签出错:', error)
  } finally {
    isSaving.value = false
  }
}

// 输入框失去焦点时保存
function handleBlur() {
  saveTagsQuietly()
}

// 回车键时移出焦点并保存
function handleKeyEnter(event: KeyboardEvent) {
  ;(event.target as HTMLInputElement).blur()
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

      <!-- 标签编辑器（实时编辑） -->
      <div class="info-section tags-section">
        <div class="section-header">
          <span class="section-title">标签 ({{ editableTags.length }})</span>
          <span v-if="isSaving" class="saving-indicator">保存中...</span>
        </div>

        <!-- Way 可编辑 -->
        <div v-if="canEdit" class="tags-editor">
          <div
            v-for="(tag, index) in editableTags"
            :key="index"
            class="tag-edit-row"
          >
            <input
              v-model="tag.key"
              type="text"
              class="tag-input key-input"
              placeholder="键"
              @blur="handleBlur"
              @keydown.enter="handleKeyEnter"
            />
            <span class="tag-separator">=</span>
            <input
              v-model="tag.value"
              type="text"
              class="tag-input value-input"
              placeholder="值"
              @blur="handleBlur"
              @keydown.enter="handleKeyEnter"
            />
            <button class="tag-delete-btn" @click="removeTag(index)" title="删除标签">
              ×
            </button>
          </div>
          <button class="add-tag-btn" @click="addTag">
            + 添加标签
          </button>
        </div>

        <div v-else class="empty-tags">
          {{ featureType === 'Node' ? '此节点没有标签' : '此路径没有标签' }}
        </div>
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

.section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.section-title {
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  color: var(--color-text-muted);
  letter-spacing: 0.5px;
}

.saving-indicator {
  font-size: 10px;
  color: var(--color-accent);
  font-style: italic;
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

/* 标签编辑器样式 */
.tags-editor {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.tag-edit-row {
  display: flex;
  align-items: center;
  gap: 4px;
}

.tag-input {
  flex: 1;
  padding: 6px 8px;
  border: 1px solid var(--color-border);
  border-radius: 4px;
  background: var(--color-bg-primary);
  color: var(--color-text-primary);
  font-size: 12px;
  font-family: var(--font-mono);
}

.tag-input:focus {
  outline: none;
  border-color: var(--color-accent);
}

.key-input {
  flex: 0.8;
}

.value-input {
  flex: 1.2;
}

.tag-separator {
  color: var(--color-text-muted);
  font-family: var(--font-mono);
  font-size: 12px;
}

.tag-delete-btn {
  width: 24px;
  height: 24px;
  border: none;
  background: transparent;
  color: var(--color-text-muted);
  font-size: 16px;
  cursor: pointer;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.tag-delete-btn:hover {
  background: rgba(244, 67, 54, 0.2);
  color: #f44336;
}

.add-tag-btn {
  margin-top: 4px;
  padding: 6px 12px;
  border: 1px dashed var(--color-border);
  border-radius: 4px;
  background: transparent;
  color: var(--color-text-muted);
  font-size: 12px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.add-tag-btn:hover {
  border-color: var(--color-accent);
  color: var(--color-accent);
  background: var(--color-accent-subtle);
}
</style>

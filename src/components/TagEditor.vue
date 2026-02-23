<script setup lang="ts">
/**
 * 标签编辑器组件
 *
 * 支持 OSM 实体标签的实时增删改查
 */
import { ref, watch } from 'vue'
import { updateWayTags, updateNodeTags } from '../core/ipc-bridge'

const props = defineProps<{
  tags: [string, string][]
  featureType: 'Node' | 'Way'
  featureId: number
  originalRenderFeature?: number
}>()

const emit = defineEmits<{
  tagsUpdated: [renderFeatureChanged: boolean]
}>()

const isSaving = ref(false)
const editableTags = ref<{ key: string; value: string }[]>([])
let currentRenderFeature = props.originalRenderFeature ?? 0

watch(
  () => props.tags,
  (newTags) => {
    editableTags.value = (newTags || []).map(([key, value]) => ({
      key,
      value,
    }))
  },
  { immediate: true },
)

watch(
  () => props.originalRenderFeature,
  (newValue) => {
    if (newValue !== undefined) {
      currentRenderFeature = newValue
    }
  },
)

function addTag() {
  editableTags.value.push({ key: '', value: '' })
}

async function removeTag(index: number) {
  editableTags.value.splice(index, 1)
  await saveTagsQuietly()
}

async function saveTagsQuietly() {
  if (isSaving.value) return

  // 检查是否有正在编辑的不完整标签
  const hasIncompleteTag = editableTags.value.some(
    (tag) =>
      (tag.key.trim() !== '' && tag.value.trim() === '') ||
      (tag.key.trim() === '' && tag.value.trim() !== ''),
  )

  if (hasIncompleteTag) {
    return
  }

  const validTags = editableTags.value.filter(
    (tag) => tag.key.trim() !== '' && tag.value.trim() !== '',
  )

  isSaving.value = true

  try {
    let result
    if (props.featureType === 'Way') {
      result = await updateWayTags(
        props.featureId,
        validTags.map((tag) => [tag.key.trim(), tag.value.trim()]),
      )

      if (result.success) {
        const renderFeatureChanged = result.render_feature !== currentRenderFeature
        currentRenderFeature = result.render_feature
        emit('tagsUpdated', renderFeatureChanged)
      }
    } else if (props.featureType === 'Node') {
      result = await updateNodeTags(
        props.featureId,
        validTags.map((tag) => [tag.key.trim(), tag.value.trim()]),
      )

      if (result.success) {
        emit('tagsUpdated', false)
      }
    }
  } catch (error) {
    console.error('保存标签出错:', error)
  } finally {
    isSaving.value = false
  }
}

function handleBlur() {
  saveTagsQuietly()
}

function handleKeyEnter(event: KeyboardEvent) {
  ;(event.target as HTMLInputElement).blur()
}
</script>

<template>
  <div class="tag-editor">
    <div class="editor-header">
      <span class="tag-count">标签 ({{ editableTags.length }})</span>
      <span v-if="isSaving" class="saving-indicator">保存中...</span>
    </div>

    <div class="tags-list">
      <div v-for="(tag, index) in editableTags" :key="index" class="tag-row">
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
        <button class="delete-btn" @click="removeTag(index)" title="删除标签">×</button>
      </div>
    </div>

    <button class="add-btn" @click="addTag">+ 添加标签</button>
  </div>
</template>

<style scoped>
.tag-editor {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.editor-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.tag-count {
  font-weight: 600;
  font-size: 12px;
  color: var(--color-text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.saving-indicator {
  font-size: 11px;
  color: var(--color-text-secondary);
  font-style: italic;
}

.tags-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.tag-row {
  display: flex;
  align-items: center;
  gap: 4px;
}

.tag-input {
  flex: 1;
  padding: 4px 8px;
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
  max-width: 120px;
}

.tag-separator {
  color: var(--color-text-secondary);
  font-family: var(--font-mono);
}

.delete-btn {
  width: 20px;
  height: 20px;
  border: none;
  background: transparent;
  color: var(--color-text-secondary);
  font-size: 14px;
  cursor: pointer;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.delete-btn:hover {
  background: var(--color-error);
  color: white;
}

.add-btn {
  padding: 6px 12px;
  border: 1px dashed var(--color-border);
  border-radius: 4px;
  background: transparent;
  color: var(--color-text-secondary);
  font-size: 12px;
  cursor: pointer;
  transition: all 0.15s;
}

.add-btn:hover {
  border-color: var(--color-accent);
  color: var(--color-accent);
  background: rgba(var(--color-accent-rgb), 0.1);
}
</style>

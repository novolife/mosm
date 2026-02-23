<script setup lang="ts">
/**
 * 顶部工具栏
 *
 * 包含编辑工具、视图控制等。
 */

import { ref } from 'vue'

const emit = defineEmits<{
  (e: 'tool-change', tool: string): void
  (e: 'zoom-in'): void
  (e: 'zoom-out'): void
  (e: 'zoom-fit'): void
}>()

const activeTool = ref('select')

const tools = [
  { id: 'select', icon: '⊙', label: '选择' },
  { id: 'node', icon: '●', label: '添加节点' },
  { id: 'way', icon: '━', label: '绘制路径' },
  { id: 'area', icon: '▢', label: '绘制区域' },
]

const setTool = (toolId: string) => {
  activeTool.value = toolId
  emit('tool-change', toolId)
}
</script>

<template>
  <header class="toolbar">
    <div class="toolbar-group">
      <button
        v-for="tool in tools"
        :key="tool.id"
        class="tool-btn"
        :class="{ active: activeTool === tool.id }"
        :title="tool.label"
        @click="setTool(tool.id)"
      >
        {{ tool.icon }}
      </button>
    </div>

    <div class="toolbar-spacer" />

    <div class="toolbar-group">
      <button class="tool-btn" title="放大" @click="emit('zoom-in')">+</button>
      <button class="tool-btn" title="缩小" @click="emit('zoom-out')">−</button>
      <button class="tool-btn" title="适应视图" @click="emit('zoom-fit')">◎</button>
    </div>
  </header>
</template>

<style scoped>
.toolbar {
  display: flex;
  align-items: center;
  height: 100%;
  padding: 0 12px;
  background: var(--color-bg-secondary);
  border-bottom: 1px solid var(--color-border);
}

.toolbar-group {
  display: flex;
  gap: 4px;
}

.toolbar-spacer {
  flex: 1;
}

.tool-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--color-text-secondary);
  font-size: 16px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.tool-btn:hover {
  background: var(--color-bg-hover);
  color: var(--color-text-primary);
}

.tool-btn.active {
  background: var(--color-accent);
  color: var(--color-text-inverse);
}
</style>

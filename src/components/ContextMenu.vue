<script setup lang="ts">
/**
 * 自定义右键菜单组件
 *
 * 提供原生感的上下文菜单，替代浏览器默认菜单。
 */

import { ref, onMounted, onUnmounted, computed } from 'vue'

export interface MenuItem {
  id: string
  label: string
  icon?: string
  shortcut?: string
  disabled?: boolean
  separator?: boolean
}

interface Props {
  items?: MenuItem[]
}

const { items } = withDefaults(defineProps<Props>(), {
  items: () => [
    { id: 'add-node', label: '添加节点', shortcut: 'N' },
    { id: 'separator-1', label: '', separator: true },
    { id: 'properties', label: '属性', shortcut: 'Alt+Enter' },
  ],
})

const emit = defineEmits<{
  (e: 'select', id: string): void
  (e: 'close'): void
}>()

const visible = ref(false)
const x = ref(0)
const y = ref(0)
const menuRef = ref<HTMLElement | null>(null)

// 显示菜单
const show = (clientX: number, clientY: number) => {
  x.value = clientX
  y.value = clientY
  visible.value = true

  // 下一帧检查边界
  requestAnimationFrame(() => {
    if (!menuRef.value) return

    const rect = menuRef.value.getBoundingClientRect()
    const viewportWidth = window.innerWidth
    const viewportHeight = window.innerHeight

    // 如果超出右边界，向左偏移
    if (x.value + rect.width > viewportWidth) {
      x.value = viewportWidth - rect.width - 8
    }

    // 如果超出下边界，向上偏移
    if (y.value + rect.height > viewportHeight) {
      y.value = viewportHeight - rect.height - 8
    }
  })
}

// 隐藏菜单
const hide = () => {
  visible.value = false
  emit('close')
}

// 处理菜单项点击
const handleItemClick = (item: MenuItem) => {
  if (item.disabled || item.separator) return
  emit('select', item.id)
  hide()
}

// 点击外部关闭
const handleClickOutside = (e: MouseEvent) => {
  if (menuRef.value && !menuRef.value.contains(e.target as Node)) {
    hide()
  }
}

// ESC 键关闭
const handleKeydown = (e: KeyboardEvent) => {
  if (e.key === 'Escape') {
    hide()
  }
}

onMounted(() => {
  document.addEventListener('mousedown', handleClickOutside)
  document.addEventListener('keydown', handleKeydown)
})

onUnmounted(() => {
  document.removeEventListener('mousedown', handleClickOutside)
  document.removeEventListener('keydown', handleKeydown)
})

// 计算样式
const menuStyle = computed(() => ({
  left: `${x.value}px`,
  top: `${y.value}px`,
}))

// 暴露方法给父组件
defineExpose({ show, hide })
</script>

<template>
  <Teleport to="body">
    <Transition name="context-menu">
      <div
        v-if="visible"
        ref="menuRef"
        class="context-menu"
        :style="menuStyle"
      >
        <template v-for="item in items" :key="item.id">
          <div v-if="item.separator" class="menu-separator" />
          <div
            v-else
            class="menu-item"
            :class="{ disabled: item.disabled }"
            @click="handleItemClick(item)"
          >
            <span class="menu-label">{{ item.label }}</span>
            <span v-if="item.shortcut" class="menu-shortcut">{{
              item.shortcut
            }}</span>
          </div>
        </template>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.context-menu {
  position: fixed;
  z-index: 10000;
  min-width: 180px;
  padding: 4px 0;
  background: var(--color-bg-tertiary);
  border: 1px solid var(--color-border);
  border-radius: 6px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
}

.menu-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 12px;
  cursor: pointer;
  transition: background-color 0.1s;
}

.menu-item:hover {
  background: var(--color-bg-hover);
}

.menu-item.disabled {
  opacity: 0.5;
  pointer-events: none;
}

.menu-label {
  font-size: 13px;
  color: var(--color-text-primary);
}

.menu-shortcut {
  margin-left: 24px;
  font-size: 11px;
  color: var(--color-text-muted);
}

.menu-separator {
  height: 1px;
  margin: 4px 8px;
  background: var(--color-border);
}

/* 动画 */
.context-menu-enter-active,
.context-menu-leave-active {
  transition:
    opacity 0.15s ease,
    transform 0.15s ease;
}

.context-menu-enter-from,
.context-menu-leave-to {
  opacity: 0;
  transform: scale(0.95);
}
</style>

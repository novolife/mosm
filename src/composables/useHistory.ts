/**
 * 历史记录 (Undo/Redo) Composable
 *
 * 管理撤销/重做操作和全局快捷键
 */

import { onMounted, onUnmounted, ref } from 'vue'
import { undo as ipcUndo, redo as ipcRedo, getHistoryState } from '../core/ipc-bridge'

export interface UseHistoryOptions {
  onUndoRedo?: (needsRedraw: boolean) => void
}

export function useHistory(options: UseHistoryOptions = {}) {
  const undoCount = ref(0)
  const redoCount = ref(0)
  const isProcessing = ref(false)

  const handleUndo = async () => {
    if (isProcessing.value) return
    isProcessing.value = true

    try {
      const result = await ipcUndo()
      if (result.success) {
        undoCount.value = result.undo_count
        redoCount.value = result.redo_count
        options.onUndoRedo?.(result.needs_redraw)
      }
    } catch (error) {
      console.error('撤销失败:', error)
    } finally {
      isProcessing.value = false
    }
  }

  const handleRedo = async () => {
    if (isProcessing.value) return
    isProcessing.value = true

    try {
      const result = await ipcRedo()
      if (result.success) {
        undoCount.value = result.undo_count
        redoCount.value = result.redo_count
        options.onUndoRedo?.(result.needs_redraw)
      }
    } catch (error) {
      console.error('重做失败:', error)
    } finally {
      isProcessing.value = false
    }
  }

  const handleKeyDown = (e: KeyboardEvent) => {
    const target = e.target as HTMLElement
    if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable) {
      return
    }

    // Ctrl+Z / Cmd+Z = Undo
    if ((e.ctrlKey || e.metaKey) && !e.shiftKey && e.key.toLowerCase() === 'z') {
      e.preventDefault()
      handleUndo()
      return
    }

    // Ctrl+Shift+Z / Cmd+Shift+Z = Redo
    // Ctrl+Y / Cmd+Y = Redo (Windows)
    if (
      ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === 'z') ||
      ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'y')
    ) {
      e.preventDefault()
      handleRedo()
      return
    }
  }

  const refreshHistoryState = async () => {
    try {
      const [undo, redo] = await getHistoryState()
      undoCount.value = undo
      redoCount.value = redo
    } catch (error) {
      console.error('获取历史状态失败:', error)
    }
  }

  onMounted(() => {
    window.addEventListener('keydown', handleKeyDown)
    refreshHistoryState()
  })

  onUnmounted(() => {
    window.removeEventListener('keydown', handleKeyDown)
  })

  return {
    undoCount,
    redoCount,
    isProcessing,
    handleUndo,
    handleRedo,
    refreshHistoryState,
  }
}

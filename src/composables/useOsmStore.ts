/**
 * OSM 数据存储组合式函数
 *
 * 管理与 Rust 后端的数据交互。
 */

import { ref, shallowRef } from 'vue'
import {
  getStats,
  getBounds,
  loadPbf,
  type StoreStats,
  type ParseProgress,
  type DataBounds,
} from '../core/ipc-bridge'

export function useOsmStore() {
  const stats = ref<StoreStats>({
    node_count: 0,
    way_count: 0,
    relation_count: 0,
  })
  const isLoading = ref(false)
  const loadProgress = shallowRef<ParseProgress | null>(null)
  const bounds = shallowRef<DataBounds | null>(null)
  const error = ref<string | null>(null)

  const refreshStats = async () => {
    try {
      stats.value = await getStats()
    } catch (e) {
      error.value = String(e)
    }
  }

  const openPbfFile = async (path: string): Promise<DataBounds | null> => {
    isLoading.value = true
    error.value = null
    loadProgress.value = null

    try {
      const progress = await loadPbf(path)
      loadProgress.value = progress
      await refreshStats()

      const dataBounds = await getBounds()
      bounds.value = dataBounds
      return dataBounds
    } catch (e) {
      error.value = String(e)
      return null
    } finally {
      isLoading.value = false
    }
  }

  return {
    stats,
    isLoading,
    loadProgress,
    bounds,
    error,
    refreshStats,
    openPbfFile,
  }
}

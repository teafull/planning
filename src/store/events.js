import { ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { ElMessage } from 'element-plus'

// 事件存储 —— SQLite 后端(~/Documents/planning/planning.db)
// 视图使用方式不变: import { events, loadEvents } from '../store/events'

export const events = ref([])

let hydrated = false
let saveTimer = null
let lastErrorToast = 0

export const loadEvents = async () => {
  try {
    const rows = await invoke('db_get_events')
    events.value = Array.isArray(rows) ? rows : []
  } catch (e) {
    console.error('加载事件失败(SQLite):', e)
    events.value = []
  } finally {
    hydrated = true
  }
}

const persistEvents = async () => {
  if (!hydrated) return
  try {
    await invoke('db_save_events', { events: events.value })
  } catch (e) {
    console.error('保存事件失败(SQLite):', e)
    // 集群不可用等场景: 提示用户(限频5s, 避免拖拽重试刷屏)
    const now = Date.now()
    if (now - lastErrorToast > 5000) {
      lastErrorToast = now
      ElMessage({
        type: 'error',
        duration: 6000,
        message: `事件保存失败: ${typeof e === 'string' ? e : String(e)}`
      })
    }
  }
}

// 防抖 500ms, 避免拖拽等高频操作打库
const scheduleSave = () => {
  if (!hydrated) return
  clearTimeout(saveTimer)
  saveTimer = setTimeout(persistEvents, 500)
}

watch(events, scheduleSave, { deep: true })

// 立即落盘(退出前/同步前调用)
export const flushEvents = () => {
  clearTimeout(saveTimer)
  return persistEvents()
}

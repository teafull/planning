import { invoke } from '@tauri-apps/api/core'
import { loadEvents } from './events'
import { hydrateSettings } from './settings'

// 启动初始化: localStorage 旧数据一次性迁移到 SQLite, 然后水合各 store
// main.js 在 mount 前 await initStores()

const lsParse = (key) => {
  try {
    const raw = localStorage.getItem(key)
    return raw ? JSON.parse(raw) : null
  } catch {
    return null
  }
}

// localStorage 键 → SQLite kv 键
const LEGACY_KV_MAP = {
  'tool-quadrant-tasks': 'tasks.quadrants',
  'tool-quick-notes': 'tasks.quickNotes',
  'performance-items': 'performance.items',
  'performance-decomposition': 'performance.decompositions'
}

const migrateLegacyData = async () => {
  const kv = {}

  const settings = lsParse('app-settings')
  if (settings) kv['settings'] = JSON.stringify(settings)

  for (const [lsKey, dbKey] of Object.entries(LEGACY_KV_MAP)) {
    const raw = localStorage.getItem(lsKey)
    if (!raw) continue
    try {
      JSON.parse(raw) // 校验合法 JSON 才迁移
      kv[dbKey] = raw
    } catch {
      // 脏数据跳过
    }
  }

  if (Object.keys(kv).length) {
    await invoke('db_set_kv_multi', { map: kv })
  }

  const legacyEvents = lsParse('calendar-events')
  if (Array.isArray(legacyEvents) && legacyEvents.length) {
    await invoke('db_save_events', { events: legacyEvents })
  }
}

export const initStores = async () => {
  try {
    // meta.booted 标记: 首次启动才检测迁移(用户确实无数据时不重复写)
    let booted = null
    try {
      booted = await invoke('db_get_kv', { key: 'meta.booted' })
    } catch {
      // db 未就绪时由 load/hydrate 报错兜底
    }

    if (!booted) {
      await migrateLegacyData()
      await invoke('db_set_kv', { key: 'meta.booted', value: '1' })
      console.log('[存储迁移] localStorage 旧数据已迁入 SQLite')
    }
  } catch (e) {
    console.error('存储迁移失败:', e)
  }

  await Promise.all([loadEvents(), hydrateSettings()])
}

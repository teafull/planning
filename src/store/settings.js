import { ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'

// 应用设置 —— SQLite KV 后端(key: 'settings', 整体 JSON)
// 对外导出与旧版完全一致

const DEFAULT_SETTINGS = {
  profile: {
    name: '',
    title: '',
    email: ''
  },
  preferences: {
    theme: 'system',
    density: 'default',
    language: 'zh-CN',
    weekStart: 'monday',
    holidayApiUrl: 'https://timor.tech/api/holiday/year/{year}?country=CN&type=YEAR',
    dayStartTime: '08:00',
    dayEndTime: '22:00'
  },
  notifications: {
    desktop: true,
    email: true,
    weeklyReport: true,
    dailyDigest: false
  },
  privacy: {
    showInStats: true,
    shareFocusTime: false
  },
  storage: {
    autoBackup: true,
    backupFrequency: 'weekly'
  },
  ai: {
    models: [
      {
        id: 'default',
        name: '默认模型',
        modelUrl: '',
        apiKey: '',
        modelName: 'gpt-3.5-turbo',
        maxTokens: 2000,
        temperature: 0.7,
        enabled: true
      }
    ],
    selectedModelId: 'default'
  }
}

const initial = structuredClone(DEFAULT_SETTINGS)

export const profile = ref(initial.profile)
export const preferences = ref(initial.preferences)
export const notifications = ref(initial.notifications)
export const privacy = ref(initial.privacy)
export const storage = ref(initial.storage)
export const ai = ref(initial.ai)

let hydrated = false
let saveTimer = null

const collect = () => ({
  profile: profile.value,
  preferences: preferences.value,
  notifications: notifications.value,
  privacy: privacy.value,
  storage: storage.value,
  ai: ai.value
})

const persist = async () => {
  if (!hydrated) return
  try {
    await invoke('db_set_kv', { key: 'settings', value: JSON.stringify(collect()) })
  } catch (e) {
    console.error('保存设置失败(SQLite):', e)
  }
}

const scheduleSave = () => {
  if (!hydrated) return
  clearTimeout(saveTimer)
  saveTimer = setTimeout(persist, 500)
}

watch([profile, preferences, notifications, privacy, storage, ai], scheduleSave, { deep: true })

/// 启动时从 SQLite 水合(深合并, 保留新增默认字段)
export const hydrateSettings = async () => {
  try {
    const raw = await invoke('db_get_kv', { key: 'settings' })
    if (raw) {
      const saved = JSON.parse(raw)
      Object.assign(profile.value, DEFAULT_SETTINGS.profile, saved.profile)
      Object.assign(preferences.value, DEFAULT_SETTINGS.preferences, saved.preferences)
      Object.assign(notifications.value, DEFAULT_SETTINGS.notifications, saved.notifications)
      Object.assign(privacy.value, DEFAULT_SETTINGS.privacy, saved.privacy)
      Object.assign(storage.value, DEFAULT_SETTINGS.storage, saved.storage)
      Object.assign(ai.value, DEFAULT_SETTINGS.ai, saved.ai)
    }
  } catch (e) {
    console.error('加载设置失败(SQLite):', e)
  } finally {
    hydrated = true
  }
}

export const resetSettings = () => {
  const defaults = structuredClone(DEFAULT_SETTINGS)
  profile.value = defaults.profile
  preferences.value = defaults.preferences
  notifications.value = defaults.notifications
  privacy.value = defaults.privacy
  storage.value = defaults.storage
  ai.value = defaults.ai
}

// AI模型管理函数
export const addModel = (modelData) => {
  const newModel = {
    id: Date.now().toString(),
    name: modelData.name || '新模型',
    modelUrl: modelData.modelUrl || '',
    apiKey: modelData.apiKey || '',
    modelName: modelData.modelName || 'gpt-3.5-turbo',
    maxTokens: modelData.maxTokens || 2000,
    temperature: modelData.temperature || 0.7,
    enabled: true
  }
  ai.value.models.push(newModel)
  return newModel.id
}

export const removeModel = (modelId) => {
  const index = ai.value.models.findIndex(m => m.id === modelId)
  if (index > -1) {
    ai.value.models.splice(index, 1)
    // 如果删除的是当前选中的模型，则选择第一个可用的模型
    if (ai.value.selectedModelId === modelId) {
      const firstModel = ai.value.models.find(m => m.enabled)
      ai.value.selectedModelId = firstModel ? firstModel.id : ''
    }
  }
}

export const updateModel = (modelId, updates) => {
  const model = ai.value.models.find(m => m.id === modelId)
  if (model) {
    Object.assign(model, updates)
  }
}

export const getSelectedModel = () => {
  return ai.value.models.find(m => m.id === ai.value.selectedModelId)
}

export const getEnabledModels = () => {
  return ai.value.models.filter(m => m.enabled)
}

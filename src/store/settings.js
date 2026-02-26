import { ref, watch } from 'vue'

const STORAGE_KEY = 'app-settings'

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
  }
}

const loadFromStorage = () => {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return structuredClone(DEFAULT_SETTINGS)
    const saved = JSON.parse(raw)
    // 深合并，保证新增默认字段不会丢失
    return {
      profile: { ...DEFAULT_SETTINGS.profile, ...saved.profile },
      preferences: { ...DEFAULT_SETTINGS.preferences, ...saved.preferences },
      notifications: { ...DEFAULT_SETTINGS.notifications, ...saved.notifications },
      privacy: { ...DEFAULT_SETTINGS.privacy, ...saved.privacy },
      storage: { ...DEFAULT_SETTINGS.storage, ...saved.storage }
    }
  } catch {
    return structuredClone(DEFAULT_SETTINGS)
  }
}

const initial = loadFromStorage()

export const profile = ref(initial.profile)
export const preferences = ref(initial.preferences)
export const notifications = ref(initial.notifications)
export const privacy = ref(initial.privacy)
export const storage = ref(initial.storage)

const saveToStorage = () => {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({
      profile: profile.value,
      preferences: preferences.value,
      notifications: notifications.value,
      privacy: privacy.value,
      storage: storage.value
    }))
  } catch {
    // ignore
  }
}

export const resetSettings = () => {
  const defaults = structuredClone(DEFAULT_SETTINGS)
  profile.value = defaults.profile
  preferences.value = defaults.preferences
  notifications.value = defaults.notifications
  privacy.value = defaults.privacy
  storage.value = defaults.storage
}

watch([profile, preferences, notifications, privacy, storage], saveToStorage, { deep: true })

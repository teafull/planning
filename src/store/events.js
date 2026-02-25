import { ref, watch } from 'vue'

const STORAGE_KEY = 'calendar-events'

export const events = ref([])

export const loadEvents = () => {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    events.value = raw ? JSON.parse(raw) : []
  } catch (error) {
    events.value = []
  }
}

const saveEvents = () => {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(events.value))
  } catch (error) {
    // ignore
  }
}

loadEvents()

watch(events, saveEvents, { deep: true })

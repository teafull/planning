import { ref } from 'vue'
import { events } from './events'

// 事件剪贴板（跨视图共享：周视图复制 → 月视图粘贴）
// clipboard.value = { event: 事件快照, copiedAt: 时间戳 }
export const clipboard = ref(null)

// 生成唯一 id（避免快速连续操作时 Date.now() 撞车）
const genId = () => {
  if (typeof crypto !== 'undefined' && crypto.randomUUID) {
    return crypto.randomUUID()
  }
  return `${Date.now()}-${Math.random().toString(36).slice(2, 10)}`
}

// 本地时区安全的 YYYY-MM-DD
const toYMD = (d) => {
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${day}`
}

const parseYMD = (s) => new Date(`${s}T00:00:00`)

const clone = (obj) => JSON.parse(JSON.stringify(obj))

// 复制事件到剪贴板（快照，源事件后续修改/删除不影响粘贴）
export const copyEvent = (event) => {
  clipboard.value = { event: clone(event), copiedAt: Date.now() }
  return clipboard.value
}

export const clearClipboard = () => {
  clipboard.value = null
}

/**
 * 将事件副本放置到指定日期/时间槽（保持时长与内容）
 * @param {object} srcEvent 源事件
 * @param {string} dateStr 目标日期 YYYY-MM-DD
 * @param {number|null} startSlot 目标开始时间（小时数值，如 14 / 14.5）；null 表示保持原时间
 * @param {object|null} range 可用时间范围 { min, max }（小时），用于越界钳制
 * @returns 新事件对象
 */
export const placeEventCopy = (srcEvent, dateStr, startSlot = null, range = null) => {
  const newEvent = clone(srcEvent)
  newEvent.id = genId()
  newEvent.date = dateStr

  if (newEvent.isAllDay) {
    // 全天事件：保持天数跨度
    const srcEnd = srcEvent.endDate && srcEvent.endDate !== srcEvent.date
      ? parseYMD(srcEvent.endDate)
      : parseYMD(srcEvent.date)
    const spanDays = Math.max(0, Math.round((srcEnd - parseYMD(srcEvent.date)) / 86400000))
    const newStart = parseYMD(dateStr)
    newEvent.endDate = toYMD(new Date(newStart.getTime() + spanDays * 86400000))
  } else if (startSlot !== null) {
    // 保持时长落在目标槽；越界时优先保时长向前挪，仍不够则截断
    const duration = srcEvent.endTime - srcEvent.startTime
    let start = startSlot
    let end = start + duration
    if (range) {
      if (end > range.max) {
        end = range.max
        start = Math.max(range.min, end - duration)
      }
      if (start < range.min) start = range.min
      if (end > range.max) end = range.max
    }
    newEvent.startTime = start
    newEvent.endTime = end
  }

  events.value.push(newEvent)
  return newEvent
}

// 粘贴剪贴板事件（返回新事件；剪贴板为空返回 null）
export const pasteEvent = (dateStr, startSlot = null, range = null) => {
  if (!clipboard.value) return null
  return placeEventCopy(clipboard.value.event, dateStr, startSlot, range)
}

// 原位复制（dayOffset 日期偏移，如 +7 = 复制到下周同一天）
export const duplicateEvent = (event, dayOffset = 0) => {
  const newEvent = clone(event)
  newEvent.id = genId()
  if (dayOffset !== 0) {
    const d = parseYMD(event.date)
    d.setDate(d.getDate() + dayOffset)
    newEvent.date = toYMD(d)
    if (event.endDate) {
      const e = parseYMD(event.endDate)
      e.setDate(e.getDate() + dayOffset)
      newEvent.endDate = toYMD(e)
    }
  }
  events.value.push(newEvent)
  return newEvent
}

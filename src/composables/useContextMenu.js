// 右键菜单组合式函数：事件菜单(复制/下周/另存副本/编辑/删除) + 格子菜单(粘贴/新建)
import { ref, onMounted, onUnmounted } from 'vue'
import { events } from '../store/events'
import { clipboard, copyEvent, pasteEvent, duplicateEvent } from '../store/clipboard'
import { formatTime } from '../utils/eventTypes'

export function useContextMenu({
  START_HOUR,
  END_HOUR,
  formatDateLocal,
  notify,
  openEventDialog,   // (event) => void
  openCreateDialog,  // ({date, startTime, endTime, ...}) => void
  deleteEventItem    // (event) => void
}) {
  const ctxMenu = ref(null) // { x, y, kind: 'event'|'slot', event?, day?, slot? }

  const onEventContextMenu = (e, event) => {
    e.preventDefault()
    e.stopPropagation()
    ctxMenu.value = {
      x: Math.min(e.clientX, window.innerWidth - 190),
      y: Math.min(e.clientY, window.innerHeight - 250),
      kind: 'event',
      event
    }
  }

  const onSlotContextMenu = (e, day, slot) => {
    e.preventDefault()
    ctxMenu.value = {
      x: Math.min(e.clientX, window.innerWidth - 190),
      y: Math.min(e.clientY, window.innerHeight - 140),
      kind: 'slot',
      day,
      slot: slot.value
    }
  }

  const closeCtxMenu = () => { ctxMenu.value = null }

  // 点击菜单外部关闭（点击菜单内部不关闭）
  const onOutsideMouseDown = (e) => {
    if (ctxMenu.value && !(e.target && e.target.closest && e.target.closest('.ctx-menu'))) {
      closeCtxMenu()
    }
  }

  const doCopy = () => {
    const ev = ctxMenu.value?.event
    if (!ev) return
    copyEvent(ev)
    notify('已复制', `「${ev.title || '未命名'}」已复制，在目标格子右键可粘贴`)
    closeCtxMenu()
  }

  const doCopyNextWeek = () => {
    const ev = ctxMenu.value?.event
    if (!ev) return
    const src = events.value.find(e => e.id === ev.id) || ev
    const newEvent = duplicateEvent(src, 7)
    notify('已复制到下周', `「${newEvent.title || '未命名'}」→ ${newEvent.date}`)
    closeCtxMenu()
  }

  // 以副本模式打开编辑弹窗（保存时作为新事件添加，不覆盖原事件）
  const doSaveAsCopy = () => {
    const ev = ctxMenu.value?.event
    if (!ev) return
    const src = events.value.find(e => e.id === ev.id) || ev
    openEventDialog({ ...src, id: Date.now() })
    closeCtxMenu()
  }

  const doCtxEdit = () => {
    const ev = ctxMenu.value?.event
    if (!ev) return
    openEventDialog(ev)
    closeCtxMenu()
  }

  const doCtxDelete = () => {
    const ev = ctxMenu.value?.event
    if (!ev) return
    deleteEventItem(ev)
    closeCtxMenu()
  }

  const doPaste = () => {
    const m = ctxMenu.value
    if (!m || !clipboard.value) return
    const newEvent = pasteEvent(formatDateLocal(m.day), m.slot, { min: START_HOUR.value, max: END_HOUR.value })
    if (newEvent) {
      const timeText = newEvent.isAllDay ? '全天' : `${formatTime(newEvent.startTime)}-${formatTime(newEvent.endTime)}`
      notify('已粘贴', `「${newEvent.title || '未命名'}」→ ${newEvent.date} ${timeText}`)
    }
    closeCtxMenu()
  }

  // 在目标格子新建事件（复用创建流程）
  const doCreateAt = () => {
    const m = ctxMenu.value
    if (!m) return
    const dateStr = formatDateLocal(m.day)
    openCreateDialog({
      id: Date.now(),
      date: dateStr,
      startTime: m.slot,
      endTime: Math.min(m.slot + 1, END_HOUR.value),
      title: '',
      type: 'task',
      isAllDay: false
    })
    closeCtxMenu()
  }

  onMounted(() => {
    document.addEventListener('mousedown', onOutsideMouseDown)
  })
  onUnmounted(() => {
    document.removeEventListener('mousedown', onOutsideMouseDown)
  })

  return {
    ctxMenu,
    onEventContextMenu,
    onSlotContextMenu,
    closeCtxMenu,
    onOutsideMouseDown,
    doCopy,
    doCopyNextWeek,
    doSaveAsCopy,
    doCtxEdit,
    doCtxDelete,
    doPaste,
    doCreateAt
  }
}

// Ctrl+拖拽复制组合式函数：光标位置即副本开始时间, 实时预览块跟随
import { ref, onMounted, onUnmounted } from 'vue'
import { events } from '../store/events'
import { placeEventCopy } from '../store/clipboard'
import { formatTime } from '../utils/eventTypes'

export function useCopyDrag({ START_HOUR, END_HOUR, openEventDialog, notify }) {
  const dragCopy = ref(null) // { src, startX, startY, started, ghost: {x,y}, drop: {...}|null }

  // 光标 → 目标时间槽（吸附 0.5h，返回槽元素矩形供预览定位）
  const cursorToSlot = (e) => {
    const el = document.elementFromPoint(e.clientX, e.clientY)
    const slotEl = el && el.closest ? el.closest('.time-slot[data-date]') : null
    if (!slotEl) return null
    const rect = slotEl.getBoundingClientRect()
    const value = parseFloat(slotEl.dataset.value)
    const snapped = Math.round((value + (e.clientY - rect.top) / 30) * 2) / 2
    return { date: slotEl.dataset.date, slot: snapped, rect }
  }

  // 计算落点（光标位置即副本开始时间，越界时保时长前移）与预览几何
  const computeDrop = (e, dc) => {
    const pos = cursorToSlot(e)
    if (!pos) { dc.drop = null; return }
    const duration = dc.src.endTime - dc.src.startTime
    const start = Math.max(START_HOUR.value, Math.min(pos.slot, END_HOUR.value - duration))
    dc.drop = {
      date: pos.date,
      start,
      left: pos.rect.left,
      width: pos.rect.width,
      top: pos.rect.top - (pos.slot - start) * 60,
      height: duration * 60
    }
  }

  const onEventMouseDown = (e, event) => {
    if (e.ctrlKey || e.metaKey) {
      // Ctrl+拖拽 = 复制到新位置；位移<4px 视为普通点击
      e.preventDefault()
      e.stopPropagation()
      const src = events.value.find(ev => ev.id === event.id) || event
      dragCopy.value = {
        src,
        startX: e.clientX,
        startY: e.clientY,
        started: false,
        ghost: { x: e.clientX, y: e.clientY },
        drop: null
      }
    } else {
      openEventDialog(event)
    }
  }

  const onDragCopyMove = (e) => {
    const dc = dragCopy.value
    if (!dc) return
    dc.ghost = { x: e.clientX, y: e.clientY }
    if (!dc.started && Math.hypot(e.clientX - dc.startX, e.clientY - dc.startY) > 4) {
      dc.started = true
      document.body.style.cursor = 'copy'
      document.body.style.userSelect = 'none'
      document.body.classList.add('dragging-copy')
    }
    if (dc.started) computeDrop(e, dc)
  }

  const onDragCopyEnd = (e) => {
    const dc = dragCopy.value
    if (!dc) return
    document.body.style.cursor = ''
    document.body.style.userSelect = ''
    document.body.classList.remove('dragging-copy')
    if (!dc.started) { // 视为普通点击，不产生副本
      dragCopy.value = null
      return
    }

    computeDrop(e, dc)
    const drop = dc.drop
    dragCopy.value = null
    if (!drop) return // 未落在时间格子上，取消

    const newEvent = placeEventCopy(dc.src, drop.date, drop.start, { min: START_HOUR.value, max: END_HOUR.value })
    const timeText = newEvent.isAllDay ? '全天' : `${formatTime(newEvent.startTime)}-${formatTime(newEvent.endTime)}`
    notify('已复制事件', `「${newEvent.title || '未命名'}」→ ${newEvent.date} ${timeText}`)
  }

  onMounted(() => {
    document.addEventListener('mousemove', onDragCopyMove)
    document.addEventListener('mouseup', onDragCopyEnd)
  })
  onUnmounted(() => {
    document.removeEventListener('mousemove', onDragCopyMove)
    document.removeEventListener('mouseup', onDragCopyEnd)
  })

  return { dragCopy, onEventMouseDown, onDragCopyMove, onDragCopyEnd }
}

// 拖拽交互组合式函数：创建事件(拖选格子/点击全天区) + 调整开始/结束时间 + 全天日期调整
import { ref, onMounted, onUnmounted } from 'vue'
import { events } from '../store/events'

export function useEventDrag({ START_HOUR, END_HOUR, weekDays, openCreateDialog }) {
  // ---------- 拖拽创建 ----------
  const isDragging = ref(false)
  const dragStartTime = ref(null)
  const dragDay = ref(null)

  const startDrag = (e, day, slot) => {
    if (e.button !== 0) return // 右键留给菜单
    isDragging.value = true
    dragDay.value = day
    dragStartTime.value = slot.value
  }

  const onDrag = (e) => {
    if (!isDragging.value) return
    e.preventDefault()
  }

  const endDrag = (e, day, slot) => {
    if (!isDragging.value) return
    isDragging.value = false

    const endValue = slot.value
    // 使用拖拽起始的日期，确保事件在正确的一天
    if (dragDay.value && dragStartTime.value !== null) {
      const eventDate = formatDateLocal(dragDay.value)
      const startTime = Math.min(dragStartTime.value, endValue)
      let endTime = Math.max(dragStartTime.value, endValue)

      // 至少需要30分钟的时长
      if (endTime - startTime < 0.5) {
        endTime = startTime + 0.5
      }

      openCreateDialog({
        date: eventDate,
        startTime,
        endTime,
        title: '',
        type: 'task',
        isAllDay: false
      })
    }

    dragStartTime.value = null
    dragDay.value = null
  }

  // 点击全天区域创建
  const startDragAllDay = (e, day) => {
    const eventDate = formatDateLocal(day)
    openCreateDialog({
      date: eventDate,
      endDate: eventDate,
      startTime: START_HOUR.value,
      endTime: END_HOUR.value,
      title: '',
      type: 'allDay',
      isAllDay: true
    })
  }

  // ---------- 拖拽调整开始/结束时间 ----------
  const resizing = ref(null) // { eventId, mode: 'start'|'end', startY, origStart, origEnd }

  const startResize = (e, event, mode) => {
    e.stopPropagation()
    e.preventDefault()
    resizing.value = {
      eventId: event.id,
      mode,
      startY: e.clientY,
      origStart: event.startTime,
      origEnd: event.endTime
    }
    document.body.style.cursor = 'ns-resize'
    document.body.style.userSelect = 'none'
  }

  const onResizeMove = (e) => {
    const r = resizing.value
    if (!r) return
    const deltaSlots = Math.round((e.clientY - r.startY) / 30) // 每30px = 0.5小时
    const target = events.value.find(ev => ev.id === r.eventId)
    if (!target) return

    if (r.mode === 'start') {
      let newStart = r.origStart + deltaSlots * 0.5
      newStart = Math.max(START_HOUR.value, Math.min(newStart, r.origEnd - 0.5))
      if (newStart !== target.startTime) target.startTime = newStart
    } else {
      let newEnd = r.origEnd + deltaSlots * 0.5
      newEnd = Math.max(r.origStart + 0.5, Math.min(newEnd, END_HOUR.value))
      if (newEnd !== target.endTime) target.endTime = newEnd
    }
  }

  const endResize = () => {
    if (!resizing.value) return
    resizing.value = null
    document.body.style.cursor = ''
    document.body.style.userSelect = ''
  }

  // ---------- 全天事件拖拽调整开始/结束日期 ----------
  const alldayResize = ref(null) // { eventId, mode: 'start'|'end', gridRect }

  const startAllDayResize = (e, event, mode) => {
    e.stopPropagation()
    e.preventDefault()
    const grid = e.currentTarget.closest('.all-day-events-grid')
    if (!grid) return
    alldayResize.value = {
      eventId: event.id,
      mode,
      gridRect: grid.getBoundingClientRect()
    }
    document.body.style.cursor = 'ew-resize'
    document.body.style.userSelect = 'none'
  }

  const onAllDayResizeMove = (e) => {
    const ar = alldayResize.value
    if (!ar) return
    const colWidth = ar.gridRect.width / 7
    // 限制在当前可见周（周一~周日，0~6）
    const dayIndex = Math.max(0, Math.min(6, Math.floor((e.clientX - ar.gridRect.left) / colWidth)))
    const target = events.value.find(ev => ev.id === ar.eventId)
    if (!target) return
    const dayStr = formatDateLocal(weekDays.value[dayIndex])
    if (ar.mode === 'start') {
      // 开始日期不能晚于结束日期（同日 = 单日事件）
      if (dayStr <= (target.endDate || target.date)) target.date = dayStr
    } else {
      if (dayStr >= target.date) target.endDate = dayStr
    }
  }

  const endAllDayResize = () => {
    if (!alldayResize.value) return
    alldayResize.value = null
    document.body.style.cursor = ''
    document.body.style.userSelect = ''
  }

  onMounted(() => {
    document.addEventListener('mousemove', onResizeMove)
    document.addEventListener('mouseup', endResize)
    document.addEventListener('mousemove', onAllDayResizeMove)
    document.addEventListener('mouseup', endAllDayResize)
  })
  onUnmounted(() => {
    document.removeEventListener('mousemove', onResizeMove)
    document.removeEventListener('mouseup', endResize)
    document.removeEventListener('mousemove', onAllDayResizeMove)
    document.removeEventListener('mouseup', endAllDayResize)
  })

  return {
    isDragging,
    startDrag,
    onDrag,
    endDrag,
    startDragAllDay,
    resizing,
    startResize,
    onResizeMove,
    endResize,
    alldayResize,
    startAllDayResize,
    onAllDayResizeMove,
    endAllDayResize
  }
}

// 时区安全的日期格式化(YYYY-MM-DD, 本地时区)
const formatDateLocal = (date) => {
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}

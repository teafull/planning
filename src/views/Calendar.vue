<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { ElNotification, ElSwitch, ElButton, ElTooltip } from 'element-plus'
import { sendNotification, requestPermission, isPermissionGranted } from '@tauri-apps/plugin-notification'
import { events, loadEvents } from '../store/events'
import { clipboard } from '../store/clipboard'
import { invoke } from '@tauri-apps/api/core'
import { notifications, preferences } from '../store/settings'
import { getEventType, formatTime, getEventDetailHtml } from '../utils/eventTypes'
import { useEventDrag } from '../composables/useEventDrag'
import { useCopyDrag } from '../composables/useCopyDrag'
import { useContextMenu } from '../composables/useContextMenu'
import EventBlock from '../components/calendar/EventBlock.vue'
import AllDayBar from '../components/calendar/AllDayBar.vue'
import EventDialog from '../components/calendar/EventDialog.vue'

// ---------- 节假日 ----------
const holidayData = ref({})

const loadHolidayData = () => {
  const raw = localStorage.getItem('holiday-data')
  if (raw) {
    try {
      holidayData.value = JSON.parse(raw)
    } catch {}
  }
}

const saveHolidayData = () => {
  localStorage.setItem('holiday-data', JSON.stringify(holidayData.value))
}

const isHoliday = (date) => {
  if (!holidayData.value || typeof holidayData.value !== 'object') return false
  const dateKey = formatDateKey(date)
  const info = holidayData.value[dateKey]
  return info && info.holiday === true
}

const getHolidayName = (date) => {
  if (!holidayData.value || typeof holidayData.value !== 'object') return ''
  const dateKey = formatDateKey(date)
  return holidayData.value[dateKey]?.name || ''
}

const isWorkday = (date) => {
  if (!holidayData.value || typeof holidayData.value !== 'object') return false
  const dateKey = formatDateKey(date)
  const info = holidayData.value[dateKey]
  return info && info.holiday === false && info.wage === 1
}

const getWorkdayName = (date) => {
  if (!holidayData.value || typeof holidayData.value !== 'object') return ''
  const dateKey = formatDateKey(date)
  const info = holidayData.value[dateKey]
  return info && info.holiday === false ? info.name : ''
}

const formatDateKey = (date) => {
  const m = String(date.getMonth() + 1).padStart(2, '0')
  const d = String(date.getDate()).padStart(2, '0')
  return `${m}-${d}`
}

// 已有节假日数据则不重复请求
async function getHolidayData() {
  const raw = localStorage.getItem('holiday-data')
  if (raw) return
  const res = await invoke('http_get', { url: preferences.value.holidayApiUrl })
  holidayData.value = res.holiday
  saveHolidayData()
}

// ---------- 周视图基础 ----------
const currentWeekStart = ref(new Date())

const weekHeaderRef = ref(null)
const calendarBodyRef = ref(null)

const onCalendarScroll = () => {
  if (weekHeaderRef.value && calendarBodyRef.value) {
    weekHeaderRef.value.style.transform = `translateX(-${calendarBodyRef.value.scrollLeft}px)`
  }
}

const START_HOUR = computed(() => parseInt(preferences.value.dayStartTime?.split(':')[0] || 8))
const END_HOUR = computed(() => parseInt(preferences.value.dayEndTime?.split(':')[0] || 23))

// 当前周的日期（周一到周日）
const weekDays = computed(() => {
  const days = []
  const start = new Date(currentWeekStart.value)
  const dayOfWeek = start.getDay()
  const diff = start.getDate() - dayOfWeek + (dayOfWeek === 0 ? -6 : 1)
  const monday = new Date(start.getFullYear(), start.getMonth(), diff)
  for (let i = 0; i < 7; i++) {
    days.push(new Date(monday.getFullYear(), monday.getMonth(), monday.getDate() + i))
  }
  return days
})

// 时间片（每半小时一个单位）
const timeSlots = computed(() => {
  const slots = []
  for (let hour = START_HOUR.value; hour < END_HOUR.value; hour++) {
    for (let half = 0; half < 2; half++) {
      slots.push({ hour, half, value: hour + half * 0.5 })
    }
  }
  slots.push({ hour: END_HOUR.value, half: 0, value: END_HOUR.value })
  return slots
})

// 时间轴标签（只显示整点）
const timeLabels = computed(() => {
  const labels = []
  for (let hour = START_HOUR.value; hour <= END_HOUR.value; hour++) {
    labels.push(hour)
  }
  return labels
})

const formatDate = (date) => `${date.getMonth() + 1}/${date.getDate()}`

// 时区安全的日期格式化（YYYY-MM-DD, 本地时区）
const formatDateLocal = (date) => {
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}

const getEventsByDay = (day) => {
  const dayStr = formatDateLocal(day)
  return events.value.filter(event => {
    if (!event.isAllDay) {
      return event.date === dayStr
    }
    const eventStart = new Date(event.date)
    const eventEnd = new Date(event.endDate || event.date)
    const targetDay = new Date(dayStr)
    return targetDay >= eventStart && targetDay <= eventEnd
  })
}

const getRegularEvents = (day) => getEventsByDay(day).filter(event => !event.isAllDay)

// 计算事件块位置与高度（支持并行分列）
const getDayEventsForRender = (day) => {
  const dayEvents = getRegularEvents(day)
  if (dayEvents.length === 0) return []

  const eventsWithSlots = dayEvents.map(event => {
    const startSlotIndex = timeSlots.value.findIndex(slot => Math.abs(slot.value - event.startTime) < 0.001)
    const endSlotIndex = timeSlots.value.findIndex(slot => Math.abs(slot.value - event.endTime) < 0.001)
    return { event, startSlotIndex, endSlotIndex }
  }).filter(e => e.startSlotIndex !== -1 && e.endSlotIndex !== -1)

  eventsWithSlots.sort((a, b) => a.startSlotIndex - b.startSlotIndex)

  const columns = []
  const maxColumnsPerSlot = new Map()

  eventsWithSlots.forEach(({ event, startSlotIndex, endSlotIndex }) => {
    let columnIndex = 0
    let foundColumn = false
    while (!foundColumn) {
      let canUseColumn = true
      for (const colEvent of columns) {
        if (colEvent.columnIndex === columnIndex) {
          const isOverlapping = !(endSlotIndex <= colEvent.startSlotIndex || startSlotIndex >= colEvent.endSlotIndex)
          if (isOverlapping) {
            canUseColumn = false
            break
          }
        }
      }
      if (canUseColumn) {
        foundColumn = true
      } else {
        columnIndex++
      }
    }

    columns.push({ eventId: event.id, startSlotIndex, endSlotIndex, columnIndex })

    for (let i = startSlotIndex; i < endSlotIndex; i++) {
      const currentMax = maxColumnsPerSlot.get(i) || 0
      maxColumnsPerSlot.set(i, Math.max(currentMax, columnIndex + 1))
    }
  })

  return eventsWithSlots.map(({ event, startSlotIndex, endSlotIndex }) => {
    const columnData = columns.find(c => c.eventId === event.id)
    if (!columnData) return { ...event, style: {} }

    const top = startSlotIndex * 30
    const height = (endSlotIndex - startSlotIndex) * 30

    let maxColsInTimeRange = 1
    for (let i = startSlotIndex; i < endSlotIndex; i++) {
      maxColsInTimeRange = Math.max(maxColsInTimeRange, maxColumnsPerSlot.get(i) || 1)
    }

    const padding = 4
    const availableWidth = 100 - padding * 2
    const columnWidth = availableWidth / maxColsInTimeRange
    const left = padding + columnData.columnIndex * columnWidth
    const right = 100 - (left + columnWidth)

    return {
      ...event,
      style: {
        position: 'absolute',
        top: `${top}px`,
        height: `${height}px`,
        left: `${left}%`,
        width: `${columnWidth}%`,
        padding: '0 2px'
      }
    }
  })
}

// 本周事件列表（右侧栏）：与本周相交的事件各计一次（跨天全天事件不重复）
const weekEvents = computed(() => {
  const weekDateStrings = weekDays.value.map(day => formatDateLocal(day))
  return events.value.filter(event => {
    if (event.isAllDay) {
      const eventStart = new Date(event.date)
      const eventEnd = new Date(event.endDate || event.date)
      const weekStart = new Date(weekDateStrings[0])
      const weekEnd = new Date(weekDateStrings[6])
      return eventEnd >= weekStart && eventStart <= weekEnd
    }
    return weekDateStrings.includes(event.date)
  }).sort((a, b) => {
    if (a.date !== b.date) return a.date.localeCompare(b.date)
    if (a.isAllDay && !b.isAllDay) return -1
    if (!a.isAllDay && b.isAllDay) return 1
    return (a.startTime || 0) - (b.startTime || 0)
  })
})

// 本周统计
const weekStatistics = computed(() => {
  const totalCount = weekEvents.value.length
  const allDayCount = weekEvents.value.filter(e => e.isAllDay).length
  const multiDayCount = weekEvents.value.filter(
    e => e.isAllDay && e.endDate && e.endDate !== e.date
  ).length
  return { totalCount, allDayCount, multiDayCount }
})

// ---------- 周导航 ----------
const prevWeek = () => {
  const newDate = new Date(currentWeekStart.value)
  newDate.setDate(newDate.getDate() - 7)
  currentWeekStart.value = newDate
}

const nextWeek = () => {
  const newDate = new Date(currentWeekStart.value)
  newDate.setDate(newDate.getDate() + 7)
  currentWeekStart.value = newDate
}

const goToToday = () => { currentWeekStart.value = new Date() }

const isToday = (day) => formatDateLocal(day) === formatDateLocal(new Date())

const getWeekNumber = (date) => {
  const d = new Date(date.getFullYear(), date.getMonth(), date.getDate())
  d.setDate(d.getDate() + 3 - ((d.getDay() + 6) % 7))
  const week1 = new Date(d.getFullYear(), 0, 4)
  return 1 + Math.round(((d.getTime() - week1.getTime()) / 86400000 - 3 + ((week1.getDay() + 6) % 7)) / 7)
}

const getTotalWeeksInYear = (year) => {
  const dec28 = new Date(year, 11, 28)
  const week = getWeekNumber(dec28)
  return week === 53 ? 53 : 52
}

const remainingWeeks = computed(() => {
  const now = new Date()
  return getTotalWeeksInYear(now.getFullYear()) - getWeekNumber(now)
})

// ---------- 事件弹窗 ----------
const showDialog = ref(false)
const currentEvent = ref(null)
const form = ref({
  title: '',
  date: '',
  endDate: '',
  startTime: 9,
  endTime: 10,
  description: '',
  type: 'task',
  isAllDay: false
})

const notify = (title, message) => {
  ElNotification({ title, message, type: 'success', duration: 2000 })
}

const openEventDialog = (event) => {
  currentEvent.value = event
  form.value = {
    title: event.title,
    date: event.isAllDay ? [event.date, event.endDate || event.date] : event.date,
    endDate: event.endDate || event.date,
    startTime: event.startTime || 9,
    endTime: event.endTime || 10,
    description: event.description || '',
    type: event.type || 'task',
    isAllDay: event.isAllDay || false
  }
  showDialog.value = true
}

// 创建/复制入口：填充临时事件与表单后打开弹窗
const openCreateDialog = (p) => {
  currentEvent.value = {
    id: p.id ?? Date.now(),
    date: p.date,
    endDate: p.endDate || p.date,
    startTime: p.startTime,
    endTime: p.endTime,
    title: p.title ?? '',
    type: p.type ?? 'task',
    isAllDay: !!p.isAllDay
  }
  form.value = {
    title: p.title ?? '',
    date: p.isAllDay ? [p.date, p.endDate || p.date] : p.date,
    endDate: p.endDate || p.date,
    startTime: p.startTime,
    endTime: p.endTime,
    description: p.description ?? '',
    type: p.type ?? 'task',
    isAllDay: !!p.isAllDay
  }
  showDialog.value = true
}

const saveEvent = () => {
  if (currentEvent.value) {
    const event = events.value.find(e => e.id === currentEvent.value.id)
    if (event) {
      // 已存在的事件，更新它
      event.title = form.value.title
      event.type = form.value.type
      event.isAllDay = form.value.isAllDay
      event.description = form.value.description

      if (form.value.isAllDay) {
        if (Array.isArray(form.value.date)) {
          event.date = form.value.date[0]
          event.endDate = form.value.date[1] || form.value.date[0]
        } else {
          event.date = form.value.date
          event.endDate = form.value.endDate || form.value.date
        }
      } else {
        event.date = form.value.date
        event.startTime = form.value.startTime
        event.endTime = form.value.endTime
      }
    } else {
      // 新事件，添加到事件列表
      const newEvent = { ...currentEvent.value }
      newEvent.title = form.value.title
      newEvent.type = form.value.type
      newEvent.isAllDay = form.value.isAllDay
      newEvent.description = form.value.description

      if (form.value.isAllDay) {
        if (Array.isArray(form.value.date)) {
          newEvent.date = form.value.date[0]
          newEvent.endDate = form.value.date[1] || form.value.date[0]
        } else {
          newEvent.date = form.value.date
          newEvent.endDate = form.value.endDate || form.value.date
        }
      } else {
        newEvent.date = form.value.date
        newEvent.startTime = form.value.startTime
        newEvent.endTime = form.value.endTime
      }
      events.value.push(newEvent)
    }
  }
  showDialog.value = false
}

// 另存为副本：以当前表单内容保存为新事件，不影响原事件
const saveEventAsCopy = () => {
  if (!currentEvent.value) return
  const newEvent = {
    ...currentEvent.value,
    id: Date.now(),
    title: form.value.title,
    type: form.value.type,
    isAllDay: form.value.isAllDay,
    description: form.value.description
  }
  if (form.value.isAllDay) {
    if (Array.isArray(form.value.date)) {
      newEvent.date = form.value.date[0]
      newEvent.endDate = form.value.date[1] || form.value.date[0]
    } else {
      newEvent.date = form.value.date
      newEvent.endDate = form.value.endDate || form.value.date
    }
  } else {
    newEvent.date = form.value.date
    newEvent.startTime = form.value.startTime
    newEvent.endTime = form.value.endTime
  }
  events.value.push(newEvent)
  notify('已另存为副本', `「${newEvent.title || '未命名'}」${newEvent.date}`)
  showDialog.value = false
}

// 删除事件（弹窗内）
const deleteEvent = () => {
  if (currentEvent.value) {
    events.value = events.value.filter(e => e.id !== currentEvent.value.id)
  }
  showDialog.value = false
}

// 列表中直接删除事件
const deleteEventItem = (event) => {
  events.value = events.value.filter(e => e.id !== event.id)
}

// ---------- 组合式交互 ----------
const { isDragging, startDrag, onDrag, endDrag, startDragAllDay, startResize, startAllDayResize } =
  useEventDrag({ START_HOUR, END_HOUR, weekDays, openCreateDialog })

const { dragCopy, onEventMouseDown } =
  useCopyDrag({ START_HOUR, END_HOUR, openEventDialog, notify })

const {
  ctxMenu, onEventContextMenu, onSlotContextMenu,
  doCopy, doCopyNextWeek, doSaveAsCopy, doCtxEdit, doCtxDelete, doPaste, doCreateAt
} = useContextMenu({ START_HOUR, END_HOUR, formatDateLocal, notify, openEventDialog, openCreateDialog, deleteEventItem })

// ---------- 全天事件布局 ----------
// 本周相关全天事件 → 周内 startIndex/endIndex/span
const allDayEventsLayout = computed(() => {
  const weekDateStrings = weekDays.value.map(day => formatDateLocal(day))
  const relevantEvents = events.value.filter(event => {
    if (!event.isAllDay) return false
    const eventStart = new Date(event.date)
    const eventEnd = new Date(event.endDate || event.date)
    const weekStart = new Date(weekDateStrings[0])
    const weekEnd = new Date(weekDateStrings[6])
    return eventEnd >= weekStart && eventStart <= weekEnd
  })

  return relevantEvents.map(event => {
    const eventStartStr = event.date
    const eventEndStr = event.endDate || event.date
    const startIndex = weekDateStrings.findIndex(date => date >= eventStartStr)
    const endIndex = weekDateStrings.findIndex(date => date > eventEndStr)
    const actualStartIndex = Math.max(0, startIndex)
    const actualEndIndex = endIndex === -1 ? 7 : endIndex
    return { ...event, startIndex: actualStartIndex, endIndex: actualEndIndex, span: actualEndIndex - actualStartIndex }
  }).sort((a, b) => {
    if (a.startIndex !== b.startIndex) return a.startIndex - b.startIndex
    return b.span - a.span
  })
})

// 为全天事件分配行（重叠事件不同行）
const allDayEventsWithLayout = computed(() => {
  const list = [...allDayEventsLayout.value]
  const rows = []
  list.forEach(event => {
    let placed = false
    for (let i = 0; i < rows.length; i++) {
      const hasOverlap = rows[i].some(e =>
        !(event.startIndex >= e.endIndex || event.endIndex <= e.startIndex)
      )
      if (!hasOverlap) {
        rows[i].push(event)
        event.row = i
        placed = true
        break
      }
    }
    if (!placed) {
      rows.push([event])
      event.row = rows.length - 1
    }
  })
  return list
})

const getRowEvents = (rowIndex) => allDayEventsWithLayout.value.filter(e => e.row === rowIndex)
const allDayEventsRowCount = computed(() => {
  const maxRow = allDayEventsWithLayout.value.reduce((max, e) => Math.max(max, e.row), 0)
  return maxRow + 1
})
const allDayRows = computed(() =>
  Array.from({ length: allDayEventsRowCount.value }, (_, i) => getRowEvents(i))
)

// ---------- 提醒 ----------
const reminderEnabled = computed(() => notifications.value.desktop)
let reminderInterval = null
const notifiedEvents = ref(new Set())

const sendEventReminder = async (event) => {
  try {
    const hasPermission = await isPermissionGranted()
    if (!hasPermission) {
      await requestPermission()
    }
    await sendNotification({
      title: '日程提醒',
      body: `事件标题:${event.title} (${getEventType(event.type).label}) \n事件内容:${event.description}`,
      icon: null
    })
    console.log('提醒已发送:', event.title)
  } catch (error) {
    console.error('发送提醒失败:', error)
    ElNotification({
      title: '日程提醒',
      message: event.title,
      type: 'info',
      duration: 5000
    })
  }
}

const testReminder = async () => {
  await sendEventReminder({ title: '测试提醒', type: 'reminder', description: '这是一个测试提醒' })
}

const checkReminders = () => {
  if (!reminderEnabled.value) return
  const now = new Date()
  const todayStr = formatDateLocal(now)
  const currentTime = now.getHours() + now.getMinutes() / 60

  events.value.forEach(event => {
    if (event.date === todayStr && !event.isAllDay) {
      const reminderTime = event.startTime - 5 / 60
      const eventKey = `${event.id}-${todayStr}`
      if (!notifiedEvents.value.has(eventKey) && currentTime >= reminderTime && currentTime < event.startTime) {
        sendEventReminder(event)
        notifiedEvents.value.add(eventKey)
      }
    }
  })
}

const startReminderCheck = () => {
  reminderInterval = setInterval(checkReminders, 60 * 1000)
}

const stopReminderCheck = () => {
  if (reminderInterval) {
    clearInterval(reminderInterval)
    reminderInterval = null
  }
}

// ---------- 生命周期 ----------
onMounted(() => {
  getHolidayData()
  loadEvents()
  loadHolidayData()
  startReminderCheck()
})

onUnmounted(() => {
  stopReminderCheck()
})
</script>

<template>
  <div class="calendar-app">
    <header class="calendar-header">
      <h2 class="current-month">
        {{ weekDays[0]?.getFullYear() }}年{{ weekDays[0]?.getMonth() + 1 }}月{{ weekDays[0]?.getDate() }}日 - {{ weekDays[6]?.getMonth() + 1 }}月{{ weekDays[6]?.getDate() }}日（第{{ getWeekNumber(weekDays[0]) }}周，本年剩余 {{ remainingWeeks }} 周）
      </h2>
      <div class="header-controls">
        <div class="reminder-control">
          <span class="reminder-label">提醒</span>
          <el-switch v-model="notifications.desktop" />
          <el-button size="small" @click="testReminder" type="primary" plain>测试</el-button>
        </div>
        <div class="statistics-info">
          <span class="stat-item">总事件: {{ weekStatistics.totalCount }}</span>
          <span class="stat-item">全天: {{ weekStatistics.allDayCount }}</span>
          <span class="stat-item">跨天: {{ weekStatistics.multiDayCount }}</span>
        </div>
        <el-button @click="prevWeek">上一周</el-button>
        <el-button @click="goToToday">本周</el-button>
        <el-button @click="nextWeek">下一周</el-button>
      </div>

    </header>

    <div class="calendar-container">
      <!-- 星期标题 -->
      <div class="week-header" ref="weekHeaderRef">
        <div class="time-header"></div>
        <div v-for="day in weekDays" :key="day.getTime()" class="day-header" :class="{ 'today': isToday(day), 'weekend': day.getDay() === 0 || day.getDay() === 6 }">
          <div class="day-name" :class="{ 'is-holiday': isHoliday(day), 'is-workday': isWorkday(day) }">
            {{ ['周一', '周二', '周三', '周四', '周五', '周六', '周日'][day.getDay() === 0 ? 6 : day.getDay() - 1] }}
            <span v-if="isHoliday(day)" class="holiday-tag">{{ getHolidayName(day) }}</span>
            <span v-else-if="isWorkday(day)" class="workday-tag">{{ getWorkdayName(day) }}</span>
          </div>
          <div class="day-date">{{ formatDate(day) }}</div>
          <div v-if="(day.getDay() === 0 || day.getDay() === 6) && !isHoliday(day) && !isWorkday(day)" class="rest-tag">休息</div>
        </div>
        <div class="event-list-header">本周事件</div>
      </div>

      <!-- 全天事件区域 -->
      <AllDayBar
        :days="weekDays"
        :rows="allDayRows"
        :row-count="allDayEventsRowCount"
        :is-today="isToday"
        :is-workday="isWorkday"
        @create="startDragAllDay"
        @open="openEventDialog"
        @resize="startAllDayResize"
      />

      <!-- 日历主体 - 包含滚动容器 -->
      <div class="calendar-scroll-container">
        <!-- 日历网格区域 -->
        <div class="calendar-grid-wrapper">
          <div class="calendar-body" ref="calendarBodyRef" @scroll="onCalendarScroll">
          <!-- 时间轴 -->
          <div class="time-column">
            <div v-for="(hour, index) in timeLabels" :key="hour" class="time-label" :class="{ 'last-label': index === timeLabels.length - 1 }">
              {{ formatTime(hour) }}
            </div>
          </div>

          <!-- 每日网格 -->
          <div v-for="day in weekDays" :key="day.getTime()" class="day-column">
            <div
              v-for="slot in timeSlots"
              :key="`${slot.hour}-${slot.half}`"
              class="time-slot"
              :class="{ 'half-hour': slot.half === 1, 'weekend': day.getDay() === 0 || day.getDay() === 6, 'is-workday': isWorkday(day) }"
              :data-date="formatDateLocal(day)"
              :data-value="slot.value"
              @mousedown="startDrag($event, day, slot)"
              @mousemove="onDrag($event)"
              @mouseup="endDrag($event, day, slot)"
              @mouseleave="isDragging = false"
              @contextmenu.prevent="onSlotContextMenu($event, day, slot)"
            ></div>
            <!-- 渲染事件（使用绝对定位） -->
            <EventBlock
              v-for="ev in getDayEventsForRender(day)"
              :key="ev.id"
              :event="ev"
              @mousedown="onEventMouseDown"
              @contextmenu="onEventContextMenu"
              @resize="startResize"
            />
          </div>
          </div>
        </div>

        <!-- 本周事件列表 -->
        <div class="event-list">
          <div class="event-list-content">
            <div v-if="weekEvents.length === 0" class="no-events">暂无事件</div>
            <div v-else class="events-container">
              <el-tooltip
                v-for="event in weekEvents"
                :key="event.id"
                :content="getEventDetailHtml(event)"
                raw-content
                placement="right"
                :show-after="200"
              >
                <div
                  class="event-list-item"
                  :style="{ borderLeftColor: getEventType(event.type).color }"
                  @click="openEventDialog(event)"
                >
                  <button class="event-delete-btn" @click.stop="deleteEventItem(event)" title="删除事件">×</button>
                  <div class="event-item-header">
                    <span class="event-item-type">{{ getEventType(event.type).label }}</span>
                    <span class="event-item-date">{{ event.date }}</span>
                    <span class="event-item-time">{{ event.isAllDay ? '全天' : `${formatTime(event.startTime)}-${formatTime(event.endTime)}` }}</span>
                  </div>
                  <div class="event-item-title">{{ event.title }}</div>
                  <div v-if="event.description" class="event-item-description">{{ event.description }}</div>
                </div>
              </el-tooltip>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- 事件编辑弹窗 -->
    <EventDialog
      v-model:show="showDialog"
      :form="form"
      @save="saveEvent"
      @save-copy="saveEventAsCopy"
      @delete="deleteEvent"
    />

    <!-- 右键菜单 -->
    <Teleport to="body">
      <div
        v-if="ctxMenu"
        class="ctx-menu"
        :style="{ left: ctxMenu.x + 'px', top: ctxMenu.y + 'px' }"
      >
        <template v-if="ctxMenu.kind === 'event'">
          <div class="ctx-item" @click="doCopy">复制</div>
          <div class="ctx-item" @click="doCopyNextWeek">复制到下周同一天</div>
          <div class="ctx-item" @click="doSaveAsCopy">另存为副本…</div>
          <div class="ctx-sep"></div>
          <div class="ctx-item" @click="doCtxEdit">编辑</div>
          <div class="ctx-item ctx-danger" @click="doCtxDelete">删除</div>
        </template>
        <template v-else>
          <div class="ctx-item" :class="{ 'ctx-disabled': !clipboard }" @click="clipboard && doPaste()">
            粘贴<template v-if="clipboard">「{{ clipboard.event.title || '未命名' }}」</template>
          </div>
          <div class="ctx-item" @click="doCreateAt">新建事件</div>
        </template>
      </div>
      <!-- Ctrl拖拽复制：目标位置预览块 + 跟随角标 -->
      <div
        v-if="dragCopy && dragCopy.started && dragCopy.drop"
        class="drag-copy-preview"
        :style="{
          left: dragCopy.drop.left + 'px',
          top: dragCopy.drop.top + 'px',
          width: dragCopy.drop.width + 'px',
          height: dragCopy.drop.height + 'px',
          background: getEventType(dragCopy.src.type).bgColor
        }"
      >
        <span class="drag-copy-title">{{ dragCopy.src.title || '未命名' }}</span>
      </div>
      <div
        v-if="dragCopy && dragCopy.started"
        class="drag-copy-ghost"
        :style="{ left: (dragCopy.ghost.x + 14) + 'px', top: (dragCopy.ghost.y + 14) + 'px' }"
      >+ 复制</div>
    </Teleport>
  </div>
</template>

<style scoped>
* {
  box-sizing: border-box;
}

.calendar-app {
  width: 100%;
  height: 100%;
  min-width: 0;
  display: flex;
  flex-direction: column;
  background: #f8fafc;
  overflow: hidden;
}


.calendar-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 20px 30px;
  background: white;
  border-bottom: 1px solid #e2e8f0;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.05);
}

.header-controls {
  display: flex;
  gap: 10px;
  align-items: center;
}

.reminder-control {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-right: 10px;
  padding-right: 10px;
  border-right: 1px solid #e0e0e0;
}

.reminder-label {
  font-size: 14px;
  color: #666;
}

.statistics-info {
  display: flex;
  align-items: center;
  gap: 15px;
  margin-right: 15px;
  padding-right: 15px;
  border-right: 1px solid #e0e0e0;
}

.stat-item {
  font-size: 13px;
  color: #666;
  white-space: nowrap;
}

.current-month {
  font-size: 24px;
  font-weight: 600;
  color: #333;
  margin: 0;
}

.calendar-container {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  margin: 0;
  background: white;
  border-radius: 0;
  box-shadow: none;
  box-sizing: border-box;
  min-width: 0;
}


.week-header {
  display: grid;
  grid-template-columns: 60px repeat(7, 1fr) 300px;
  border-bottom: 1px solid #e0e0e0;
  background: #fafafa;
  box-sizing: border-box;
  width: 100%;
  flex-shrink: 0;
  height: 28px;
}

.calendar-scroll-container {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  min-width: 0;
}


.time-header {
  border-right: 1px solid #e0e0e0;
  box-sizing: border-box;
  height: 28px;
}

.day-header {
  padding: 0 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  text-align: center;
  border-right: 1px solid #e0e0e0;
  box-sizing: border-box;
  height: 28px;
}

.day-header:last-child {
  border-right: none;
}

.day-name {
  font-size: 13px;
  color: #666;
}

.day-name.is-holiday {
  color: #ef4444;
  font-weight: 600;
}

.holiday-tag {
  display: inline-block;
  margin-left: 6px;
  padding: 2px 6px;
  background: #fef2f2;
  border: 1px solid #fca5a5;
  border-radius: 999px;
  font-size: 11px;
  color: #dc2626;
  font-weight: 600;
}

.day-name.is-workday {
  color: #10b981;
  font-weight: 600;
}

.workday-tag {
  display: inline-block;
  margin-left: 6px;
  padding: 2px 6px;
  background: #ecfdf5;
  border: 1px solid #6ee7b7;
  border-radius: 999px;
  font-size: 11px;
  color: #059669;
  font-weight: 600;
}

.day-date {
  font-size: 14px;
  font-weight: 600;
  color: #333;
}

.day-header.today {
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
}

.day-header.today .day-name,
.day-header.today .day-date {
  color: white;
}

.day-header.weekend {
  background: #fff5f5;
}

.day-header.weekend .day-name {
  color: #e53e3e;
  font-weight: 500;
}

.rest-tag {
  font-size: 10px;
  color: #e53e3e;
  background: #fed7d7;
  padding: 1px 6px;
  border-radius: 10px;
  display: inline-block;
  white-space: nowrap;
}

.calendar-scroll-container {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: row;
  min-width: 0;
}


.calendar-grid-wrapper {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  min-width: 0;
}


.event-list {
  width: 300px;
  display: flex;
  flex-direction: column;
  border-left: 1px solid #e0e0e0;
  flex-shrink: 0;
  background: #fafafa;
  box-sizing: border-box;
  min-width: 0;
}


.calendar-body {
  flex: 1;
  display: grid;
  grid-template-columns: 60px repeat(7, 1fr);
  overflow-y: scroll;
  overflow-x: hidden;
  position: relative;
  box-sizing: border-box;
  width: 100%;
  height: 100%;
  scrollbar-gutter: stable;
  min-width: 0;
}


/* 滚动条样式 - 仅在滚动时显示，覆盖式不挤压内容 */
.calendar-body::-webkit-scrollbar,
.event-list::-webkit-scrollbar,
.event-list-content::-webkit-scrollbar {
  width: 8px;
}

.calendar-body:-webkit-scrollbar-track,
.event-list:-webkit-scrollbar-track,
.event-list-content:-webkit-scrollbar-track {
  background: transparent;
}

.calendar-body::-webkit-scrollbar-thumb,
.event-list::-webkit-scrollbar-thumb,
.event-list-content::-webkit-scrollbar-thumb {
  background: transparent;
  border-radius: 3px;
}

.calendar-body:hover::-webkit-scrollbar-thumb,
.event-list:hover::-webkit-scrollbar-thumb,
.event-list-content:hover::-webkit-scrollbar-thumb {
  background: rgba(0, 0, 0, 0.2);
}

.calendar-body::-webkit-scrollbar-thumb:hover,
.event-list::-webkit-scrollbar-thumb:hover,
.event-list-content::-webkit-scrollbar-thumb:hover {
  background: rgba(0, 0, 0, 0.3);
}

/* Firefox 滚动条样式 */
.calendar-body scrollbar,
.event-list scrollbar,
.event-list-content scrollbar {
  width: thin;
}

.calendar-body scrollbar-thumb,
.event-list scrollbar-thumb,
.event-list-content scrollbar-thumb {
  background: transparent;
  border-radius: 3px;
}

.calendar-body:hover scrollbar-thumb,
.event-list:hover scrollbar-thumb,
.event-list-content:hover scrollbar-thumb {
  background: rgba(0, 0, 0, 0.2);
}

.calendar-body scrollbar-thumb:hover,
.event-list scrollbar-thumb:hover,
.event-list-content scrollbar-thumb:hover {
  background: rgba(0, 0, 0, 0.3);
}

.time-column {
  border-right: 1px solid #e0e0e0;
  display: flex;
  flex-direction: column;
}

.event-list-header {
  height: 28px;
  padding: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  text-align: center;
  font-size: 14px;
  font-weight: 600;
  color: #333;
  background: #fafafa;
  border-bottom: 1px solid #e0e0e0;
  border-left: 1px solid #e0e0e0;
  box-sizing: border-box;
}

.event-list-content {
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;
  padding: 10px;
  min-height: 0;
}

.no-events {
  text-align: center;
  color: #999;
  padding: 40px 20px;
  font-size: 14px;
}

.events-container {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.event-list-item {
  background: white;
  padding: 12px;
  border-radius: 8px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
  cursor: pointer;
  transition: transform 0.2s, box-shadow 0.2s;
  border-left: 4px solid #667eea;
  position: relative;
}

.event-list-item:hover {
  transform: translateX(2px);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
}

.event-delete-btn {
  position: absolute;
  top: 8px;
  right: 8px;
  width: 24px;
  height: 24px;
  border: none;
  background: #f56565;
  color: white;
  border-radius: 50%;
  font-size: 18px;
  line-height: 1;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  opacity: 0;
  transition: opacity 0.2s, background 0.2s;
}

.event-list-item:hover .event-delete-btn {
  opacity: 1;
}

.event-delete-btn:hover {
  background: #e53e3e;
}

.event-item-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
  font-size: 12px;
  color: #666;
  gap: 8px;
  flex-wrap: wrap;
}

.event-item-type {
  font-size: 10px;
  padding: 2px 8px;
  border-radius: 10px;
  background: #667eea;
  color: white;
  font-weight: 500;
}

.event-item-date {
  font-weight: 500;
}

.event-item-time {
  color: #667eea;
}

.event-item-title {
  font-size: 14px;
  font-weight: 600;
  color: #333;
  margin-bottom: 4px;
}

.event-item-description {
  font-size: 12px;
  color: #666;
  line-height: 1.5;
}

.time-label {
  height: 60px;
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding-top: 0;
  font-size: 12px;
  color: #999;
  border-bottom: 1px solid #f0f0f0;
  flex-shrink: 0;
  box-sizing: border-box;
}

.time-label:last-child {
  height: 30px;
  border-bottom: none;
  align-items: center;
}

.time-column {
  border-right: 1px solid #e0e0e0;
  box-sizing: border-box;
  overflow: hidden;
}

.day-column {
  border-right: 1px solid #e0e0e0;
  position: relative;
  box-sizing: border-box;
}

.day-column:last-child {
  border-right: none;
}

.time-slot {
  height: 30px;
  border-bottom: 1px solid #f5f5f5;
  cursor: pointer;
  transition: background 0.2s;
  position: relative;
  flex-shrink: 0;
  box-sizing: border-box;
}

.time-slot.half-hour {
  border-bottom-style: dashed;
  border-bottom-color: #f0f0f0;
}

.time-slot:hover {
  background: #f0f9ff;
}

.time-slot:first-child {
  border-top: 1px solid #e0e0e0;
}

.slot-content {
  width: 100%;
  height: 100%;
  padding: 2px 5px;
  overflow: hidden;
}

/* 事件分层内容（≥1小时：左上角类型，时间+标题一行垂直居中；短事件仅标题） */
  html.dark .calendar-app {
    background: #1a1a1a;
  }

  html.dark .calendar-header,
  html.dark .calendar-container {
    background: #2f2f2f;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
  }

  html.dark .week-header {
    background: #1f1f1f;
    border-color: #3a3a3a;
  }

  html.dark .day-header {
    border-color: #3a3a3a;
  }

  html.dark .day-name {
    color: #999;
  }

  html.dark .day-date {
    color: #e0e0e0;
  }

  html.dark .day-header.today {
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  }

  html.dark .day-header.weekend {
    background: #3d1f1f;
  }

  html.dark .day-header.weekend .day-name {
    color: #fc8181;
  }

  html.dark .rest-tag {
    color: #fc8181;
    background: #742a2a;
  }

  html.dark .current-month {
    color: #e0e0e0;
  }

  html.dark .time-column {
    border-color: #3a3a3a;
  }

  html.dark .calendar-grid-wrapper {
    border-color: #3a3a3a;
  }

  html.dark .time-label {
    color: #666;
    border-color: #2a2a2a;
  }

  html.dark .day-column {
    border-color: #3a3a3a;
  }

  html.dark .time-slot {
    border-color: #2a2a2a;
  }

  html.dark .time-slot:hover {
    background: #0a3d1f;
  }

  html.dark .time-slot.is-workday {
    background: rgba(16, 185, 129, 0.03);
    border-color: rgba(16, 185, 129, 0.1);
  }

  html.dark .time-slot.is-workday:hover {
    background: rgba(16, 185, 129, 0.08);
  }

  html.dark .event-list-header {
    border-color: #3a3a3a;
    background: #1f1f1f;
    color: #e0e0e0;
  }

  html.dark .event-list {
    background: #1f1f1f;
    border-color: #3a3a3a;
  }

  html.dark .event-list-item {
    background: #2f2f2f;
    border-left-color: #667eea;
  }

  html.dark .event-list-item:hover {
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.4);
  }

  html.dark .event-delete-btn {
    background: #f56565;
  }

  html.dark .event-delete-btn:hover {
    background: #e53e3e;
  }

  html.dark .event-item-header {
    color: #999;
  }

  html.dark .event-item-type {
    opacity: 0.9;
  }

  html.dark .event-item-time {
    color: #667eea;
  }

  html.dark .event-item-title {
    color: #e0e0e0;
  }

  html.dark .event-item-description {
    color: #999;
  }

  html.dark .no-events {
    color: #666;
  }

  /* 全天事件区域深色模式 */










/* 全天事件区域 */
/* Tooltip 样式 */
.event-tooltip {
  padding: 12px;
  line-height: 1.6;
}

.tooltip-type {
  display: inline-block;
  color: white;
  padding: 2px 10px;
  border-radius: 12px;
  font-size: 11px;
  margin-bottom: 8px;
  font-weight: 500;
}

.tooltip-title {
  font-size: 14px;
  font-weight: 600;
  color: #333;
  margin-bottom: 8px;
}

.tooltip-time {
  font-size: 12px;
  color: #666;
  margin-bottom: 8px;
  line-height: 1.5;
}

.tooltip-desc {
  font-size: 12px;
  color: #999;
  padding-top: 8px;
  border-top: 1px solid #e0e0e0;
}

/* 事件时间调整手柄 */
/* 右键菜单 */
.ctx-menu {
  position: fixed;
  z-index: 3000;
  min-width: 170px;
  background: #fff;
  border: 1px solid #e5e5e5;
  border-radius: 8px;
  box-shadow: 0 6px 24px rgba(0, 0, 0, 0.15);
  padding: 4px;
  font-size: 13px;
  color: #333;
}
.ctx-item {
  padding: 7px 12px;
  border-radius: 5px;
  cursor: pointer;
  white-space: nowrap;
}
.ctx-item:hover { background: #f0f4ff; color: #409eff; }
.ctx-item.ctx-disabled { color: #bbb; cursor: not-allowed; }
.ctx-item.ctx-disabled:hover { background: transparent; color: #bbb; }
.ctx-sep { height: 1px; background: #eee; margin: 4px 8px; }
.ctx-danger { color: #f56c6c; }
.ctx-danger:hover { background: #fef0f0; color: #f56c6c; }

/* Ctrl拖拽复制 */
.drag-copy-preview {
  position: fixed;
  z-index: 2500;
  border-radius: 4px;
  border: 2px dashed rgba(255, 255, 255, 0.9);
  box-shadow: 0 6px 20px rgba(0, 0, 0, 0.3);
  opacity: 0.8;
  pointer-events: none;
  display: flex;
  align-items: center;
  padding: 2px 6px;
  color: #fff;
  font-size: 12px;
  overflow: hidden;
  white-space: nowrap;
}
.drag-copy-title {
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
}
.drag-copy-ghost {
  position: fixed;
  z-index: 3001;
  background: rgba(0, 0, 0, 0.75);
  color: #fff;
  font-size: 11px;
  padding: 3px 9px;
  border-radius: 10px;
  pointer-events: none;
}
/* 全天事件日期拖拽手柄 */
  html.dark .ctx-menu { background: #2f2f2f; border-color: #3a3a3a; color: #ddd; }
  html.dark .ctx-item:hover { background: #3a3a3a; color: #7aa7ff; }
  html.dark .ctx-sep { background: #3a3a3a; }

</style>

<script setup>
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { ElDialog, ElForm, ElFormItem, ElInput, ElDatePicker, ElButton, ElSelect, ElOption, ElTooltip, ElNotification, ElSwitch } from 'element-plus'
import zhCn from 'element-plus/dist/locale/zh-cn.mjs'
import { sendNotification, requestPermission, isPermissionGranted } from '@tauri-apps/plugin-notification'
import { events, loadEvents } from '../store/events'
import { invoke } from "@tauri-apps/api/core";

// 日历配置参数
import {
  profile,
  preferences,
  notifications,
  privacy,
  storage,
  resetSettings as storeReset
} from '../store/settings.js'

const holidayData = ref({}) // 节假日数据

// 加载本地节假日数据
const loadHolidayData = () => {
  const raw = localStorage.getItem('holiday-data')
  if (raw) {
    try {
      holidayData.value = JSON.parse(raw)
    } catch {}
  }
}

// 保存节假日数据
const saveHolidayData = () => {
  localStorage.setItem('holiday-data', JSON.stringify(holidayData.value))
}

// 判断某天是否为节假日
const isHoliday = (date) => {
  if (!holidayData.value || typeof holidayData.value !== 'object') return false
  const dateKey = formatDateKey(date)
  const info = holidayData.value[dateKey]
  return info && info.holiday === true
}

// 获取节假日名称
const getHolidayName = (date) => {
  if (!holidayData.value || typeof holidayData.value !== 'object') return ''
  const dateKey = formatDateKey(date)
  return holidayData.value[dateKey]?.name || ''
}

// 判断某天是否为补班日
const isWorkday = (date) => {
  if (!holidayData.value || typeof holidayData.value !== 'object') return false
  const dateKey = formatDateKey(date)
  const info = holidayData.value[dateKey]
  return info && info.holiday === false && info.wage === 1
}

// 获取补班日名称
const getWorkdayName = (date) => {
  if (!holidayData.value || typeof holidayData.value !== 'object') return ''
  const dateKey = formatDateKey(date)
  const info = holidayData.value[dateKey]
  return info && info.holiday === false ? info.name : ''
}

// 格式化日期键（MM-DD）
const formatDateKey = (date) => {
  const m = String(date.getMonth() + 1).padStart(2, '0')
  const d = String(date.getDate()).padStart(2, '0')
  return `${m}-${d}`
}

async function getHolidayData() {
  const res = await invoke("http_get", { url: preferences.value.holidayApiUrl });
  console.log('Rust 响应：', res);
  holidayData.value = res.holiday;
  console.log('解析后的 holidayData：', holidayData.value);
  saveHolidayData();
}

// 事件类型配置
const eventTypes = [
  { value: 'task', label: '任务', color: '#667eea', bgColor: 'linear-gradient(135deg, #667eea 0%, #5a67d8 100%)' },
  { value: 'meeting', label: '会议', color: '#48bb78', bgColor: 'linear-gradient(135deg, #48bb78 0%, #38a169 100%)' },
  { value: 'issue', label: '问题', color: '#f56565', bgColor: 'linear-gradient(135deg, #f56565 0%, #e53e3e 100%)' },
  { value: 'reminder', label: '提醒', color: '#ed8936', bgColor: 'linear-gradient(135deg, #ed8936 0%, #dd6b20 100%)' },
  { value: 'other', label: '其他', color: '#718096', bgColor: 'linear-gradient(135deg, #718096 0%, #4a5568 100%)' }
]

// 获取事件类型配置
const getEventType = (type) => {
  return eventTypes.find(t => t.value === type) || eventTypes[4] // 默认返回"其他"
}

// 当前周的开始日期
const currentWeekStart = ref(new Date())



// 提醒开关
const reminderEnabled = ref(true)
let reminderInterval = null
const notifiedEvents = ref(new Set()) // 记录已通知的事件ID

// 拖拽状态
const isDragging = ref(false)
const dragStartTime = ref(null)
const dragDay = ref(null)

// 引用
const weekHeaderRef = ref(null)
const calendarBodyRef = ref(null)

// 同步滚动
const onCalendarScroll = () => {
  if (weekHeaderRef.value && calendarBodyRef.value) {
    weekHeaderRef.value.style.transform = `translateX(-${calendarBodyRef.value.scrollLeft}px)`
  }
}

// 每小时半小时时间片数
const HALF_HOUR_SLOTS = 2

// 使用设置中的时间配置
const START_HOUR = computed(() => parseInt(preferences.value.dayStartTime?.split(':')[0] || 8))
const END_HOUR = computed(() => parseInt(preferences.value.dayEndTime?.split(':')[0] || 23))

// 计算当前周的日期（周一到周日）
const weekDays = computed(() => {
  const days = []
  const start = new Date(currentWeekStart.value)
  const dayOfWeek = start.getDay()
  const diff = start.getDate() - dayOfWeek + (dayOfWeek === 0 ? -6 : 1)
  start.setDate(diff)

  for (let i = 0; i < 7; i++) {
    const date = new Date(start)
    date.setDate(start.getDate() + i)
    days.push(date)
  }
  return days
})

// 生成时间轴（使用设置中的时间范围，每半小时一个单位）
const timeSlots = computed(() => {
  const slots = []
  const startHour = START_HOUR.value
  const endHour = END_HOUR.value
  for (let hour = startHour; hour < endHour; hour++) {
    for (let half = 0; half < HALF_HOUR_SLOTS; half++) {
      slots.push({
        hour: hour,
        half: half,
        value: hour + half * 0.5
      })
    }
  }
  // 添加最后一小时
  slots.push({
    hour: endHour,
    half: 0,
    value: endHour
  })
  return slots
})

// 生成时间轴标签（只显示整点）
const timeLabels = computed(() => {
  const labels = []
  const startHour = START_HOUR.value
  const endHour = END_HOUR.value
  for (let hour = startHour; hour <= endHour; hour++) {
    labels.push(hour)
  }
  return labels
})

// 格式化日期
const formatDate = (date) => {
  return `${date.getMonth() + 1}/${date.getDate()}`
}

// 格式化时间
const formatTime = (hour) => {
  const h = Math.floor(hour)
  const m = (hour % 1) * 60
  return `${h.toString().padStart(2, '0')}:${m === 0 ? '00' : m.toString()}`
}

// 获取某天的事件
const getEventsByDay = (day) => {
  const dayStr = day.toISOString().split('T')[0]
  return events.value.filter(event => {
    if (!event.isAllDay) {
      return event.date === dayStr
    }
    // 全天事件：检查日期范围
    const eventStart = new Date(event.date)
    const eventEnd = new Date(event.endDate || event.date)
    const targetDay = new Date(dayStr)
    return targetDay >= eventStart && targetDay <= eventEnd
  })
}

// 获取某天的全天事件
const getAllDayEvents = (day) => {
  const dayStr = day.toISOString().split('T')[0]
  return events.value.filter(event => {
    if (!event.isAllDay) return false

    const eventStart = new Date(event.date)
    const eventEnd = new Date(event.endDate || event.date)
    const targetDay = new Date(dayStr)
    return targetDay >= eventStart && targetDay <= eventEnd
  })
}

// 获取某天的非全天事件
const getRegularEvents = (day) => {
  const dayEvents = getEventsByDay(day)
  return dayEvents.filter(event => !event.isAllDay)
}

// 计算事件的位置和高度（支持并行事件）
const getDayEventsForRender = (day) => {
  const dayEvents = getRegularEvents(day)
  if (dayEvents.length === 0) return []

  // 为每个事件计算时间槽索引
  const eventsWithSlots = dayEvents.map(event => {
    const startSlotIndex = timeSlots.value.findIndex(slot => Math.abs(slot.value - event.startTime) < 0.001)
    const endSlotIndex = timeSlots.value.findIndex(slot => Math.abs(slot.value - event.endTime) < 0.001)
    return {
      event,
      startSlotIndex,
      endSlotIndex
    }
  }).filter(e => e.startSlotIndex !== -1 && e.endSlotIndex !== -1)

  // 按开始时间排序
  eventsWithSlots.sort((a, b) => a.startSlotIndex - b.startSlotIndex)

  // 为事件分配列和宽度
  const columns = [] // 每列的事件 [ { eventId, startSlotIndex, endSlotIndex, columnIndex } ]
  const maxColumnsPerSlot = new Map() // 记录每个时间槽需要的最大列数

  eventsWithSlots.forEach(({ event, startSlotIndex, endSlotIndex }) => {
    let columnIndex = 0
    let foundColumn = false

    // 尝试在现有列中找到可以放置的位置
    while (!foundColumn) {
      let canUseColumn = true

      // 检查该列中是否有与当前事件重叠的事件
      for (const colEvent of columns) {
        if (colEvent.columnIndex === columnIndex) {
          // 判断时间是否重叠
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

    // 将事件放入该列
    columns.push({
      eventId: event.id,
      startSlotIndex,
      endSlotIndex,
      columnIndex
    })

    // 更新每个时间槽的最大列数
    for (let i = startSlotIndex; i < endSlotIndex; i++) {
      const currentMax = maxColumnsPerSlot.get(i) || 0
      maxColumnsPerSlot.set(i, Math.max(currentMax, columnIndex + 1))
    }
  })

  // 计算每个事件的样式
  return eventsWithSlots.map(({ event, startSlotIndex, endSlotIndex }) => {
    const columnData = columns.find(c => c.eventId === event.id)
    if (!columnData) return { ...event, style: {} }

    const top = startSlotIndex * 30
    const height = (endSlotIndex - startSlotIndex) * 30

    // 找到该事件覆盖的所有时间槽中的最大列数
    let maxColsInTimeRange = 1
    for (let i = startSlotIndex; i < endSlotIndex; i++) {
      maxColsInTimeRange = Math.max(maxColsInTimeRange, maxColumnsPerSlot.get(i) || 1)
    }

    // 计算宽度和位置
    const padding = 4
    const availableWidth = 100 - (padding * 2)
    const columnWidth = availableWidth / maxColsInTimeRange
    const left = padding + (columnData.columnIndex * columnWidth)
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

// 获取可选的结束时间（必须大于开始时间）
const availableEndTimes = computed(() => {
  if (!form.value.startTime || form.value.startTime === '') {
    return timeSlots.value
  }
  return timeSlots.value.filter(slot => slot.value > form.value.startTime)
})

// 获取事件详细信息HTML
const getEventDetailHtml = (event) => {
  const timeText = event.isAllDay ? '全天' : `${formatTime(event.startTime)} - ${formatTime(event.endTime)}`
  const dateText = event.isAllDay && event.endDate && event.endDate !== event.date
    ? `${event.date} 至 ${event.endDate}`
    : event.date
  return `
    <div class="event-tooltip">
      <div class="tooltip-type" style="background: ${getEventType(event.type).color}">
        ${getEventType(event.type).label}
      </div>
      <div class="tooltip-title">${event.title || '无标题'}</div>
      <div class="tooltip-time">
        📅 ${dateText}<br>
        ⏰ ${timeText}
      </div>
      ${event.description ? `<div class="tooltip-desc">${event.description}</div>` : ''}
    </div>
  `
}

// 开始拖拽
const startDrag = (e, day, slot) => {
  isDragging.value = true
  dragDay.value = day
  dragStartTime.value = slot.value
}

// 在全天事件区域点击创建新事件
const startDragAllDay = (e, day) => {
  const eventDate = day.toISOString().split('T')[0]

  // 创建临时全天事件对象
  const tempEvent = {
    id: Date.now(),
    date: eventDate,
    endDate: eventDate,
    startTime: START_HOUR.value,
    endTime: END_HOUR.value,
    title: '',
    type: 'allDay',
    isAllDay: true
  }

  // 标记为新创建的事件
  currentEvent.value = tempEvent
  // 初始化表单值为新事件的值（使用日期范围格式）
  form.value = {
    title: tempEvent.title,
    date: [tempEvent.date, tempEvent.endDate],
    endDate: tempEvent.endDate,
    startTime: tempEvent.startTime,
    endTime: tempEvent.endTime,
    description: '',
    type: tempEvent.type,
    isAllDay: tempEvent.isAllDay
  }
  showDialog.value = true
}

// 拖拽中
const onDrag = (e) => {
  if (!isDragging.value) return
  e.preventDefault()
}

// 结束拖拽
const endDrag = (e, day, slot) => {
  if (!isDragging.value) return
  isDragging.value = false

  const endValue = slot.value

  // 使用拖拽起始的日期，确保事件在正确的一天
  if (dragDay.value && dragStartTime.value !== null) {
    const eventDate = dragDay.value.toISOString().split('T')[0]
    const startTime = Math.min(dragStartTime.value, endValue)
    let endTime = Math.max(dragStartTime.value, endValue)

    // 至少需要30分钟的时长
    if (endTime - startTime < 0.5) {
      // 如果时长不足30分钟，自动扩展到30分钟
      endTime = startTime + 0.5
    }

    // 创建临时事件对象（暂不添加到事件列表）
    const tempEvent = {
      id: Date.now(),
      date: eventDate,
      startTime: startTime,
      endTime: endTime,
      title: '',
      type: 'task',
      isAllDay: false
    }
    // 标记为新创建的事件
    currentEvent.value = tempEvent
    // 初始化表单值为新事件的值
    form.value = {
      title: tempEvent.title,
      date: tempEvent.date,
      startTime: tempEvent.startTime,
      endTime: tempEvent.endTime,
      description: '',
      type: tempEvent.type,
      isAllDay: tempEvent.isAllDay
    }
    showDialog.value = true
  }

  dragStartTime.value = null
  dragDay.value = null
}

// 事件弹窗
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

// 打开事件弹窗
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

// 保存事件
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
        // 全天事件：处理日期范围
        if (Array.isArray(form.value.date)) {
          event.date = form.value.date[0]
          event.endDate = form.value.date[1] || form.value.date[0]
        } else {
          event.date = form.value.date
          event.endDate = form.value.endDate || form.value.date
        }
      } else {
        // 非全天事件：处理时间和日期
        event.date = form.value.date
        event.startTime = form.value.startTime
        event.endTime = form.value.endTime
      }
    } else {
      // 新事件，添加到事件列表
      const newEvent = {
        ...currentEvent.value,
        title: form.value.title,
        type: form.value.type,
        isAllDay: form.value.isAllDay,
        description: form.value.description
      }

      if (form.value.isAllDay) {
        // 全天事件：处理日期范围
        if (Array.isArray(form.value.date)) {
          newEvent.date = form.value.date[0]
          newEvent.endDate = form.value.date[1] || form.value.date[0]
        } else {
          newEvent.date = form.value.date
          newEvent.endDate = form.value.endDate || form.value.date
        }
      } else {
        // 非全天事件：处理时间和日期
        newEvent.date = form.value.date
        newEvent.startTime = form.value.startTime
        newEvent.endTime = form.value.endTime
      }

      events.value.push(newEvent)
    }
  }
  showDialog.value = false
}

// 删除事件
const deleteEvent = () => {
  if (currentEvent.value) {
    const event = events.value.find(e => e.id === currentEvent.value.id)
    if (event) {
      // 已存在的事件，删除它
      events.value = events.value.filter(e => e.id !== currentEvent.value.id)
    }
    // 新事件直接关闭弹窗即可（不需要删除，因为还没添加）
  }
  showDialog.value = false
}

// 列表中直接删除事件
const deleteEventItem = (event) => {
  events.value = events.value.filter(e => e.id !== event.id)
}

// 切换到上一周
const prevWeek = () => {
  const newDate = new Date(currentWeekStart.value)
  newDate.setDate(newDate.getDate() - 7)
  currentWeekStart.value = newDate
}

// 切换到下一周
const nextWeek = () => {
  const newDate = new Date(currentWeekStart.value)
  newDate.setDate(newDate.getDate() + 7)
  currentWeekStart.value = newDate
}

// 返回本周
const goToToday = () => {
  currentWeekStart.value = new Date()
}

// 判断是否为今天
const isToday = (day) => {
  const today = new Date()
  return day.getDate() === today.getDate() &&
         day.getMonth() === today.getMonth() &&
         day.getFullYear() === today.getFullYear()
}

// 获取周数（ISO周数）
const getWeekNumber = (date) => {
  const d = new Date(date)
  d.setHours(0, 0, 0, 0)
  d.setDate(d.getDate() + 4 - (d.getDay() || 7))
  const yearStart = new Date(d.getFullYear(), 0, 1)
  const weekNo = Math.ceil((((d - yearStart) / 86400000) + 1) / 7)
  return weekNo
}

// 获取全年总周数
const getTotalWeeksInYear = (year) => {
  const lastDay = new Date(year, 11, 31)
  const weekNo = getWeekNumber(lastDay)
  // 如果12月31日属于下一年的第一周，则返回52或53
  return weekNo > 52 ? weekNo - 1 : weekNo
}

// 计算本年剩余周数
const remainingWeeks = computed(() => {
  const currentYear = weekDays.value[0]?.getFullYear()
  const currentWeek = getWeekNumber(weekDays.value[0])
  const totalWeeks = getTotalWeeksInYear(currentYear)
  return totalWeeks - currentWeek
})

// 获取本周所有事件
const weekEvents = computed(() => {
  const weekDateStrings = weekDays.value.map(day => day.toISOString().split('T')[0])
  return events.value.filter(event => {
    // 检查事件是否在本周范围内
    if (event.isAllDay) {
      // 全天事件：检查事件日期范围是否与本周有交集
      const eventStart = new Date(event.date)
      const eventEnd = new Date(event.endDate || event.date)
      const weekStart = new Date(weekDateStrings[0])
      const weekEnd = new Date(weekDateStrings[6])
      return eventEnd >= weekStart && eventStart <= weekEnd
    } else {
      // 非全天事件：检查日期是否在本周
      return weekDateStrings.includes(event.date)
    }
  }).sort((a, b) => {
    // 按日期和时间排序
    const dateCompare = a.date.localeCompare(b.date)
    if (dateCompare !== 0) return dateCompare
    return a.startTime - b.startTime
  })
})

// 本周事件统计
const weekStatistics = computed(() => {
  const eventsList = weekEvents.value
  const totalCount = eventsList.length
  const allDayCount = eventsList.filter(e => e.isAllDay).length
  const multiDayCount = eventsList.filter(e => e.isAllDay && e.endDate && e.endDate !== e.date).length
  
  return {
    totalCount,
    allDayCount,
    multiDayCount
  }
})

onMounted(() => {
  getHolidayData()
  loadEvents()
  loadHolidayData()

  // 启动提醒检查
  startReminderCheck()

  // 组件卸载时清理定时器
  onUnmounted(() => {
    stopReminderCheck()
  })
})


// 监听开始时间变化，自动调整结束时间
watch(() => form.value.startTime, (newStartTime) => {
  if (form.value.endTime && form.value.endTime <= newStartTime) {
    // 如果结束时间小于等于开始时间，设置为下一个时间段
    const nextSlot = timeSlots.value.find(slot => slot.value > newStartTime)
    form.value.endTime = nextSlot ? nextSlot.value : newStartTime + 0.5
  }
})

// 发送提醒通知
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

// 测试提醒
const testReminder = async () => {
  const testEvent = {
    title: '测试提醒',
    type: 'reminder',
    description: "这是一个测试提醒"
  }
  await sendEventReminder(testEvent);
}

// 检查需要提醒的事件
const checkReminders = () => {
  if (!reminderEnabled.value) return

  const now = new Date()
  const todayStr = now.toISOString().split('T')[0]
  const currentHour = now.getHours()
  const currentMinute = now.getMinutes()
  const currentTime = currentHour + currentMinute / 60

  // 检查当前时间段前5分钟内的事件
  events.value.forEach(event => {
    if (event.date === todayStr && !event.isAllDay) {
      const reminderTime = event.startTime - 5/60 // 提前5分钟提醒
      const eventKey = `${event.id}-${todayStr}`

      // 如果事件还未通知，且到达提醒时间
      if (!notifiedEvents.value.has(eventKey) && currentTime >= reminderTime && currentTime < event.startTime) {
        sendEventReminder(event)
        notifiedEvents.value.add(eventKey)
      }
    }
  })

  // 清理过期的通知记录（超过1天的记录）
  const oldDate = new Date(now.getTime() - 24 * 60 * 60 * 1000).toISOString().split('T')[0]
  notifiedEvents.value.forEach(key => {
    if (key.includes(oldDate)) {
      notifiedEvents.value.delete(key)
    }
  })
}

// 启动提醒检查
const startReminderCheck = () => {
  // 每分钟检查一次
  reminderInterval = setInterval(checkReminders, 60 * 1000)
}

// 停止提醒检查
const stopReminderCheck = () => {
  if (reminderInterval) {
    clearInterval(reminderInterval)
    reminderInterval = null
  }
}

// 检查全天事件是否是跨天事件的第一天
const isAllDayEventFirstDay = (event, day) => {
  const dayStr = day.toISOString().split('T')[0]
  return event.date === dayStr
}

// 获取本周所有全天事件并计算布局
const allDayEventsLayout = computed(() => {
  const weekDateStrings = weekDays.value.map(day => day.toISOString().split('T')[0])

  // 获取所有与本周相关的全天事件
  const relevantEvents = events.value.filter(event => {
    if (!event.isAllDay) return false

    const eventStart = new Date(event.date)
    const eventEnd = new Date(event.endDate || event.date)
    const weekStart = new Date(weekDateStrings[0])
    const weekEnd = new Date(weekDateStrings[6])

    return eventEnd >= weekStart && eventStart <= weekEnd
  })

  // 为每个事件计算在本周的开始和结束位置
  return relevantEvents.map(event => {
    const eventStartStr = event.date
    const eventEndStr = event.endDate || event.date

    const startIndex = weekDateStrings.findIndex(date => date >= eventStartStr)
    const endIndex = weekDateStrings.findIndex(date => date > eventEndStr)

    const actualStartIndex = Math.max(0, startIndex)
    const actualEndIndex = endIndex === -1 ? 7 : endIndex

    return {
      ...event,
      startIndex: actualStartIndex,
      endIndex: actualEndIndex,
      span: actualEndIndex - actualStartIndex
    }
  }).sort((a, b) => {
    // 先按开始日期排序
    if (a.startIndex !== b.startIndex) {
      return a.startIndex - b.startIndex
    }
    // 开始日期相同，按跨度的逆序排序（跨度大的优先）
    return b.span - a.span
  })
})

// 为全天事件分配行和列
const allDayEventsWithLayout = computed(() => {
  const events = [...allDayEventsLayout.value]
  const rows = [] // 每行的事件

  events.forEach(event => {
    // 找到可以放置的行
    let placed = false
    for (let i = 0; i < rows.length; i++) {
      const row = rows[i]
      // 检查该行是否有重叠
      const hasOverlap = row.some(e =>
        !(event.startIndex >= e.endIndex || event.endIndex <= e.startIndex)
      )
      if (!hasOverlap) {
        // 放入该行
        row.push(event)
        event.row = i
        placed = true
        break
      }
    }

    // 如果没有找到合适的行，创建新行
    if (!placed) {
      rows.push([event])
      event.row = rows.length - 1
    }
  })

  return events
})

// 获取每一行的事件
const getRowEvents = (rowIndex) => {
  return allDayEventsWithLayout.value.filter(e => e.row === rowIndex)
}

// 获取全天事件区域的行数
const allDayEventsRowCount = computed(() => {
  const maxRow = allDayEventsWithLayout.value.reduce((max, e) => Math.max(max, e.row), 0)
  return maxRow + 1
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
          <el-switch v-model="reminderEnabled" />
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
      <div class="all-day-events-section" :style="{ height: `${allDayEventsRowCount * 26}px` }">
        <div class="all-day-time-header">
          <div class="all-day-label">全天</div>
        </div>
        <div class="all-day-days-container">
          <div v-for="row in allDayEventsRowCount" :key="row" class="all-day-events-row">
            <div class="all-day-events-grid">
              <div
                v-for="day in weekDays"
                :key="day.getTime()"
                class="all-day-day-cell"
                :class="{ 'today': isToday(day), 'weekend': day.getDay() === 0 || day.getDay() === 6, 'is-workday': isWorkday(day) }"
                @mousedown.stop="startDragAllDay($event, day)"
              ></div>
              <div
                v-for="event in getRowEvents(row - 1)"
                :key="event.id"
                class="all-day-event-bar"
                :style="{
                  left: `${(event.startIndex / 7) * 100}%`,
                  width: `${(event.span / 7) * 100}%`,
                  background: getEventType(event.type).bgColor,
                  zIndex: 10
                }"
                @mousedown.stop="openEventDialog(event)"
              >
                <el-tooltip
                  :content="getEventDetailHtml(event)"
                  raw-content
                  placement="top"
                  :show-after="200"
                >
                  <div class="all-day-event-content">
                    <span class="all-day-event-title">{{ event.title }}</span>
                    <span v-if="event.span > 1" class="all-day-event-duration">{{ event.span }}天</span>
                  </div>
                </el-tooltip>
              </div>
            </div>
          </div>
        </div>
        <div class="all-day-event-list-placeholder"></div>
      </div>

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
              @mousedown="startDrag($event, day, slot)"
              @mousemove="onDrag($event)"
              @mouseup="endDrag($event, day, slot)"
              @mouseleave="isDragging = false"
            ></div>
            <!-- 渲染事件（使用绝对定位） -->
            <el-tooltip
              v-for="event in getDayEventsForRender(day)"
              :key="event.id"
              :content="getEventDetailHtml(event)"
              raw-content
              placement="top"
              :show-after="200"
            >
              <div
                class="event-item-absolute"
                :style="{ ...event.style, background: getEventType(event.type).bgColor }"
                @mousedown.stop="openEventDialog(event)"
              >
                <span class="event-time">{{ formatTime(event.startTime) }}-{{ formatTime(event.endTime) }}</span>
                <span class="event-title">{{ event.title }}</span>
                <span class="event-type-tag">{{ getEventType(event.type).label }}</span>
              </div>
            </el-tooltip>
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
    <el-dialog v-model="showDialog" title="编辑事件" width="500px">
      <el-form :model="form" label-width="80px">
        <el-form-item label="事件类型">
          <el-radio-group v-model="form.type">
            <el-radio v-for="type in eventTypes" :key="type.value" :value="type.value">
              {{ type.label }}
            </el-radio>
          </el-radio-group>
        </el-form-item>
        <el-form-item label="事件名称">
          <el-input v-model="form.title" placeholder="输入事件名称" />
        </el-form-item>
        <el-form-item label="事件周期" v-if="form.isAllDay">
          <el-date-picker
            v-model="form.date"
            type="daterange"
            start-placeholder="开始日期"
            end-placeholder="结束日期"
            format="YYYY/MM/DD"
            value-format="YYYY-MM-DD"
            :locale="zhCn"
          />
        </el-form-item>
        <el-form-item label="日期" v-if="!form.isAllDay">
          <el-date-picker
            v-model="form.date"
            type="date"
            placeholder="选择日期"
            format="YYYY/MM/DD"
            value-format="YYYY-MM-DD"
            :locale="zhCn"
          />
        </el-form-item>
        <template v-if="!form.isAllDay">
          <el-form-item label="开始时间">
            <el-select v-model="form.startTime" placeholder="选择开始时间">
              <el-option v-for="slot in timeSlots" :key="slot.value" :label="formatTime(slot.value)" :value="slot.value" />
            </el-select>
          </el-form-item>
          <el-form-item label="结束时间">
            <el-select v-model="form.endTime" placeholder="选择结束时间">
              <el-option v-for="slot in availableEndTimes" :key="slot.value" :label="formatTime(slot.value)" :value="slot.value" />
            </el-select>
          </el-form-item>
        </template>
        <el-form-item label="描述">
          <el-input
            v-model="form.description"
            type="textarea"
            :rows="3"
            placeholder="输入事件描述"
          />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="deleteEvent" type="danger">删除</el-button>
        <el-button @click="showDialog = false">取消</el-button>
        <el-button type="primary" @click="saveEvent">保存</el-button>
      </template>
    </el-dialog>
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
  background: #f5f5f5;
  overflow: hidden;
}


.calendar-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 20px 30px;
  background: white;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
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

.event-item-absolute {
  position: absolute;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  color: white;
  padding: 4px 4px;
  border-radius: 4px;
  font-size: 12px;
  cursor: pointer;
  transition: transform 0.2s, box-shadow 0.2s;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
  display: flex;
  align-items: center;
  gap: 4px;
  min-width: 0;
  box-sizing: border-box;
}

.event-item-absolute:hover {
  transform: translateY(-2px);
  box-shadow: 0 4px 12px rgba(102, 126, 234, 0.4);
  z-index: 10;
}

.event-type-tag {
  font-size: 10px;
  background: rgba(255, 255, 255, 0.2);
  padding: 1px 6px;
  border-radius: 8px;
  white-space: nowrap;
}

.event-time {
  font-size: 10px;
  opacity: 0.9;
  flex-shrink: 0;
}

.event-title {
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
  min-width: 0;
}

@media (prefers-color-scheme: dark) {
  .calendar-app {
    background: #1a1a1a;
  }

  .calendar-header,
  .calendar-container {
    background: #2f2f2f;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
  }

  .week-header {
    background: #1f1f1f;
    border-color: #3a3a3a;
  }

  .day-header {
    border-color: #3a3a3a;
  }

  .day-name {
    color: #999;
  }

  .day-date {
    color: #e0e0e0;
  }

  .day-header.today {
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  }

  .day-header.weekend {
    background: #3d1f1f;
  }

  .day-header.weekend .day-name {
    color: #fc8181;
  }

  .rest-tag {
    color: #fc8181;
    background: #742a2a;
  }

  .current-month {
    color: #e0e0e0;
  }

  .time-column {
    border-color: #3a3a3a;
  }

  .calendar-grid-wrapper {
    border-color: #3a3a3a;
  }

  .time-label {
    color: #666;
    border-color: #2a2a2a;
  }

  .day-column {
    border-color: #3a3a3a;
  }

  .time-slot {
    border-color: #2a2a2a;
  }

  .time-slot:hover {
    background: #0a3d1f;
  }

  .time-slot.is-workday {
    background: rgba(16, 185, 129, 0.03);
    border-color: rgba(16, 185, 129, 0.1);
  }

  .time-slot.is-workday:hover {
    background: rgba(16, 185, 129, 0.08);
  }

  .event-list-header {
    border-color: #3a3a3a;
    background: #1f1f1f;
    color: #e0e0e0;
  }

  .event-list {
    background: #1f1f1f;
    border-color: #3a3a3a;
  }

  .event-list-item {
    background: #2f2f2f;
    border-left-color: #667eea;
  }

  .event-list-item:hover {
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.4);
  }

  .event-delete-btn {
    background: #f56565;
  }

  .event-delete-btn:hover {
    background: #e53e3e;
  }

  .event-item-header {
    color: #999;
  }

  .event-item-type {
    opacity: 0.9;
  }

  .event-item-time {
    color: #667eea;
  }

  .event-item-title {
    color: #e0e0e0;
  }

  .event-item-description {
    color: #999;
  }

  .no-events {
    color: #666;
  }

  /* 全天事件区域深色模式 */
  .all-day-events-section {
    background: #1f1f1f;
    border-color: #3a3a3a;
  }

  .all-day-time-header {
    border-color: #3a3a3a;
  }

  .all-day-days-container {
    border-color: #3a3a3a;
  }

  .all-day-label {
    color: #666;
  }

  .all-day-events-row {
    border-color: #2a2a2a;
  }

  .all-day-day-cell {
    border-color: #2a2a2a;
  }

  .all-day-day-cell:hover {
    background: #0a3d1f;
  }

  .all-day-day-cell.today {
    background: linear-gradient(135deg, rgba(102, 126, 234, 0.2) 0%, rgba(118, 75, 162, 0.2) 100%);
  }

  .all-day-day-cell.weekend {
    background: rgba(245, 101, 101, 0.1);
  }

  .all-day-event-list-placeholder {
    border-color: #3a3a3a;
  }
}

/* 全天事件区域 */
.all-day-events-section {
  display: grid;
  grid-template-columns: 60px 1fr 300px;
  border-bottom: 1px solid #e0e0e0;
  background: #fafafa;
  box-sizing: border-box;
  width: 100%;
  flex-shrink: 0;
}

.all-day-time-header {
  border-right: 1px solid #e0e0e0;
  box-sizing: border-box;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
}

.all-day-label {
  font-size: 12px;
  color: #999;
  font-weight: 500;
}

.all-day-days-container {
  display: flex;
  flex-direction: column;
  border-right: 1px solid #e0e0e0;
  position: relative;
}

.all-day-events-row {
  height: 26px;
  border-bottom: 0px solid #f0f0f0;
  position: relative;
}

.all-day-events-grid {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  width: 100%;
  height: 100%;
}

.all-day-day-cell {
  border-right: 1px solid #f0f0f0;
  height: 100%;
  cursor: pointer;
  transition: background 0.2s;
}

.all-day-day-cell:last-child {
  border-right: none;
}

.all-day-day-cell:hover {
  background: #f0f9ff;
}

.all-day-day-cell.today {
  background: linear-gradient(135deg, rgba(102, 126, 234, 0.1) 0%, rgba(118, 75, 162, 0.1) 100%);
}

.all-day-day-cell.weekend {
  background: rgba(245, 101, 101, 0.05);
}

.all-day-day-cell.is-workday {
  background: rgba(16, 185, 129, 0.05);
}

.all-day-event-bar {
  position: absolute;
  top: 1px;
  height: 24px;
  padding: 0 8px;
  border-radius: 4px;
  color: white;
  font-size: 11px;
  cursor: pointer;
  display: flex;
  align-items: center;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.1);
  transition: transform 0.2s, box-shadow 0.2s;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.all-day-event-bar:hover {
  transform: translateY(-1px);
  box-shadow: 0 3px 8px rgba(102, 126, 234, 0.4);
  z-index: 20;
}

.all-day-event-content {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  min-width: 0;
}

.all-day-event-title {
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
}

.all-day-event-duration {
  font-size: 10px;
  background: rgba(255, 255, 255, 0.2);
  padding: 1px 6px;
  border-radius: 8px;
  white-space: nowrap;
  flex-shrink: 0;
}

.all-day-event-list-placeholder {
  border-left: 1px solid #e0e0e0;
  box-sizing: border-box;
}

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

/* 单选框样式优化 */
:deep(.el-radio-group) {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
}

:deep(.el-radio) {
  margin-right: 0;
}

:deep(.el-radio__label) {
  font-size: 13px;
}
</style>

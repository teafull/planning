<script setup>
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { ElDialog, ElForm, ElFormItem, ElInput, ElDatePicker, ElButton, ElSelect, ElOption, ElTooltip, ElNotification, ElSwitch } from 'element-plus'
import { sendNotification, requestPermission, isPermissionGranted } from '@tauri-apps/plugin-notification'

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

// 事件数据
const events = ref([])

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
const START_HOUR = 0
const END_HOUR = 22

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

// 生成时间轴（8:00 - 22:00，每半小时一个单位）
const timeSlots = computed(() => {
  const slots = []
  for (let hour = START_HOUR; hour < END_HOUR; hour++) {
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
    hour: END_HOUR,
    half: 0,
    value: END_HOUR
  })
  return slots
})

// 生成时间轴标签（只显示整点）
const timeLabels = computed(() => {
  const labels = []
  for (let hour = START_HOUR; hour <= END_HOUR; hour++) {
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
  return events.value.filter(event => event.date === dayStr)
}

// 计算事件的位置和高度
const getEventStyle = (event) => {
  const startSlotIndex = timeSlots.value.findIndex(slot => Math.abs(slot.value - event.startTime) < 0.001)
  const endSlotIndex = timeSlots.value.findIndex(slot => Math.abs(slot.value - event.endTime) < 0.001)

  if (startSlotIndex === -1 || endSlotIndex === -1) {
    return {}
  }

  const top = startSlotIndex * 30
  const height = (endSlotIndex - startSlotIndex) * 30

  return {
    position: 'absolute',
    top: `${top}px`,
    height: `${height}px`,
    left: '4px',
    right: '4px'
  }
}

// 每天的事件列表（用于绝对定位渲染）
const getDayEventsForRender = (day) => {
  return getEventsByDay(day).map(event => ({
    ...event,
    style: getEventStyle(event)
  }))
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
  return `
    <div class="event-tooltip">
      <div class="tooltip-type" style="background: ${getEventType(event.type).color}">
        ${getEventType(event.type).label}
      </div>
      <div class="tooltip-title">${event.title || '无标题'}</div>
      <div class="tooltip-time">
        📅 ${event.date}<br>
        ⏰ ${formatTime(event.startTime)} - ${formatTime(event.endTime)}
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
      type: 'task'
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
      type: tempEvent.type
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
  startTime: 9,
  endTime: 10,
  description: '',
  type: 'task'
})

// 打开事件弹窗
const openEventDialog = (event) => {
  currentEvent.value = event
  form.value = {
    title: event.title,
    date: event.date,
    startTime: event.startTime,
    endTime: event.endTime,
    description: event.description || '',
    type: event.type || 'task'
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
      event.startTime = form.value.startTime
      event.endTime = form.value.endTime
      event.description = form.value.description
      event.type = form.value.type
    } else {
      // 新事件，添加到事件列表
      events.value.push({
        ...currentEvent.value,
        title: form.value.title,
        startTime: form.value.startTime,
        endTime: form.value.endTime,
        description: form.value.description,
        type: form.value.type
      })
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
  return events.value.filter(event => weekDateStrings.includes(event.date)).sort((a, b) => {
    // 按日期和时间排序
    const dateCompare = a.date.localeCompare(b.date)
    if (dateCompare !== 0) return dateCompare
    return a.startTime - b.startTime
  })
})

onMounted(() => {
  const savedEvents = localStorage.getItem('calendar-events')
  if (savedEvents) {
    events.value = JSON.parse(savedEvents)
  }

  // 启动提醒检查
  startReminderCheck()

  // 组件卸载时清理定时器
  onUnmounted(() => {
    stopReminderCheck()
  })
})

// 监听 events 变化并保存到本地存储
watch(events, (newEvents) => {
  localStorage.setItem('calendar-events', JSON.stringify(newEvents))
}, { deep: true })

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
    if (event.date === todayStr) {
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

// 监听提醒开关
watch(reminderEnabled, (enabled) => {
  if (enabled) {
    startReminderCheck()
  } else {
    stopReminderCheck()
  }
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
          <div class="day-name">{{ ['周一', '周二', '周三', '周四', '周五', '周六', '周日'][day.getDay() === 0 ? 6 : day.getDay() - 1] }}</div>
          <div class="day-date">{{ formatDate(day) }}</div>
          <div v-if="day.getDay() === 0 || day.getDay() === 6" class="rest-tag">休息</div>
        </div>
        <div class="event-list-header">本周事件</div>
      </div>

      <!-- 日历主体 - 包含滚动容器 -->
      <div class="calendar-scroll-container">
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
            :class="{ 'half-hour': slot.half === 1 }"
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
                    <span class="event-item-time">{{ formatTime(event.startTime) }}-{{ formatTime(event.endTime) }}</span>
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
        <el-form-item label="日期">
          <el-date-picker
            v-model="form.date"
            type="date"
            placeholder="选择日期"
            format="YYYY/MM/DD"
            value-format="YYYY-MM-DD"
          />
        </el-form-item>
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
  width: 100vw;
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: #f5f5f5;
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
}

.week-header {
  display: grid;
  grid-template-columns: 60px repeat(7, 1fr) 300px;
  border-bottom: 1px solid #e0e0e0;
  background: #fafafa;
  box-sizing: border-box;
  width: 100%;
  flex-shrink: 0;
  /* 预留滚动条宽度 */
  padding-right: 17px;
}

.calendar-scroll-container {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.time-header {
  border-right: 1px solid #e0e0e0;
  box-sizing: border-box;
}

.day-header {
  padding: 15px;
  text-align: center;
  border-right: 1px solid #e0e0e0;
  box-sizing: border-box;
}

.day-header:last-child {
  border-right: none;
}

.event-list-header {
  padding: 15px;
  text-align: center;
  font-size: 14px;
  font-weight: 600;
  color: #333;
  border-left: 1px solid #e0e0e0;
  background: #fafafa;
  box-sizing: border-box;
}

.day-name {
  font-size: 14px;
  color: #666;
  margin-bottom: 5px;
}

.day-date {
  font-size: 18px;
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
  margin-top: 4px;
  font-size: 10px;
  color: #e53e3e;
  background: #fed7d7;
  padding: 2px 8px;
  border-radius: 10px;
  display: inline-block;
}

.calendar-scroll-container {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.calendar-body {
  flex: 1;
  display: grid;
  grid-template-columns: 60px repeat(7, 1fr) 300px;
  overflow-y: scroll;
  overflow-x: hidden;
  position: relative;
  box-sizing: border-box;
  width: 100%;
  height: 100%;
}

/* 强制显示滚动条，确保宽度固定 */
.calendar-body::-webkit-scrollbar {
  width: 17px;
}

.calendar-body::-webkit-scrollbar-track {
  background: transparent;
}

.calendar-body::-webkit-scrollbar-thumb {
  background: rgba(0, 0, 0, 0.2);
  border-radius: 3px;
}

.calendar-body::-webkit-scrollbar-thumb:hover {
  background: rgba(0, 0, 0, 0.3);
}

.time-column {
  border-right: 1px solid #e0e0e0;
  display: flex;
  flex-direction: column;
}

.event-list {
  border-left: 1px solid #e0e0e0;
  display: flex;
  flex-direction: column;
  background: #fafafa;
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
  padding: 4px 8px;
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
  gap: 6px;
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
  margin-bottom: 2px;
  display: block;
}

.event-title {
  font-weight: 500;
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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

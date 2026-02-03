<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { ElDialog, ElForm, ElFormItem, ElInput, ElDatePicker, ElButton, ElSelect, ElOption } from 'element-plus'

// 当前周的开始日期
const currentWeekStart = ref(new Date())

// 事件数据
const events = ref([])

// 拖拽状态
const isDragging = ref(false)
const dragStartTime = ref(null)
const dragDay = ref(null)

// 每小时半小时时间片数
const HALF_HOUR_SLOTS = 2
const START_HOUR = 8
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

  if (dragDay.value && dragStartTime.value !== null) {
    const newEvent = {
      id: Date.now(),
      date: day.toISOString().split('T')[0],
      startTime: Math.min(dragStartTime.value, endValue),
      endTime: Math.max(dragStartTime.value, endValue),
      title: '新事件'
    }
    events.value.push(newEvent)
    showDialog.value = true
    currentEvent.value = newEvent
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
  description: ''
})

// 打开事件弹窗
const openEventDialog = (event) => {
  currentEvent.value = event
  form.value = {
    title: event.title,
    date: event.date,
    startTime: event.startTime,
    endTime: event.endTime,
    description: event.description || ''
  }
  showDialog.value = true
}

// 保存事件
const saveEvent = () => {
  if (currentEvent.value) {
    const event = events.value.find(e => e.id === currentEvent.value.id)
    if (event) {
      event.title = form.value.title
      event.startTime = form.value.startTime
      event.endTime = form.value.endTime
      event.description = form.value.description
    }
  }
  showDialog.value = false
}

// 删除事件
const deleteEvent = () => {
  if (currentEvent.value) {
    events.value = events.value.filter(e => e.id !== currentEvent.value.id)
  }
  showDialog.value = false
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
})

// 监听 events 变化并保存到本地存储
watch(events, (newEvents) => {
  localStorage.setItem('calendar-events', JSON.stringify(newEvents))
}, { deep: true })
</script>

<template>
  <div class="calendar-app">
    <header class="calendar-header">
      <h2 class="current-month">
        {{ weekDays[0]?.getFullYear() }}年{{ weekDays[0]?.getMonth() + 1 }}月
      </h2>
      <div class="header-controls">
        <el-button @click="prevWeek">上一周</el-button>
        <el-button @click="goToToday">本周</el-button>
        <el-button @click="nextWeek">下一周</el-button>
      </div>

    </header>

    <div class="calendar-container">
      <!-- 星期标题 -->
      <div class="week-header">
        <div class="time-header"></div>
        <div v-for="day in weekDays" :key="day.getTime()" class="day-header" :class="{ 'today': isToday(day) }">
          <div class="day-name">{{ ['周一', '周二', '周三', '周四', '周五', '周六', '周日'][day.getDay() === 0 ? 6 : day.getDay() - 1] }}</div>
          <div class="day-date">{{ formatDate(day) }}</div>
        </div>
        <div class="event-list-header">本周事件</div>
      </div>

      <!-- 日历主体 -->
      <div class="calendar-body">
        <!-- 时间轴 -->
        <div class="time-column">
          <div v-for="slot in timeSlots" :key="`${slot.hour}-${slot.half}`" class="time-label">
            {{ formatTime(slot.value) }}
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
          >
            <div class="slot-content">
              <!-- 渲染该时间段的事件 -->
              <div
                v-for="event in getEventsByDay(day).filter(e => e.startTime <= slot.value && e.endTime > slot.value)"
                :key="event.id"
                class="event-item"
                :class="{ 'event-start': Math.abs(event.startTime - slot.value) < 0.01 }"
                @click.stop="openEventDialog(event)"
              >
                <span class="event-time" v-if="Math.abs(event.startTime - slot.value) < 0.01">
                  {{ formatTime(event.startTime) }}-{{ formatTime(event.endTime) }}
                </span>
                <span class="event-title">{{ event.title }}</span>
              </div>
            </div>
          </div>
        </div>

        <!-- 本周事件列表 -->
        <div class="event-list">
          <div class="event-list-content">
            <div v-if="weekEvents.length === 0" class="no-events">暂无事件</div>
            <div v-else class="events-container">
              <div
                v-for="event in weekEvents"
                :key="event.id"
                class="event-list-item"
                @click="openEventDialog(event)"
              >
                <div class="event-item-header">
                  <span class="event-item-date">{{ event.date }}</span>
                  <span class="event-item-time">{{ formatTime(event.startTime) }}-{{ formatTime(event.endTime) }}</span>
                </div>
                <div class="event-item-title">{{ event.title }}</div>
                <div v-if="event.description" class="event-item-description">{{ event.description }}</div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- 事件编辑弹窗 -->
    <el-dialog v-model="showDialog" title="编辑事件" width="500px">
      <el-form :model="form" label-width="80px">
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
            <el-option v-for="slot in timeSlots" :key="slot.value" :label="formatTime(slot.value)" :value="slot.value" />
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
}

.week-header {
  display: flex;
  border-bottom: 1px solid #e0e0e0;
  background: #fafafa;
}

.time-header {
  width: 60px;
  min-width: 60px;
  border-right: 1px solid #e0e0e0;
}

.day-header {
  flex: 1;
  padding: 15px;
  text-align: center;
  border-right: 1px solid #e0e0e0;
}

.day-header:last-child {
  border-right: none;
}

.event-list-header {
  width: 300px;
  min-width: 300px;
  padding: 15px;
  text-align: center;
  font-size: 14px;
  font-weight: 600;
  color: #333;
  border-left: 1px solid #e0e0e0;
  background: #fafafa;
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

.calendar-body {
  flex: 1;
  display: flex;
  overflow-y: auto;
}

.time-column {
  width: 60px;
  min-width: 60px;
  border-right: 1px solid #e0e0e0;
}

.event-list {
  width: 300px;
  min-width: 300px;
  border-left: 1px solid #e0e0e0;
  display: flex;
  flex-direction: column;
  background: #fafafa;
}

.event-list-content {
  flex: 1;
  overflow-y: auto;
  padding: 10px;
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
}

.event-list-item:hover {
  transform: translateX(2px);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
}

.event-item-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
  font-size: 12px;
  color: #666;
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
  padding-top: 5px;
  font-size: 12px;
  color: #999;
  border-bottom: 1px solid #f0f0f0;
}

.day-column {
  flex: 1;
  display: flex;
  flex-direction: column;
  border-right: 1px solid #e0e0e0;
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

.event-item {
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  color: white;
  padding: 4px 8px;
  margin-bottom: 2px;
  border-radius: 4px;
  font-size: 12px;
  cursor: pointer;
  transition: transform 0.2s, box-shadow 0.2s;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.event-item:hover {
  transform: translateY(-2px);
  box-shadow: 0 4px 12px rgba(102, 126, 234, 0.4);
}

.event-start {
  border-radius: 4px 4px 0 0;
  padding-top: 6px;
}

.event-time {
  font-size: 10px;
  opacity: 0.9;
  margin-bottom: 2px;
}

.event-title {
  font-weight: 500;
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

  .event-item-header {
    color: #999;
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
</style>

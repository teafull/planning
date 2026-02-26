<template>
  <div class="month-view">
    <header class="month-header">
      <div class="month-navigation">
        <el-button @click="prevMonth" :icon="ArrowLeft" circle />
        <div class="current-month">
          <h2>{{ currentYear }}年{{ currentMonth }}月</h2>
        </div>
        <el-button @click="nextMonth" :icon="ArrowRight" circle />
      </div>
      <div class="month-actions">
        <el-button @click="goToToday" type="primary">今天</el-button>
        <el-button @click="showDatePicker">选择日期</el-button>
      </div>
    </header>

    <main class="month-calendar-container">
      <section class="month-calendar">
        <div class="weekdays">
          <div v-for="day in weekdays" :key="day" class="weekday">
            {{ day }}
          </div>
        </div>
        
        <div class="calendar-grid">
          <!-- 空白日期占位符 -->
          <div 
            v-for="n in firstDayOfWeek" 
            :key="`empty-${n}`" 
            class="calendar-day empty"
          ></div>
          
          <!-- 实际日期 -->
          <div 
            v-for="day in daysInMonth" 
            :key="day.date"
            class="calendar-day"
            :class="{
              'today': isToday(day.date),
              'other-month': day.month !== currentMonth,
              'has-events': day.events.length > 0,
              'weekend': isWeekend(day.date),
              'holiday': isHoliday(day.date)
            }"
            @click="selectDate(day)"
          >
            <div class="day-content">
              <div class="day-number">
                {{ day.day }}
                <span v-if="isHoliday(day.date)" class="holiday-tag">
                  {{ getHolidayName(day.date) }}
                </span>
              </div>
              <div v-if="day.events.length > 0" class="day-events">
                <div 
                  v-for="event in day.events.slice(0, 3)" 
                  :key="event.id"
                  class="day-event"
                  :class="`day-event-${event.type}`"
                  :title="event.title"
                >
                  <span class="event-title-text">
                  {{ getEventShortTitle(event.title) }}
                  <span v-if="!event.isAllDay" class="event-time-text">
                    {{ formatEventTime(event.startTime) }}
                  </span>
                </span>
                </div>
                <span v-if="day.events.length > 3" class="more-events">
                  +{{ day.events.length - 3 }}
                </span>
              </div>
            </div>
          </div>
        </div>
      </section>

      <!-- 右侧事件列表 -->
      <aside class="month-events-sidebar">
        <div class="sidebar-header">
          <h3>{{ currentYear }}年{{ currentMonth }}月事件</h3>
          <div class="sidebar-stats">
            <span class="total-events">共 {{ currentMonthEvents.length }} 个事件</span>
          </div>
        </div>
        
        <div class="events-list-container">
          <div v-if="currentMonthEvents.length === 0" class="no-month-events">
            <p>本月暂无事件</p>
            <el-button type="primary" size="small" @click="addEvent">
              添加事件
            </el-button>
          </div>
          
          <div v-else class="month-events-list">
            <div 
              v-for="event in currentMonthEvents" 
              :key="event.id"
              class="month-event-item"
              :class="`month-event-${event.type}`"
              @click="goToEventDate(event)"
            >
              <div class="event-date-badge">
                <div class="event-day">{{ getEventDay(event.date) }}</div>
                <div class="event-month">{{ getEventMonth(event.date) }}</div>
              </div>
              <div class="event-content">
                <div class="event-header">
                  <span class="event-type-badge">{{ getEventTypeLabel(event.type) }}</span>
                  <span class="event-time" v-if="!event.isAllDay">
                    {{ formatTime(event.startTime) }}
                  </span>
                  <span class="event-time event-allday" v-else>全天</span>
                </div>
                <div class="event-title">{{ event.title }}</div>
                <div v-if="event.description" class="event-description">
                  {{ event.description }}
                </div>
              </div>
            </div>
          </div>
        </div>
      </aside>
    </main>

    <!-- 日期选择器 -->
    <el-dialog v-model="showDateDialog" title="选择日期" width="300px">
      <el-date-picker
        v-model="selectedDate"
        type="date"
        placeholder="选择日期"
        format="YYYY-MM-DD"
        value-format="YYYY-MM-DD"
        @change="handleDateSelect"
      />
      <template #footer>
        <el-button @click="showDateDialog = false">取消</el-button>
        <el-button type="primary" @click="handleDateSelect">确定</el-button>
      </template>
    </el-dialog>

    <!-- 事件详情抽屉 -->
    <el-drawer
      v-model="showEventDrawer"
      title="事件详情"
      direction="rtl"
      size="400px"
    >
      <div v-if="selectedDay" class="selected-day-info">
        <div class="selected-date">
          <h3>{{ formatFullDate(selectedDay.date) }}</h3>
          <div v-if="isHoliday(selectedDay.date)" class="holiday-info">
            🎉 {{ getHolidayName(selectedDay.date) }}
          </div>
        </div>
        
        <div v-if="selectedDay.events.length > 0" class="events-list">
          <div 
            v-for="event in selectedDay.events" 
            :key="event.id"
            class="event-item"
            :class="`event-item-${event.type}`"
          >
            <div class="event-header">
              <span class="event-type">{{ getEventTypeLabel(event.type) }}</span>
              <span class="event-time" v-if="!event.isAllDay">
                {{ formatTime(event.startTime) }} - {{ formatTime(event.endTime) }}
              </span>
            </div>
            <div class="event-title">{{ event.title }}</div>
            <div v-if="event.description" class="event-description">
              {{ event.description }}
            </div>
          </div>
        </div>
        
        <div v-else class="no-events">
          <p>当天没有事件</p>
          <el-button type="primary" size="small" @click="addEvent">
            添加事件
          </el-button>
        </div>
      </div>
    </el-drawer>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { useRouter } from 'vue-router'
import { ArrowLeft, ArrowRight } from '@element-plus/icons-vue'
import { events } from '../store/events.js'

// 响应式数据
const currentDate = ref(new Date())
const showDateDialog = ref(false)
const selectedDate = ref(null)
const showEventDrawer = ref(false)
const selectedDay = ref(null)
const holidayData = ref({}) // 节假日数据

// 性能优化：添加计算缓存
const daysInMonthCache = ref(new Map())

// 加载本地节假日数据
const loadHolidayData = async () => {
  try {
    const raw = localStorage.getItem('holiday-data')
    if (raw) {
      const data = JSON.parse(raw)
      if (data && typeof data === 'object') {
        holidayData.value = data
        console.log('节假日数据加载成功，共', Object.keys(data).length, '条记录')
      } else {
        console.warn('节假日数据格式不正确，使用空数据')
        holidayData.value = {}
      }
    } else {
      console.log('未找到本地节假日数据')
      holidayData.value = {}
    }
  } catch (error) {
    console.warn('加载节假日数据失败:', error)
    holidayData.value = {}
  }
}

// 获取节假日相关函数
const isHoliday = (date) => {
  try {
    if (!holidayData.value || typeof holidayData.value !== 'object') return false
    const dateKey = formatDateKey(date)
    const info = holidayData.value[dateKey]
    return info && info.holiday === true
  } catch (error) {
    console.warn('检查节假日状态失败:', error)
    return false
  }
}

const getHolidayName = (date) => {
  try {
    if (!holidayData.value || typeof holidayData.value !== 'object') return ''
    const dateKey = formatDateKey(date)
    return holidayData.value[dateKey]?.name || ''
  } catch (error) {
    console.warn('获取节假日名称失败:', error)
    return ''
  }
}

const formatDateKey = (date) => {
  const m = String(date.getMonth() + 1).padStart(2, '0')
  const d = String(date.getDate()).padStart(2, '0')
  return `${m}-${d}`
}

// 星期标签
const weekdays = ['一', '二', '三', '四', '五', '六', '日']

// 计算属性
const currentYear = computed(() => currentDate.value.getFullYear())
const currentMonth = computed(() => currentDate.value.getMonth() + 1)

const firstDayOfWeek = computed(() => {
  const firstDay = new Date(currentYear.value, currentMonth.value - 1, 1)
  const dayIndex = firstDay.getDay()
  // 转换为周一开始的模式：周日(0) -> 6, 其他天数(1-6) -> dayIndex-1
  return dayIndex === 0 ? 6 : dayIndex - 1
})

const daysInMonth = computed(() => {
  const cacheKey = `${currentYear.value}-${currentMonth.value}`
  
  // 检查缓存
  if (daysInMonthCache.value.has(cacheKey)) {
    return daysInMonthCache.value.get(cacheKey)
  }
  
  const year = currentYear.value
  const month = currentMonth.value
  const days = []
  const lastDay = new Date(year, month, 0).getDate()
  
  for (let day = 1; day <= lastDay; day++) {
    const date = new Date(year, month - 1, day)
    const dayEvents = events.value.filter(event => {
      // 验证事件基本信息
      if (!event || !event.date) return false
      
      try {
        // 统一转换为Date对象进行比较
        const eventStartDate = new Date(event.date)
        const eventEndDate = event.endDate ? new Date(event.endDate) : eventStartDate
        
        // 检查是否为有效日期
        if (isNaN(eventStartDate.getTime())) return false
        if (event.endDate && isNaN(eventEndDate.getTime())) return false
        
        // 对于非跨天事件，检查日期是否完全匹配
        if (!event.endDate || event.endDate === event.date) {
          const eventDateString = eventStartDate.toISOString().split('T')[0]
          const currentDateString = date.toISOString().split('T')[0]
          return eventDateString === currentDateString
        }
        
        // 对于跨天事件，检查日期范围
        const currentDate = new Date(date)
        currentDate.setHours(0, 0, 0, 0)
        const startOfDay = new Date(eventStartDate)
        startOfDay.setHours(0, 0, 0, 0)
        const endOfDay = new Date(eventEndDate)
        endOfDay.setHours(0, 0, 0, 0)
        
        // 比较日期是否在范围内
        return currentDate >= startOfDay && currentDate <= endOfDay
      } catch (error) {
        console.warn('事件日期解析失败:', event, error)
        return false
      }
    }).sort((a, b) => {
      // 按开始时间排序
      if (a.isAllDay && !b.isAllDay) return -1
      if (!a.isAllDay && b.isAllDay) return 1
      const timeA = String(a.startTime || '00:00')
      const timeB = String(b.startTime || '00:00')
      return timeA.localeCompare(timeB)
    })
    
    days.push({
      day,
      date,
      month: month,
      year: year,
      events: dayEvents
    })
  }
  
  // 缓存结果
  daysInMonthCache.value.set(cacheKey, days)
  return days
})

// 计算当前月的事件列表
const currentMonthEvents = computed(() => {
  const year = currentYear.value
  const month = currentMonth.value
  
  return events.value.filter(event => {
    if (!event || !event.date) return false
    
    try {
      const eventDate = new Date(event.date)
      if (isNaN(eventDate.getTime())) return false
      
      return eventDate.getFullYear() === year && (eventDate.getMonth() + 1) === month
    } catch (error) {
      return false
    }
  }).sort((a, b) => {
    // 先按日期排序，再按时间排序
    const dateCompare = a.date.localeCompare(b.date)
    if (dateCompare !== 0) return dateCompare
    
    // 全天事件排前面
    if (a.isAllDay && !b.isAllDay) return -1
    if (!a.isAllDay && b.isAllDay) return 1
    
    // 按时间排序
    const timeA = String(a.startTime || '00:00')
    const timeB = String(b.startTime || '00:00')
    return timeA.localeCompare(timeB)
  })
})

// 事件类型配置
const eventTypes = [
  { value: 'task', label: '任务', color: '#667eea' },
  { value: 'meeting', label: '会议', color: '#48bb78' },
  { value: 'issue', label: '问题', color: '#f56565' },
  { value: 'reminder', label: '提醒', color: '#ed8936' },
  { value: 'other', label: '其他', color: '#718096' }
]

// 方法
const prevMonth = () => {
  currentDate.value = new Date(currentYear.value, currentMonth.value - 2, 1)
}

const nextMonth = () => {
  currentDate.value = new Date(currentYear.value, currentMonth.value, 1)
}

const goToToday = () => {
  currentDate.value = new Date()
}

const showDatePicker = () => {
  selectedDate.value = formatDateForPicker(new Date())
  showDateDialog.value = true
}

const handleDateSelect = () => {
  if (selectedDate.value) {
    currentDate.value = new Date(selectedDate.value)
    showDateDialog.value = false
  }
}

const selectDate = (day) => {
  selectedDay.value = day
  showEventDrawer.value = true
}

const addEvent = () => {
  // 跳转到日历页面添加事件
  const router = useRouter()
  router.push('/calendar')
}

const goToEventDate = (event) => {
  // 跳转到事件对应的日期
  try {
    const eventDate = new Date(event.date)
    currentDate.value = eventDate
    selectDate({
      day: eventDate.getDate(),
      date: eventDate,
      month: eventDate.getMonth() + 1,
      year: eventDate.getFullYear(),
      events: []
    })
  } catch (error) {
    console.error('跳转到事件日期失败:', error)
  }
}

const getEventDay = (dateStr) => {
  try {
    const date = new Date(dateStr)
    return date.getDate()
  } catch (error) {
    return ''
  }
}

const getEventMonth = (dateStr) => {
  try {
    const date = new Date(dateStr)
    return `${date.getMonth() + 1}月`
  } catch (error) {
    return ''
  }
}

const isToday = (date) => {
  const today = new Date()
  return date.toDateString() === today.toDateString()
}

const isWeekend = (date) => {
  const day = date.getDay()
  return day === 0 || day === 6 // 周日(0)和周六(6)仍然是周末
}

const getEventTypeLabel = (type) => {
  const eventType = eventTypes.find(t => t.value === type)
  return eventType ? eventType.label : '其他'
}

const formatTime = (hour) => {
  const hourNum = typeof hour === 'string' ? parseInt(hour) : hour
  return `${hourNum.toString().padStart(2, '0')}:00`
}

const formatFullDate = (date) => {
  return `${date.getFullYear()}年${date.getMonth() + 1}月${date.getDate()}日`
}

const formatDateForPicker = (date) => {
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}

const getEventShortTitle = (title) => {
  return title.length > 4 ? title.substring(0, 4) + '...' : title
}

const formatEventTime = (time) => {
  if (!time || typeof time !== 'string') return ''
  try {
    return time.substring(0, 5) // 只显示小时和分钟，如 "09:00"
  } catch (error) {
    console.warn('格式化事件时间失败:', error)
    return ''
  }
}

// 监听当前日期变化，自动加载相关事件
watch([currentYear, currentMonth], () => {
  // 可以在这里加载该月的事件数据
  console.log(`加载 ${currentYear.value}年${currentMonth.value}月的事件`)
})

// 监听事件变化时清除缓存
watch(events, () => {
  daysInMonthCache.value.clear()
}, { deep: true })

// 计算事件行布局，处理重叠事件的并列显示
const getEventRows = (events) => {
  const rows = []
  
  // 为每个事件添加宽度信息
  const eventsWithLayout = events.slice(0, 3).map((event) => {
    const startTime = parseTime(event.startTime)
    const endTime = parseTime(event.endTime)
    
    return {
      ...event,
      startMinutes: startTime,
      endMinutes: endTime,
      width: calculateEventWidth(startTime, endTime),
      offset: 0
    }
  })
  
  // 简单的冲突检测和布局算法
  for (let i = 0; i < eventsWithLayout.length; i++) {
    const currentEvent = eventsWithLayout[i]
    
    // 检查与前面事件的冲突
    let maxOffset = 0
    for (let j = 0; j < i; j++) {
      const prevEvent = eventsWithLayout[j]
      if (eventsOverlap(currentEvent, prevEvent)) {
        maxOffset = Math.max(maxOffset, prevEvent.offset + 25) // 每个偏移25%
      }
    }
    
    currentEvent.offset = Math.min(maxOffset, 75) // 最大偏移75%
  }
  
  // 按行分组（简化版本，按偏移量分组）
  const rowsMap = new Map()
  eventsWithLayout.forEach(event => {
    const rowKey = Math.floor(event.offset / 25)
    if (!rowsMap.has(rowKey)) {
      rowsMap.set(rowKey, [])
    }
    rowsMap.get(rowKey).push(event)
  })
  
  // 转换为数组格式
  rowsMap.forEach((events, index) => {
    rows.push({
      index,
      events: events.sort((a, b) => a.startMinutes - b.startMinutes)
    })
  })
  
  return rows.length > 0 ? rows : [{ index: 0, events: [] }]
}

// 将时间字符串转换为分钟数
const parseTime = (timeStr) => {
  if (!timeStr || typeof timeStr !== 'string') return 480 // 默认8:00
  
  try {
    const [hours, minutes] = timeStr.split(':').map(Number)
    return (hours || 8) * 60 + (minutes || 0)
  } catch (error) {
    return 480 // 默认8:00
  }
}

// 计算事件宽度（百分比）
const calculateEventWidth = (startMinutes, endMinutes) => {
  const dayStart = 480 // 8:00
  const dayEnd = 1080 // 18:00
  const dayDuration = dayEnd - dayStart
  
  const eventStart = Math.max(startMinutes, dayStart)
  const eventEnd = Math.min(endMinutes, dayStart + dayDuration)
  
  const eventDuration = Math.max(eventEnd - eventStart, 60) // 最小1小时
  return Math.min(100, (eventDuration / dayDuration) * 100)
}

// 检查两个事件是否重叠
const eventsOverlap = (event1, event2) => {
  return event1.startMinutes < event2.endMinutes && event2.startMinutes < event1.endMinutes
}

// 调试函数：检查事件重复
const debugEventDuplicates = () => {
  const eventsByDate = new Map()
  
  events.value.forEach(event => {
    const dateKey = event.date || 'unknown'
    if (!eventsByDate.has(dateKey)) {
      eventsByDate.set(dateKey, [])
    }
    eventsByDate.get(dateKey).push(event)
  })
  
  // 检查每个日期是否有重复事件
  eventsByDate.forEach((dateEvents, date) => {
    const eventKeys = dateEvents.map(e => `${e.id || 'no-id'}-${e.title}-${e.startTime}`)
    const uniqueKeys = new Set(eventKeys)
    
    if (eventKeys.length !== uniqueKeys.size) {
      console.warn(`日期 ${date} 发现重复事件:`, dateEvents)
    }
  })
  
  console.log('事件统计:', {
    总数: events.value.length,
    按日期分组: Array.from(eventsByDate.entries()).map(([date, evs]) => ({
      日期: date,
      数量: evs.length
    }))
  })
}

// 组件挂载
onMounted(() => {
  loadHolidayData()
  debugEventDuplicates()
  console.log('月视图组件已挂载')
})
</script>

<style scoped>
.month-view {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: #f8fafc;
}

.month-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 20px;
  background: white;
  border-bottom: 1px solid #e2e8f0;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
}

.month-navigation {
  display: flex;
  align-items: center;
  gap: 16px;
}

.current-month h2 {
  margin: 0;
  font-size: 20px;
  font-weight: 600;
  color: #1f2937;
}

.month-actions {
  display: flex;
  gap: 12px;
}

.month-calendar-container {
  flex: 1;
  display: flex;
  gap: 20px;
  padding: 20px;
  overflow: hidden;
}

.month-calendar {
  flex: 1;
  overflow: hidden;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.weekdays {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  gap: 1px;
  margin-bottom: 1px;
}

.weekday {
  padding: 10px;
  text-align: center;
  font-weight: 600;
  color: #6b7280;
  background: #f1f5f9;
  border-radius: 8px 8px 0 0;
}

.calendar-grid {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  grid-template-rows: repeat(6, 1fr);
  gap: 1px;
  background: #e2e8f0;
  border-radius: 0 0 8px 8px;
  overflow: hidden;
  flex: 1;
}

.calendar-day {
  height: calc((100vh - 200px) / 6);
  min-height: 80px;
  padding: 6px;
  background: white;
  cursor: pointer;
  position: relative;
  display: flex;
  flex-direction: column;
  transition: all 0.2s ease;
}

.calendar-day:hover {
  background: #f0f9ff;
  transform: scale(1.02);
  z-index: 1;
}

.calendar-day.today {
  background: #eff6ff;
  border: 2px solid #3b82f6;
}

.calendar-day.other-month {
  background: #f9fafb;
  color: #9ca3af;
  opacity: 0.6;
}

.calendar-day.weekend {
  background: #fef2f2;
}

.calendar-day.has-events {
  background: #f0fdf4;
}

.calendar-day.holiday {
  background: #fef2f2;
  border: 1px solid #fca5a5;
}

.day-content {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.day-number {
  font-weight: 600;
  font-size: 15px;
  margin-bottom: 6px;
  display: flex;
  align-items: center;
  gap: 5px;
  flex-wrap: wrap;
}

.day-events {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin-bottom: 6px;
}

.day-event {
  padding: 2px 6px;
  border-radius: 3px;
  font-size: 10px;
  line-height: 1.3;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 100%;
  display: flex;
  align-items: center;
  gap: 3px;
  margin-bottom: 1px;
}

.day-event-task { 
  background: rgba(102, 126, 234, 0.9); 
  color: white;
}

.day-event-meeting { 
  background: rgba(72, 187, 120, 0.9); 
  color: white;
}

.day-event-issue { 
  background: rgba(245, 101, 101, 0.9); 
  color: white;
}

.day-event-reminder { 
  background: rgba(237, 137, 54, 0.9); 
  color: white;
}

.day-event-other { 
  background: rgba(113, 128, 150, 0.9); 
  color: white;
}

.event-title-text {
  font-weight: 500;
  flex: 1;
  font-size: 10px;
}

.event-time-text {
  font-size: 9px;
  opacity: 0.8;
  font-weight: 400;
}

.more-events {
  font-size: 10px;
  color: #6b7280;
  font-weight: 600;
  background: #f3f4f6;
  border-radius: 3px;
  padding: 2px 6px;
  align-self: flex-start;
}

.holiday-tag {
  font-size: 10px;
  background: #fef2f2;
  border: 1px solid #fca5a5;
  border-radius: 4px;
  padding: 2px 4px;
  color: #dc2626;
  font-weight: 600;
  white-space: nowrap;
  order: 1;
}

.selected-day-info {
  padding: 20px;
}

.selected-date {
  margin-bottom: 20px;
  padding-bottom: 16px;
  border-bottom: 1px solid #e5e7eb;
}

.selected-date h3 {
  margin: 0 0 8px;
  color: #1f2937;
}

.holiday-info {
  color: #dc2626;
  font-weight: 600;
  font-size: 14px;
}

.events-list {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.event-item {
  padding: 16px;
  border-radius: 8px;
  border-left: 4px solid #667eea;
  background: #f8fafc;
}

.event-item-meeting { border-left-color: #48bb78; }
.event-item-issue { border-left-color: #f56565; }
.event-item-reminder { border-left-color: #ed8936; }
.event-item-other { border-left-color: #718096; }

.event-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
}

.event-type {
  padding: 2px 8px;
  border-radius: 4px;
  font-size: 12px;
  font-weight: 600;
  background: #667eea;
  color: white;
}

.event-item-meeting .event-type { background: #48bb78; }
.event-item-issue .event-type { background: #f56565; }
.event-item-reminder .event-type { background: #ed8936; }
.event-item-other .event-type { background: #718096; }

.event-time {
  font-size: 12px;
  color: #6b7280;
}

.event-title {
  font-weight: 600;
  color: #1f2937;
  margin-bottom: 4px;
}

.event-description {
  font-size: 14px;
  color: #6b7280;
  line-height: 1.5;
}

.no-events {
  text-align: center;
  padding: 40px 20px;
  color: #6b7280;
}

.no-events p {
  margin: 0 0 16px;
  font-size: 16px;
}

.month-events-sidebar {
  width: 320px;
  flex: 0 0 320px;
  background: white;
  border-radius: 8px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.sidebar-header {
  padding: 20px;
  border-bottom: 1px solid #e5e7eb;
  background: #f9fafb;
}

.sidebar-header h3 {
  margin: 0 0 8px 0;
  font-size: 16px;
  font-weight: 600;
  color: #1f2937;
}

.sidebar-stats {
  display: flex;
  align-items: center;
  gap: 12px;
}

.total-events {
  font-size: 14px;
  color: #6b7280;
  font-weight: 500;
}

.events-list-container {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
}

.no-month-events {
  text-align: center;
  padding: 40px 20px;
  color: #6b7280;
}

.no-month-events p {
  margin: 0 0 16px 0;
  font-size: 16px;
}

.month-events-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.month-event-item {
  display: flex;
  gap: 12px;
  padding: 12px;
  border-radius: 8px;
  border-left: 4px solid #667eea;
  background: #f8fafc;
  cursor: pointer;
  transition: all 0.2s ease;
}

.month-event-item:hover {
  transform: translateY(-1px);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
}

.month-event-meeting { border-left-color: #48bb78; }
.month-event-issue { border-left-color: #f56565; }
.month-event-reminder { border-left-color: #ed8936; }
.month-event-other { border-left-color: #718096; }

.event-date-badge {
  flex: 0 0 auto;
  width: 48px;
  height: 48px;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  border-radius: 8px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  color: white;
  font-weight: 600;
}

.month-event-meeting .event-date-badge {
  background: linear-gradient(135deg, #48bb78 0%, #38a169 100%);
}

.month-event-issue .event-date-badge {
  background: linear-gradient(135deg, #f56565 0%, #e53e3e 100%);
}

.month-event-reminder .event-date-badge {
  background: linear-gradient(135deg, #ed8936 0%, #dd6b20 100%);
}

.month-event-other .event-date-badge {
  background: linear-gradient(135deg, #718096 0%, #4a5568 100%);
}

.event-day {
  font-size: 18px;
  line-height: 1;
  margin-bottom: 2px;
}

.event-month {
  font-size: 10px;
  line-height: 1;
  opacity: 0.9;
}

.event-content {
  flex: 1;
  min-width: 0;
}

.month-event-item .event-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 6px;
}

.event-type-badge {
  padding: 2px 8px;
  border-radius: 4px;
  font-size: 10px;
  font-weight: 600;
  background: #667eea;
  color: white;
}

.month-event-meeting .event-type-badge { background: #48bb78; }
.month-event-issue .event-type-badge { background: #f56565; }
.month-event-reminder .event-type-badge { background: #ed8936; }
.month-event-other .event-type-badge { background: #718096; }

.month-event-item .event-time {
  font-size: 12px;
  color: #6b7280;
  font-weight: 500;
}

.month-event-item .event-time.event-allday {
  color: #10b981;
}

.month-event-item .event-title {
  font-weight: 600;
  color: #1f2937;
  font-size: 14px;
  margin-bottom: 4px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.month-event-item .event-description {
  font-size: 12px;
  color: #6b7280;
  line-height: 1.4;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

/* 响应式设计 */
@media (max-width: 1200px) {
  .month-events-sidebar {
    width: 280px;
    flex: 0 0 280px;
  }
  
  .event-date-badge {
    width: 40px;
    height: 40px;
  }
  
  .event-day {
    font-size: 16px;
  }
}

@media (max-width: 768px) {
  .month-calendar-container {
    flex-direction: column;
    gap: 16px;
  }
  
  .month-events-sidebar {
    width: 100%;
    flex: none;
    max-height: 300px;
  }
  
  .month-header {
    flex-direction: column;
    gap: 16px;
    align-items: stretch;
  }
  
  .month-navigation {
    justify-content: center;
  }
  
  .month-actions {
    justify-content: center;
  }
  
  .calendar-day {
    height: calc((100vh - 250px) / 6);
    min-height: 60px;
    padding: 4px;
  }
  
  .day-number {
    font-size: 12px;
  }
  
  .day-event {
    font-size: 10px;
    padding: 2px 6px;
  }
  
  .holiday-tag {
    font-size: 8px;
  }
  
  .month-event-item {
    padding: 8px;
  }
  
  .event-date-badge {
    width: 36px;
    height: 36px;
  }
  
  .event-day {
    font-size: 14px;
  }
}

@media (max-width: 480px) {
  .calendar-day {
    height: calc((100vh - 280px) / 6);
    min-height: 50px;
    padding: 2px;
  }
  
  .day-number {
    font-size: 11px;
  }
  
  .day-event {
    font-size: 8px;
    padding: 1px 3px;
  }
  
  .event-title-text {
    display: none; /* 超小屏幕隐藏事件标题，只显示时间 */
  }
  
  .day-event {
    justify-content: center;
  }
  
  .holiday-tag {
    font-size: 7px;
    padding: 0 2px;
  }
}
</style>
<script setup>
import { ref, computed, onMounted } from 'vue'
import { events, loadEvents } from '../store/events'

const range = ref('近7天')
const dateRange = ref([])



const typeConfig = [
  { value: 'task', label: '任务', color: '#6366f1' },
  { value: 'meeting', label: '会议', color: '#22c55e' },
  { value: 'reminder', label: '提醒', color: '#f59e0b' },
  { value: 'issue', label: '问题', color: '#ef4444' },
  { value: 'other', label: '其他', color: '#94a3b8' }
]

onMounted(() => {
  loadEvents()
})


const startOfDay = (date) => new Date(date.getFullYear(), date.getMonth(), date.getDate())

const parseDate = (value) => {
  if (!value) return null
  const [year, month, day] = value.split('-').map(Number)
  if (!year || !month || !day) return null
  return new Date(year, month - 1, day)
}

const getEventRange = (event) => {
  const start = parseDate(event.date)
  const end = parseDate(event.endDate || event.date)
  return { start, end }
}

const rangeInfo = computed(() => {
  const today = startOfDay(new Date())
  if (Array.isArray(dateRange.value) && dateRange.value.length === 2 && dateRange.value[0] && dateRange.value[1]) {
    const first = parseDate(dateRange.value[0])
    const second = parseDate(dateRange.value[1])
    if (first && second) {
      const start = first <= second ? first : second
      const end = first <= second ? second : first
      const days = Math.floor((end - start) / 86400000) + 1
      return { start, end, days }
    }
  }
  if (range.value === '今天') {
    return { start: today, end: today, days: 1 }
  }

  if (range.value === '本月') {
    const start = new Date(today.getFullYear(), today.getMonth(), 1)
    const end = new Date(today.getFullYear(), today.getMonth() + 1, 0)
    const days = Math.floor((end - start) / 86400000) + 1
    return { start, end, days }
  }
  if (range.value === '本季度') {
    const quarterStartMonth = Math.floor(today.getMonth() / 3) * 3
    const start = new Date(today.getFullYear(), quarterStartMonth, 1)
    const end = new Date(today.getFullYear(), quarterStartMonth + 3, 0)
    const days = Math.floor((end - start) / 86400000) + 1
    return { start, end, days }
  }
  if (range.value === '本半年') {
    const halfStartMonth = today.getMonth() < 6 ? 0 : 6
    const start = new Date(today.getFullYear(), halfStartMonth, 1)
    const end = new Date(today.getFullYear(), halfStartMonth + 6, 0)
    const days = Math.floor((end - start) / 86400000) + 1
    return { start, end, days }
  }
  if (range.value === '本年') {
    const start = new Date(today.getFullYear(), 0, 1)
    const end = new Date(today.getFullYear(), 11, 31)
    const days = Math.floor((end - start) / 86400000) + 1
    return { start, end, days }
  }
  const days = range.value === '近30天' ? 30 : 7
  const start = new Date(today)
  start.setDate(today.getDate() - (days - 1))
  return { start, end: today, days }
})

const previousRangeInfo = computed(() => {
  const { start, end, days } = rangeInfo.value
  if (Array.isArray(dateRange.value) && dateRange.value.length === 2 && dateRange.value[0] && dateRange.value[1]) {
    const prevStart = new Date(start)
    prevStart.setDate(start.getDate() - days)
    const prevEnd = new Date(end)
    prevEnd.setDate(end.getDate() - days)
    return { start: prevStart, end: prevEnd }
  }
  if (range.value === '今天') {
    const prevStart = new Date(start)
    prevStart.setDate(start.getDate() - 1)
    return { start: prevStart, end: prevStart }
  }

  if (range.value === '本月') {
    const prevStart = new Date(start.getFullYear(), start.getMonth() - 1, 1)
    const prevEnd = new Date(start.getFullYear(), start.getMonth(), 0)
    return { start: prevStart, end: prevEnd }
  }
  if (range.value === '本季度') {
    const prevStart = new Date(start.getFullYear(), start.getMonth() - 3, 1)
    const prevEnd = new Date(start.getFullYear(), start.getMonth(), 0)
    return { start: prevStart, end: prevEnd }
  }
  if (range.value === '本半年') {
    const prevStart = new Date(start.getFullYear(), start.getMonth() - 6, 1)
    const prevEnd = new Date(start.getFullYear(), start.getMonth(), 0)
    return { start: prevStart, end: prevEnd }
  }
  if (range.value === '本年') {
    const prevStart = new Date(start.getFullYear() - 1, 0, 1)
    const prevEnd = new Date(start.getFullYear() - 1, 11, 31)
    return { start: prevStart, end: prevEnd }
  }
  const prevStart = new Date(start)
  prevStart.setDate(start.getDate() - days)
  const prevEnd = new Date(end)
  prevEnd.setDate(end.getDate() - days)
  return { start: prevStart, end: prevEnd }
})



const isInRange = (event, rangeStart, rangeEnd) => {
  const { start, end } = getEventRange(event)
  if (!start || !end) return false
  return end >= rangeStart && start <= rangeEnd
}

const eventsInRange = computed(() => {
  const { start, end } = rangeInfo.value
  return events.value.filter(event => isInRange(event, start, end))
})

const getTypeLabel = (value) => {
  const match = typeConfig.find(item => item.value === value)
  return match ? match.label : '其他'
}

const getStatus = (event) => {
  const today = startOfDay(new Date())
  const todayEnd = new Date(today)
  todayEnd.setDate(todayEnd.getDate() + 1)
  todayEnd.setMilliseconds(todayEnd.getMilliseconds() - 1)
  const { start, end } = getEventRange(event)
  if (!start || !end) return '进行中'
  if (end < today) return event.type === 'task' ? '逾期' : '已完成'
  if (start > todayEnd) return '待开始'
  return '进行中'
}

const formatDuration = (event) => {
  if (event.isAllDay) {
    if (event.endDate && event.endDate !== event.date) {
      const start = parseDate(event.date)
      const end = parseDate(event.endDate)
      if (start && end) {
        const days = Math.floor((end - start) / 86400000) + 1
        return `${days}天`
      }
    }
    return '全天'
  }
  const duration = Math.max(0, Number(event.endTime || 0) - Number(event.startTime || 0))
  if (duration < 1) return `${Math.round(duration * 60)}m`
  const fixed = duration % 1 === 0 ? duration.toFixed(0) : duration.toFixed(1)
  return `${fixed}h`
}

const focusHours = computed(() => {
  const labels = ['周一', '周二', '周三', '周四', '周五', '周六', '周日']
  const data = labels.map(label => ({ day: label, hours: 0 }))

  eventsInRange.value.forEach(event => {
    if (event.isAllDay) return
    if (typeof event.startTime !== 'number' || typeof event.endTime !== 'number') return
    const { start } = getEventRange(event)
    if (!start) return
    const index = (start.getDay() + 6) % 7
    const duration = Math.max(0, event.endTime - event.startTime)
    data[index].hours += duration
  })

  return data.map(item => ({ ...item, hours: Number(item.hours.toFixed(1)) }))
})

const typeDistribution = computed(() => {
  const total = eventsInRange.value.length
  const counts = typeConfig.reduce((acc, item) => ({ ...acc, [item.value]: 0 }), {})

  eventsInRange.value.forEach(event => {
    const key = typeConfig.some(item => item.value === event.type) ? event.type : 'other'
    counts[key] += 1
  })

  let accumulated = 0
  return typeConfig.map((item, index) => {
    let percent = total ? Math.round((counts[item.value] / total) * 100) : 0
    if (index === typeConfig.length - 1) {
      percent = Math.max(0, 100 - accumulated)
    } else {
      accumulated += percent
    }
    return { type: item.label, percent, color: item.color }
  })
})

const recentItems = computed(() => {
  const recentEnd = startOfDay(new Date())
  const recentStart = new Date(recentEnd)
  recentStart.setDate(recentEnd.getDate() - 2)

  return [...events.value]
    .filter(event => isInRange(event, recentStart, recentEnd))
    .sort((a, b) => {
      const aDate = parseDate(a.date) || new Date(0)
      const bDate = parseDate(b.date) || new Date(0)
      if (bDate.getTime() !== aDate.getTime()) return bDate - aDate
      return (b.startTime || 0) - (a.startTime || 0)
    })
    .map(event => ({
      title: event.title || '未命名事件',
      type: getTypeLabel(event.type),
      status: getStatus(event),
      date: event.date || '--',
      duration: formatDuration(event)
    }))
})


const countStats = (rangeStart, rangeEnd) => {
  const filtered = events.value.filter(event => isInRange(event, rangeStart, rangeEnd))
  const statusCounts = { completed: 0, ongoing: 0, overdue: 0 }

  filtered.forEach(event => {
    const status = getStatus(event)
    if (status === '已完成') statusCounts.completed += 1
    if (status === '逾期') statusCounts.overdue += 1
    if (status === '进行中' || status === '待开始') statusCounts.ongoing += 1
  })

  return {
    total: filtered.length,
    completed: statusCounts.completed,
    ongoing: statusCounts.ongoing,
    overdue: statusCounts.overdue
  }
}

const formatChange = (current, previous) => {
  if (!previous && current === 0) return '0%'
  if (!previous) return '+100%'
  const diff = ((current - previous) / previous) * 100
  const sign = diff > 0 ? '+' : ''
  return `${sign}${Math.round(diff)}%`
}

const summaryCards = computed(() => {
  const current = countStats(rangeInfo.value.start, rangeInfo.value.end)
  const previous = countStats(previousRangeInfo.value.start, previousRangeInfo.value.end)

  return [
    { title: '总事件数', value: current.total, change: formatChange(current.total, previous.total), color: '#4f46e5' },
    { title: '已完成', value: current.completed, change: formatChange(current.completed, previous.completed), color: '#10b981' },
    { title: '进行中', value: current.ongoing, change: formatChange(current.ongoing, previous.ongoing), color: '#f59e0b' },
    { title: '逾期', value: current.overdue, change: formatChange(current.overdue, previous.overdue), color: '#ef4444' }
  ]
})

const efficiencyStats = computed(() => {
  const total = eventsInRange.value.length
  const pastEvents = eventsInRange.value.filter(event => {
    const { end } = getEventRange(event)
    return end && end < startOfDay(new Date())
  })
  const overdueCount = pastEvents.filter(event => getStatus(event) === '逾期').length
  const completedCount = pastEvents.filter(event => getStatus(event) === '已完成').length
  const completionRate = total ? Math.round((completedCount / total) * 100) : 0
  const onTimeRate = pastEvents.length ? Math.round(((pastEvents.length - overdueCount) / pastEvents.length) * 100) : 100

  const totalFocus = focusHours.value.reduce((sum, item) => sum + item.hours, 0)
  const focusRate = rangeInfo.value.days ? Math.min(100, Math.round((totalFocus / (rangeInfo.value.days * 8)) * 100)) : 0

  return { completionRate, onTimeRate, focusRate }
})

const efficiencyTip = computed(() => {
  const maxDay = focusHours.value.reduce((prev, current) => (current.hours > prev.hours ? current : prev), focusHours.value[0])
  if (!maxDay) return '建议：将高优先级任务安排在上午 9:00-11:00。'
  return `建议：你的高效日为${maxDay.day}，将重要任务放在该日高峰时段更容易完成。`
})
</script>


<template>
  <div class="stats-page">
    <header class="stats-header">
      <div>
        <h2 class="stats-title">统计面板</h2>
        <p class="stats-subtitle">聚合你的一周节奏、完成效率与事件分布。</p>
      </div>
      <div class="stats-actions">
        <el-date-picker
          v-model="dateRange"
          type="daterange"
          size="small"
          class="stats-date"
          unlink-panels
          range-separator="至"
          start-placeholder="开始日期"
          end-placeholder="结束日期"
          value-format="YYYY-MM-DD"
          clearable
        />
        <el-select v-model="range" size="small" class="stats-select">
          <el-option label="今天" value="今天" />
          <el-option label="近7天" value="近7天" />
          <el-option label="近30天" value="近30天" />
          <el-option label="本月" value="本月" />
          <el-option label="本季度" value="本季度" />
          <el-option label="本半年" value="本半年" />
          <el-option label="本年" value="本年" />
        </el-select>
        <el-button size="small" type="primary">导出报表</el-button>
      </div>

    </header>

    <section class="stats-grid">
      <el-card v-for="card in summaryCards" :key="card.title" class="summary-card" shadow="never">
        <div class="summary-card__header">
          <span class="summary-card__title">{{ card.title }}</span>
          <span class="summary-card__change" :style="{ color: card.color }">{{ card.change }}</span>
        </div>
        <div class="summary-card__value">{{ card.value }}</div>
        <div class="summary-card__hint">较上周期</div>
        <div class="summary-card__bar" :style="{ background: card.color }"></div>
      </el-card>
    </section>

    <section class="stats-content">
      <el-card class="panel" shadow="never">
        <template #header>
          <div class="panel-title">专注时长（小时）</div>
        </template>
        <div class="bar-chart">
          <div v-for="item in focusHours" :key="item.day" class="bar-item">
            <div class="bar-item__label">{{ item.day }}</div>
            <div class="bar-item__track">
              <div class="bar-item__bar" :style="{ width: `${(item.hours / 8) * 100}%` }"></div>
            </div>
            <div class="bar-item__value">{{ item.hours }}h</div>
          </div>
        </div>
      </el-card>

      <el-card class="panel" shadow="never">
        <template #header>
          <div class="panel-title">事件类型分布</div>
        </template>
        <div class="distribution">
          <div v-for="item in typeDistribution" :key="item.type" class="distribution-item">
            <div class="distribution-item__label">
              <span>{{ item.type }}</span>
              <span>{{ item.percent }}%</span>
            </div>
            <el-progress :percentage="item.percent" :stroke-width="10" :color="item.color" />
          </div>
        </div>
      </el-card>

      <el-card class="panel" shadow="never">
        <template #header>
          <div class="panel-title">效率概览</div>
        </template>
        <div class="efficiency">
          <div class="efficiency-item">
            <span>完成率</span>
            <el-progress :percentage="efficiencyStats.completionRate" :stroke-width="12" />
          </div>
          <div class="efficiency-item">
            <span>准时率</span>
            <el-progress :percentage="efficiencyStats.onTimeRate" :stroke-width="12" color="#10b981" />
          </div>
          <div class="efficiency-item">
            <span>专注占比</span>
            <el-progress :percentage="efficiencyStats.focusRate" :stroke-width="12" color="#6366f1" />
          </div>
          <div class="efficiency-tip">
            {{ efficiencyTip }}
          </div>
        </div>
      </el-card>

      <el-card class="panel panel--wide" shadow="never">
        <template #header>
          <div class="panel-title">最近事件</div>
        </template>
        <el-table :data="recentItems" class="stats-table" stripe>
          <el-table-column prop="title" label="事件" min-width="180" />
          <el-table-column prop="type" label="类型" width="90" />
          <el-table-column prop="status" label="状态" width="90">
            <template #default="scope">
              <el-tag
                :type="scope.row.status === '已完成' ? 'success' : scope.row.status === '逾期' ? 'danger' : scope.row.status === '待开始' ? 'info' : 'warning'"
                size="small"
              >
                {{ scope.row.status }}
              </el-tag>
            </template>
          </el-table-column>

          <el-table-column prop="date" label="日期" width="110" />
          <el-table-column prop="duration" label="耗时" width="90" />
        </el-table>
      </el-card>
    </section>
  </div>
</template>

<style scoped>
.stats-page {
  display: flex;
  flex-direction: column;
  gap: 20px;
  padding: 20px;
  height: 100%;
  overflow-y: auto;
  box-sizing: border-box;
  scrollbar-gutter: stable;
}


.stats-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.stats-title {
  margin: 0 0 6px;
  font-size: 22px;
  font-weight: 600;
  color: #1f2937;
}

.stats-subtitle {
  margin: 0;
  color: #6b7280;
  font-size: 14px;
}

.stats-actions {
  display: flex;
  gap: 8px;
  align-items: center;
  flex-wrap: wrap;
}

.stats-date {
  width: 240px;
}

.stats-select {
  width: 120px;
}


.stats-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
  gap: 16px;
}

.summary-card {
  position: relative;
  border-radius: 14px;
  border: 1px solid #eef2ff;
}

.summary-card__header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
}

.summary-card__title {
  font-size: 14px;
  color: #6b7280;
}

.summary-card__change {
  font-size: 12px;
  font-weight: 600;
}

.summary-card__value {
  font-size: 28px;
  font-weight: 600;
  color: #111827;
}

.summary-card__hint {
  font-size: 12px;
  color: #9ca3af;
}

.summary-card__bar {
  margin-top: 12px;
  height: 4px;
  border-radius: 999px;
  opacity: 0.7;
}

.stats-content {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 16px;
}

.panel {
  border-radius: 14px;
  border: 1px solid #eef2ff;
}

.panel--wide {
  grid-column: 1 / -1;
}

.panel-title {
  font-size: 15px;
  font-weight: 600;
  color: #111827;
}

.bar-chart {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.bar-item {
  display: grid;
  grid-template-columns: 60px 1fr 50px;
  align-items: center;
  gap: 10px;
}

.bar-item__label {
  font-size: 13px;
  color: #6b7280;
}

.bar-item__track {
  height: 10px;
  border-radius: 999px;
  background: #eef2ff;
  overflow: hidden;
}

.bar-item__bar {
  height: 100%;
  background: linear-gradient(90deg, #6366f1, #8b5cf6);
  border-radius: inherit;
}

.bar-item__value {
  font-size: 12px;
  color: #6b7280;
  text-align: right;
}

.distribution {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.distribution-item__label {
  display: flex;
  justify-content: space-between;
  font-size: 13px;
  color: #6b7280;
  margin-bottom: 6px;
}

.efficiency {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.efficiency-item span {
  font-size: 13px;
  color: #6b7280;
  display: inline-block;
  margin-bottom: 6px;
}

.efficiency-tip {
  background: #eef2ff;
  color: #4f46e5;
  padding: 10px 12px;
  border-radius: 10px;
  font-size: 12px;
}

.stats-table {
  width: 100%;
}

/* 滚动条样式 - 仅在滚动时显示，覆盖式不挤压内容 */
.stats-page::-webkit-scrollbar {
  width: 8px;
}

.stats-page::-webkit-scrollbar-track {
  background: transparent;
}

.stats-page::-webkit-scrollbar-thumb {
  background: transparent;
  border-radius: 4px;
}

.stats-page:hover::-webkit-scrollbar-thumb {
  background: rgba(0, 0, 0, 0.2);
}

.stats-page::-webkit-scrollbar-thumb:hover {
  background: rgba(0, 0, 0, 0.3);
}

/* Firefox 滚动条样式 */
.stats-page {
  scrollbar-width: thin;
  scrollbar-color: transparent transparent;
}

.stats-page:hover {
  scrollbar-color: rgba(0, 0, 0, 0.35) transparent;
}

@media (max-width: 1024px) {

  .stats-content {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 768px) {
  .stats-header {
    flex-direction: column;
    align-items: flex-start;
  }

  .stats-grid {
    grid-template-columns: 1fr;
  }
}
</style>


<script setup>
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { ElMessage } from 'element-plus'
import { events, loadEvents } from '../store/events'
import { initRaftListener } from '../store/events'
import * as echarts from 'echarts'
import {
  startOfDay, parseDate, formatYmd, isInRange, isEventOnDay,
  dailyDistribution, hourlyDensity, weekdayDistribution, typeBreakdown, generateInsights
} from '../utils/statsAnalysis'
import { getEventType } from '../utils/eventTypes'

// ---------- 状态 ----------
const range = ref('近30天')
const dateRange = ref([])
const selectedType = ref(null) // 类型筛选(null=全部)
const reportText = ref('')
let charts = {}
let raftUnlisten = null

onMounted(async () => {
  loadEvents()
  await nextTick()
  initCharts()
  raftUnlisten = await initRaftListener()
  window.addEventListener('resize', resizeCharts)
})

onUnmounted(() => {
  Object.values(charts).forEach(c => c?.dispose())
  window.removeEventListener('resize', resizeCharts)
  if (raftUnlisten) raftUnlisten()
})

// ---------- 范围计算 ----------
const rangeInfo = computed(() => {
  const today = startOfDay(new Date())
  if (Array.isArray(dateRange.value) && dateRange.value.length === 2 && dateRange.value[0] && dateRange.value[1]) {
    const a = parseDate(dateRange.value[0])
    const b = parseDate(dateRange.value[1])
    if (a && b) {
      const start = a <= b ? a : b
      const end = a <= b ? b : a
      return { start, end, days: Math.round((end - start) / 86400000) + 1 }
    }
  }
  const map = {
    '今天': [today, today], '近7天': [addDays(today, -6), today],
    '近30天': [addDays(today, -29), today], '本月': [new Date(today.getFullYear(), today.getMonth(), 1), today],
    '本季度': [new Date(today.getFullYear(), Math.floor(today.getMonth() / 3) * 3, 1), today],
    '本半年': [new Date(today.getFullYear(), today.getMonth() >= 6 ? 6 : 0, 1), today],
    '本年': [new Date(today.getFullYear(), 0, 1), today],
  }
  const [start, end] = map[range.value] || map['近30天']
  return { start, end, days: Math.round((end - start) / 86400000) + 1 }
})

const previousRangeInfo = computed(() => {
  const { start, end, days } = rangeInfo.value
  const prevEnd = addDays(start, -1)
  const prevStart = addDays(prevEnd, -(days - 1))
  return { start: prevStart, end: prevEnd, days }
})

function addDays(d, n) { const r = new Date(d); r.setDate(d.getDate() + n); return r }

// ---------- 数据计算 ----------
const eventsInRange = computed(() => {
  const { start, end } = rangeInfo.value
  let list = events.value.filter(ev => isInRange(ev, start, end))
  if (selectedType.value) list = list.filter(ev => ev.type === selectedType.value)
  return list
})

const allEventsInPeriod = computed(() => {
  const { start, end } = rangeInfo.value
  return events.value.filter(ev => isInRange(ev, start, end))
})

const rangeDays = computed(() => {
  const { start, end } = rangeInfo.value
  const days = []
  const d = new Date(start)
  while (d <= end) { days.push(new Date(d)); d.setDate(d.getDate() + 1) }
  return days
})

const daily = computed(() => dailyDistribution(allEventsInPeriod.value, rangeDays.value))
const hourly = computed(() => hourlyDensity(allEventsInPeriod.value))
const weekday = computed(() => weekdayDistribution(allEventsInPeriod.value, rangeDays.value))
const typeDist = computed(() => typeBreakdown(allEventsInPeriod.value))

// ---------- 摘要卡片 ----------
const totalHours = computed(() => daily.value.reduce((s, d) => s + d.hours, 0))
const busiestDay = computed(() => daily.value.reduce((p, c) => c.count > p.count ? c : p, daily.value[0] || { count: 0 }))
const topType = computed(() => typeDist.value[0] || { label: '-', percent: 0 })

const prevStats = computed(() => {
  const { start, end } = previousRangeInfo.value
  const prevEvents = events.value.filter(ev => isInRange(ev, start, end))
  const prevDays = []
  const d = new Date(start)
  while (d <= end) { prevDays.push(new Date(d)); d.setDate(d.getDate() + 1) }
  const prevDaily = dailyDistribution(prevEvents, prevDays)
  const prevHours = prevDaily.reduce((s, x) => s + x.hours, 0)
  return { count: prevEvents.length, hours: prevHours }
})

const fmtChange = (cur, prev) => {
  if (!prev && cur === 0) return '0%'
  if (!prev) return '+100%'
  const pct = Math.round(((cur - prev) / prev) * 100)
  return `${pct > 0 ? '+' : ''}${pct}%`
}

const summaryCards = computed(() => [
  { title: '总事件', value: allEventsInPeriod.value.length, change: fmtChange(allEventsInPeriod.value.length, prevStats.value.count), color: '#4f46e5', icon: '📋' },
  { title: '总时长', value: `${Math.round(totalHours.value * 10) / 10}h`, change: fmtChange(totalHours.value, prevStats.value.hours), color: '#0ea5e9', icon: '⏱️' },
  { title: '日均事件', value: rangeInfo.value.days ? (allEventsInPeriod.value.length / rangeInfo.value.days).toFixed(1) : '0', change: '', color: '#8b5cf6', icon: '📊' },
  { title: '最忙日', value: busiestDay.value?.count ? `${busiestDay.value.date.getMonth() + 1}/${busiestDay.value.date.getDate()} (${busiestDay.value.count}件)` : '-', change: '', color: '#f59e0b', icon: '🔥' },
  { title: '主要类型', value: topType.value.label, change: `${topType.value.percent}%`, color: topType.value.color, icon: '🏷️' },
])

// ---------- 智能洞察 ----------
const insights = computed(() => generateInsights(allEventsInPeriod.value, rangeDays.value, hourly.value, weekday.value, typeDist.value))

// ---------- ECharts ----------
function initCharts() {
  const dark = document.documentElement.classList.contains('dark')
  const theme = dark ? 'dark' : ''

  charts.trend = echarts.init(document.getElementById('chart-trend'), theme)
  charts.pie = echarts.init(document.getElementById('chart-pie'), theme)
  charts.hourly = echarts.init(document.getElementById('chart-hourly'), theme)
  charts.weekday = echarts.init(document.getElementById('chart-weekday'), theme)

  charts.pie.on('click', params => {
    selectedType.value = selectedType.value === params.name ? null : params.name
  })

  updateCharts()
}

function updateCharts() {
  if (!charts.trend) return
  const dark = document.documentElement.classList.contains('dark')
  const textColor = dark ? '#9ca3af' : '#6b7280'
  const lineColor = dark ? '#374151' : '#e5e7eb'

  // 趋势图(双轴面积)
  const d = daily.value
  charts.trend.setOption({
    tooltip: { trigger: 'axis' },
    legend: { data: ['事件数', '时长(h)'], textStyle: { color: textColor } },
    grid: { left: 40, right: 40, top: 40, bottom: 30 },
    xAxis: { type: 'category', data: d.map(x => `${x.date.getMonth() + 1}/${x.date.getDate()}`), axisLabel: { color: textColor } },
    yAxis: [
      { type: 'value', name: '事件数', axisLabel: { color: textColor }, splitLine: { lineStyle: { color: lineColor } } },
      { type: 'value', name: '小时', axisLabel: { color: textColor }, splitLine: { show: false } }
    ],
    series: [
      { name: '事件数', type: 'line', smooth: true, areaStyle: { opacity: 0.15 }, itemStyle: { color: '#6366f1' }, data: d.map(x => x.count) },
      { name: '时长(h)', type: 'line', smooth: true, yAxisIndex: 1, areaStyle: { opacity: 0.1 }, itemStyle: { color: '#0ea5e9' }, data: d.map(x => x.hours) }
    ]
  })

  // 类型分布(环形)
  charts.pie.setOption({
    tooltip: { trigger: 'item', formatter: '{b}<br/>数量: {c} ({d}%)' },
    legend: { orient: 'vertical', right: 10, top: 'center', textStyle: { color: textColor } },
    series: [{
      type: 'pie', radius: ['45%', '70%'], center: ['40%', '50%'],
      label: { show: false },
      emphasis: { label: { show: true, fontSize: 14, fontWeight: 'bold', formatter: '{b}\n{d}%' } },
      data: typeDist.value.map(t => ({ name: t.label, value: t.count, itemStyle: { color: t.color } }))
    }]
  })

  // 时段热力
  charts.hourly.setOption({
    tooltip: { formatter: p => `${p.name}:00<br/>事件 ${p.value} 次` },
    grid: { left: 40, right: 20, top: 20, bottom: 30 },
    xAxis: { type: 'category', data: hourly.value.map(h => `${h.hour}`), axisLabel: { color: textColor } },
    yAxis: { type: 'value', axisLabel: { color: textColor }, splitLine: { lineStyle: { color: lineColor } } },
    series: [{
      type: 'bar', barWidth: '60%',
      data: hourly.value.map(h => ({
        value: h.count,
        itemStyle: {
          color: h.count === 0 ? (dark ? '#1f2937' : '#f3f4f6') : `rgba(99, 102, 241, ${Math.min(0.3 + h.count / 20, 1)})`
        }
      }))
    }]
  })

  // 星期对比
  charts.weekday.setOption({
    tooltip: { formatter: p => `${p.name}<br/>总计 ${p.value} 件` },
    grid: { left: 40, right: 20, top: 20, bottom: 30 },
    xAxis: { type: 'category', data: weekday.value.map(w => w.day), axisLabel: { color: textColor } },
    yAxis: { type: 'value', axisLabel: { color: textColor }, splitLine: { lineStyle: { color: lineColor } } },
    series: [{
      type: 'bar', barWidth: '50%',
      itemStyle: { color: '#8b5cf6', borderRadius: [4, 4, 0, 0] },
      data: weekday.value.map(w => w.total)
    }]
  })
}

function resizeCharts() { Object.values(charts).forEach(c => c?.resize()) }

watch([rangeInfo, selectedType, () => events.value.length], () => nextTick(updateCharts), { deep: true })

// ---------- 周报 ----------
const generateWeeklyReport = () => {
  const { insights: tips, typeDist: types } = { insights: insights.value, typeDist: typeDist.value }
  const total = allEventsInPeriod.value.length
  const hours = Math.round(totalHours.value * 10) / 10
  const busiest = busiestDay.value
  const typeSummary = types.slice(0, 3).map(t => `${t.label} ${t.count}件(${t.percent}%)`).join('、')

  reportText.value = [
    `统计周期：${formatYmd(rangeInfo.value.start)} 至 ${formatYmd(rangeInfo.value.end)}（${rangeInfo.value.days}天）`,
    ``,
    `📊 总览`,
    `- 总事件：${total} 件`,
    `- 总时长：${hours} 小时`,
    `- 日均事件：${(total / rangeInfo.value.days).toFixed(1)} 件`,
    `- 最忙日：${busiest?.count ? `${busiest.date.getMonth() + 1}/${busiest.date.getDate()}（${busiest.count}件）` : '无'}`,
    `- 主要类型：${typeSummary || '无'}`,
    ``,
    `💡 洞察`,
    ...tips.map(t => `- ${t.text}`)
  ].join('\n')
}

const copyReport = async () => {
  if (!reportText.value) generateWeeklyReport()
  try {
    await navigator.clipboard.writeText(reportText.value)
    ElMessage.success('已复制统计报告')
  } catch { ElMessage.error('复制失败') }
}
</script>

<template>
  <div class="stats-page">
    <header class="stats-header">
      <div>
        <h2 class="stats-title">统计中心</h2>
        <p class="stats-subtitle">多维度分析你的时间分配与工作节奏</p>
      </div>
      <div class="stats-actions">
        <el-date-picker v-model="dateRange" type="daterange" range-separator="至"
          start-placeholder="开始" end-placeholder="结束" value-format="YYYY-MM-DD" style="width: 240px" />
        <el-select v-model="range" style="width: 110px">
          <el-option label="今天" value="今天" /><el-option label="近7天" value="近7天" />
          <el-option label="近30天" value="近30天" /><el-option label="本月" value="本月" />
          <el-option label="本季度" value="本季度" /><el-option label="本半年" value="本半年" />
          <el-option label="本年" value="本年" />
        </el-select>
      </div>
    </header>

    <div class="page-body">
      <!-- 摘要卡片 -->
      <section class="stats-grid">
        <div v-for="card in summaryCards" :key="card.title" class="summary-card" :style="{ '--accent': card.color }">
          <div class="summary-card__header">
            <span class="summary-card__icon">{{ card.icon }}</span>
            <span class="summary-card__title">{{ card.title }}</span>
          </div>
          <div class="summary-card__value">{{ card.value }}</div>
          <div v-if="card.change" class="summary-card__change" :style="{ color: card.color }">{{ card.change }}</div>
        </div>
      </section>

      <!-- 趋势 + 类型分布 -->
      <section class="stats-row">
        <div class="stats-panel">
          <div class="panel-header">
            <span class="panel-title">📈 事件趋势</span>
            <span v-if="selectedType" class="filter-tag" @click="selectedType = null">
              {{ selectedType }} ✕
            </span>
          </div>
          <div id="chart-trend" class="chart-lg"></div>
        </div>
        <div class="stats-panel">
          <div class="panel-header">
            <span class="panel-title">🍩 类型分布</span>
            <span class="panel-hint">点击筛选</span>
          </div>
          <div id="chart-pie" class="chart-lg"></div>
        </div>
      </section>

      <!-- 时段 + 星期 -->
      <section class="stats-row">
        <div class="stats-panel">
          <div class="panel-header"><span class="panel-title">🕐 时段分布</span></div>
          <div id="chart-hourly" class="chart-sm"></div>
        </div>
        <div class="stats-panel">
          <div class="panel-header"><span class="panel-title">📊 星期对比</span></div>
          <div id="chart-weekday" class="chart-sm"></div>
        </div>
      </section>

      <!-- 智能洞察 -->
      <section class="stats-panel insight-panel">
        <div class="panel-header"><span class="panel-title">💡 智能洞察</span></div>
        <div class="insight-list">
          <div v-for="(tip, i) in insights" :key="i" class="insight-item">
            <span class="insight-icon">{{ tip.icon }}</span>
            <div class="insight-content">
              <div class="insight-title">{{ tip.title }}</div>
              <div class="insight-text">{{ tip.text }}</div>
            </div>
          </div>
          <div v-if="insights.length === 0" class="insight-empty">暂无洞察建议，先添加一些事件吧</div>
        </div>
      </section>

      <!-- 周报 -->
      <section class="stats-panel">
        <div class="panel-header">
          <span class="panel-title">📝 统计报告</span>
          <div>
            <el-button size="small" @click="generateWeeklyReport">生成</el-button>
            <el-button size="small" type="primary" @click="copyReport">复制</el-button>
          </div>
        </div>
        <pre v-if="reportText" class="report-text">{{ reportText }}</pre>
        <div v-else class="report-empty">点击"生成"查看统计报告</div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.stats-page {
  display: flex; flex-direction: column; height: 100%; overflow: hidden;
  box-sizing: border-box; scrollbar-gutter: stable;
}
.page-body {
  flex: 1; overflow-y: auto; padding: 20px;
  display: flex; flex-direction: column; gap: 16px; box-sizing: border-box;
}

.stats-header {
  display: flex; align-items: flex-start; justify-content: space-between; gap: 16px;
  padding: 20px 30px; background: #fff; border-bottom: 1px solid #e2e8f0;
  box-shadow: 0 1px 3px rgba(0,0,0,0.05); flex-shrink: 0;
}
.stats-title { margin: 0; font-size: 20px; color: #1f2937; }
.stats-subtitle { margin: 4px 0 0; font-size: 13px; color: #6b7280; }
.stats-actions { display: flex; align-items: center; gap: 8px; }

/* 摘要卡片 */
.stats-grid {
  display: grid; grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
  gap: 12px;
}
.summary-card {
  background: #fff; border-radius: 10px; padding: 14px 16px;
  border-left: 3px solid var(--accent, #4f46e5);
  box-shadow: 0 1px 3px rgba(0,0,0,0.05);
}
.summary-card__header { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; }
.summary-card__icon { font-size: 18px; }
.summary-card__title { font-size: 12px; color: #6b7280; }
.summary-card__value { font-size: 22px; font-weight: 700; color: #1f2937; }
.summary-card__change { font-size: 12px; margin-top: 4px; }

/* 面板 */
.stats-row { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
.stats-panel {
  background: #fff; border-radius: 10px; padding: 16px;
  box-shadow: 0 1px 3px rgba(0,0,0,0.05);
}
.panel-header {
  display: flex; align-items: center; justify-content: space-between;
  margin-bottom: 12px;
}
.panel-title { font-size: 14px; font-weight: 600; color: #374151; }
.panel-hint { font-size: 11px; color: #9ca3af; }

.chart-lg { height: 280px; }
.chart-sm { height: 200px; }

.filter-tag {
  font-size: 12px; padding: 2px 8px; border-radius: 10px;
  background: #eef2ff; color: #4f46e5; cursor: pointer;
}

/* 洞察 */
.insight-panel { grid-column: 1 / -1; }
.insight-list { display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 10px; }
.insight-item {
  display: flex; gap: 10px; padding: 12px; border-radius: 8px;
  background: #f8fafc; border: 1px solid #f1f5f9;
}
.insight-icon { font-size: 20px; flex-shrink: 0; }
.insight-title { font-size: 13px; font-weight: 600; color: #1f2937; margin-bottom: 2px; }
.insight-text { font-size: 12px; color: #6b7280; line-height: 1.5; }
.insight-empty { text-align: center; color: #9ca3af; padding: 20px; }

/* 报告 */
.report-text {
  background: #f8fafc; border-radius: 8px; padding: 16px;
  font-family: monospace; font-size: 13px; color: #374151;
  white-space: pre-wrap; line-height: 1.6;
}
.report-empty { text-align: center; color: #9ca3af; padding: 30px; }

/* 响应式 */
@media (max-width: 900px) { .stats-row { grid-template-columns: 1fr; } }

/* 深色模式 */
html.dark .stats-header { background: #2f2f2f; border-bottom-color: #3a3a3a; }
html.dark .stats-title { color: #e5e7eb; }
html.dark .stats-subtitle { color: #9ca3af; }
html.dark .summary-card { background: #2f2f2f; }
html.dark .summary-card__value { color: #e5e7eb; }
html.dark .summary-card__title { color: #9ca3af; }
html.dark .stats-panel { background: #2f2f2f; }
html.dark .panel-title { color: #d1d5db; }
html.dark .insight-item { background: #262626; border-color: #333; }
html.dark .insight-title { color: #e5e7eb; }
html.dark .insight-text { color: #9ca3af; }
html.dark .report-text { background: #262626; color: #d1d5db; }
html.dark .filter-tag { background: #312e81; color: #a5b4fc; }
</style>

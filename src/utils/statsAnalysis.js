// 统计数据分析工具：从事件列表提取统计指标
import { eventTypes, getEventType } from './eventTypes'

// ---------- 日期工具 ----------

export const startOfDay = (d) => new Date(d.getFullYear(), d.getMonth(), d.getDate())

export const parseDate = (v) => {
  if (!v) return null
  if (v instanceof Date) return startOfDay(v)
  const [y, m, d] = String(v).split('-').map(Number)
  if (y && m && d) return new Date(y, m - 1, d)
  return null
}

export const formatYmd = (d) => {
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const dd = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${dd}`
}

export const isEventOnDay = (ev, day) => {
  const s = formatYmd(day)
  if (!ev.isAllDay) return ev.date === s
  const a = parseDate(ev.date)
  const b = parseDate(ev.endDate || ev.date)
  return a && b && formatYmd(a) <= s && s <= formatYmd(b)
}

export const isInRange = (ev, start, end) => {
  const a = parseDate(ev.date)
  const b = parseDate(ev.endDate || ev.date)
  if (!a) return false
  if (!b) return formatYmd(a) >= formatYmd(start) && formatYmd(a) <= formatYmd(end)
  return formatYmd(b) >= formatYmd(start) && formatYmd(a) <= formatYmd(end)
}

// ---------- 核心统计 ----------

/// 事件在指定日列表上的分布(每日计数与时长)
export function dailyDistribution(events, days) {
  return days.map(day => {
    const dayEvents = events.filter(ev => isEventOnDay(ev, day))
    const hours = dayEvents.reduce((s, ev) => {
      if (ev.isAllDay) return s
      return s + Math.max(0, (ev.endTime || 0) - (ev.startTime || 0))
    }, 0)
    return { date: day, count: dayEvents.length, hours: Math.round(hours * 10) / 10 }
  })
}

/// 每小时事件密度(8:00~23:00)
export function hourlyDensity(events) {
  const slots = Array.from({ length: 16 }, (_, i) => ({ hour: i + 8, count: 0 }))
  events.forEach(ev => {
    if (ev.isAllDay) return
    const st = Math.max(8, Math.floor(ev.startTime || 8))
    const et = Math.min(23, Math.ceil(ev.endTime || 9))
    for (let h = st; h < et; h++) {
      if (slots[h - 8]) slots[h - 8].count++
    }
  })
  return slots
}

/// 星期分布(周一~周日)
export function weekdayDistribution(events, days) {
  const counts = Array(7).fill(0)
  const totalDays = [0, 0, 0, 0, 0, 0, 0]
  days.forEach(d => { totalDays[(d.getDay() + 6) % 7]++ })
  events.forEach(ev => {
    const st = parseDate(ev.date)
    if (!st) return
    const idx = (st.getDay() + 6) % 7
    counts[idx]++
  })
  return counts.map((c, i) => ({
    day: ['周一', '周二', '周三', '周四', '周五', '周六', '周日'][i],
    total: c,
    avg: totalDays[i] ? Math.round((c / totalDays[i]) * 10) / 10 : 0
  }))
}

/// 类型分布(数量/时长/占比)
export function typeBreakdown(events) {
  const total = events.length
  const map = {}
  eventTypes.forEach(t => { map[t.value] = { label: t.label, color: t.color, count: 0, hours: 0 } })
  events.forEach(ev => {
    const key = map[ev.type] ? ev.type : 'other'
    map[key].count++
    if (!ev.isAllDay) {
      map[key].hours += Math.max(0, (ev.endTime || 0) - (ev.startTime || 0))
    }
  })
  return Object.entries(map)
    .filter(([, v]) => v.count > 0)
    .map(([value, v]) => ({
      value, label: v.label, color: v.color,
      count: v.count, hours: Math.round(v.hours * 10) / 10,
      percent: total ? Math.round((v.count / total) * 100) : 0
    }))
    .sort((a, b) => b.count - a.count)
}

/// 年度热力图数据(365天)
export function yearlyHeatmap(events, year) {
  const data = []
  const d = new Date(year, 0, 1)
  while (d.getFullYear() === year) {
    const s = formatYmd(d)
    const count = events.filter(ev => isEventOnDay(ev, s === ev.date ? d : d)).length
    data.push([s, count])
    d.setDate(d.getDate() + 1)
  }
  return data
}

// ---------- 智能洞察 ----------

export function generateInsights(events, days, hourly, weekday, typeDist) {
  const tips = []

  // 1. 最高效时段
  const peak = hourly.reduce((p, c) => c.count > p.count ? c : p, hourly[0])
  if (peak && peak.count > 0) {
    tips.push({
      icon: '🔥', title: '高峰时段',
      text: `${peak.hour}:00-${peak.hour + 1}:00 是你事件最密集的时段（${peak.count} 次），建议将重要工作安排在此前后。`
    })
  }

  // 2. 最忙日
  const busiest = weekday.reduce((p, c) => c.total > p.total ? c : p, weekday[0])
  if (busiest && busiest.total > 0) {
    tips.push({
      icon: '📅', title: '最忙日',
      text: `${busiest.day}平均 ${busiest.avg} 个事件/天，为一周中最忙；建议深度工作避开此日。`
    })
  }

  // 3. 类型占比 TOP1
  if (typeDist.length > 0) {
    const top = typeDist[0]
    if (top.percent >= 30) {
      tips.push({
        icon: '⚖️', title: '类型集中',
        text: `"${top.label}"占总事件 ${top.percent}%（${top.count} 条），类型高度集中，注意均衡安排。`
      })
    }
  }

  // 4. 会议时长占比
  const meeting = typeDist.find(t => t.value === 'meeting')
  const totalHours = typeDist.reduce((s, t) => s + t.hours, 0)
  if (meeting && totalHours > 0 && meeting.hours / totalHours > 0.3) {
    tips.push({
      icon: '🤝', title: '会议占比',
      text: `会议占总时长 ${Math.round((meeting.hours / totalHours) * 100)}%（${meeting.hours}h），建议集中批处理减少碎片化。`
    })
  }

  // 5. 工作生活平衡
  const restTypes = ['vacation']
  const restCount = events.filter(ev => restTypes.includes(ev.type)).length
  const dayCount = days.length
  if (dayCount >= 7 && restCount === 0) {
    tips.push({
      icon: '🌿', title: '工作生活平衡',
      text: `过去 ${dayCount} 天无休息类事件，注意劳逸结合。`
    })
  }

  // 6. 学习成长
  const study = typeDist.find(t => t.value === 'study')
  if (study && study.hours > 0) {
    tips.push({
      icon: '📚', title: '学习投入',
      text: `学习/培训共投入 ${study.hours} 小时，日均 ${(study.hours / dayCount).toFixed(1)} 小时。`
    })
  }

  return tips.slice(0, 5)
}

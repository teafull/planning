// 事件类型配置与展示工具(单一来源, 供 Calendar/EventBlock 等共享)
export const eventTypes = [
  { value: 'task', label: '任务', color: '#667eea', bgColor: 'linear-gradient(135deg, #667eea 0%, #5a67d8 100%)' },
  { value: 'meeting', label: '会议', color: '#48bb78', bgColor: 'linear-gradient(135deg, #48bb78 0%, #38a169 100%)' },
  { value: 'issue', label: '问题', color: '#f56565', bgColor: 'linear-gradient(135deg, #f56565 0%, #e53e3e 100%)' },
  { value: 'reminder', label: '提醒', color: '#ed8936', bgColor: 'linear-gradient(135deg, #ed8936 0%, #dd6b20 100%)' },
  { value: 'deadline', label: '截止期限', color: '#c53030', bgColor: 'linear-gradient(135deg, #c53030 0%, #822727 100%)' },
  { value: 'study', label: '学习/培训', color: '#3182ce', bgColor: 'linear-gradient(135deg, #3182ce 0%, #2b6cb0 100%)' },
  { value: 'trip', label: '出差/外出', color: '#805ad5', bgColor: 'linear-gradient(135deg, #805ad5 0%, #6b46c1 100%)' },
  { value: 'vacation', label: '请假/休息', color: '#38b2ac', bgColor: 'linear-gradient(135deg, #38b2ac 0%, #2c7a7b 100%)' },
  { value: 'other', label: '其他', color: '#718096', bgColor: 'linear-gradient(135deg, #718096 0%, #4a5568 100%)' }
]

// 获取事件类型配置(未知类型回退"其他")
export const getEventType = (type) => {
  return eventTypes.find(t => t.value === type) || eventTypes.find(t => t.value === 'other')
}

// 格式化时间数值(9.5 → "09:30")
export const formatTime = (hour) => {
  const h = Math.floor(hour)
  const m = (hour % 1) * 60
  return `${h.toString().padStart(2, '0')}:${m === 0 ? '00' : m.toString()}`
}

// 事件悬停详情(HTML, 供 el-tooltip raw-content)
export const getEventDetailHtml = (event) => {
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

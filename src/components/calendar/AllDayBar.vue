// 全天事件区：按行布局的跨天事件条 + 日期拖拽手柄
<script setup>
import { ElTooltip } from 'element-plus'
import { getEventType, getEventDetailHtml } from '../../utils/eventTypes'

defineProps({
  days: { type: Array, required: true },        // 本周 Date 数组
  rows: { type: Array, required: true },        // 每行事件二维数组
  rowCount: { type: Number, required: true },
  isToday: { type: Function, required: true },
  isWorkday: { type: Function, required: true }
})

const emit = defineEmits(['create', 'open', 'resize'])
</script>

<template>
  <div class="all-day-events-section" :style="{ height: `${rowCount * 26}px` }">
    <div class="all-day-time-header">
      <div class="all-day-label">全天</div>
    </div>
    <div class="all-day-days-container">
      <div v-for="row in rowCount" :key="row" class="all-day-events-row">
        <div class="all-day-events-grid">
          <div
            v-for="day in days"
            :key="day.getTime()"
            class="all-day-day-cell"
            :class="{ 'today': isToday(day), 'weekend': day.getDay() === 0 || day.getDay() === 6, 'is-workday': isWorkday(day) }"
            @mousedown.stop="emit('create', $event, day)"
          ></div>
          <div
            v-for="event in rows[row - 1] || []"
            :key="event.id"
            class="all-day-event-bar"
            :style="{
              left: `${(event.startIndex / 7) * 100}%`,
              width: `${(event.span / 7) * 100}%`,
              background: getEventType(event.type).bgColor,
              zIndex: 10
            }"
            @mousedown.stop="emit('open', event)"
          >
            <div class="allday-resize-handle handle-left" title="拖拽调整开始日期" @mousedown.stop="emit('resize', $event, event, 'start')"></div>
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
            <div class="allday-resize-handle handle-right" title="拖拽调整结束日期" @mousedown.stop="emit('resize', $event, event, 'end')"></div>
          </div>
        </div>
      </div>
    </div>
    <div class="all-day-event-list-placeholder"></div>
  </div>
</template>

<style scoped>
.all-day-events-section {
  display: flex;
  align-items: stretch;
  border-bottom: 1px solid #e0e0e0;
  background: white;
}

.all-day-time-header {
  width: 60px;
  flex: 0 0 60px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-right: 1px solid #e0e0e0;
  background: #f9fafb;
}

.all-day-label {
  font-size: 12px;
  color: #6b7280;
}

.all-day-days-container {
  flex: 1;
  display: flex;
  flex-direction: column;
}

.all-day-events-row {
  height: 26px;
  display: flex;
}

.all-day-events-grid {
  flex: 1;
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  position: relative;
  border-right: 1px solid #e0e0e0;
}

.all-day-day-cell {
  border-right: 1px solid #f0f0f0;
  cursor: pointer;
  position: relative;
}

.all-day-day-cell.today {
  background: #eff6ff;
}

.all-day-day-cell.weekend {
  background: #fafafa;
}

.all-day-day-cell.is-workday {
  background: #fff7ed;
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
  overflow: hidden;
  white-space: nowrap;
}

.all-day-event-content {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  overflow: hidden;
}

.all-day-event-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
}

.all-day-event-duration {
  font-size: 10px;
  background: rgba(255, 255, 255, 0.2);
  padding: 1px 5px;
  border-radius: 8px;
  flex-shrink: 0;
}

.all-day-event-list-placeholder {
  width: 300px;
  flex: 0 0 300px;
  border-left: 1px solid #e0e0e0;
}

/* 全天事件日期拖拽手柄 */
.allday-resize-handle {
  position: absolute;
  top: 1px;
  bottom: 1px;
  width: 7px;
  cursor: ew-resize;
  z-index: 12;
}
.allday-resize-handle.handle-left { left: 0; }
.allday-resize-handle.handle-right { right: 0; }
.allday-resize-handle::after {
  content: '';
  position: absolute;
  top: 1px;
  bottom: 1px;
  width: 2px;
  border-radius: 1px;
  background: rgba(255, 255, 255, 0.85);
  opacity: 0;
  transition: opacity 0.2s;
}
.allday-resize-handle.handle-left::after { left: 2px; }
.allday-resize-handle.handle-right::after { right: 2px; }
.allday-resize-handle:hover::after { opacity: 1; }

/* 深色模式 */
html.dark .all-day-events-section {
  background: #2f2f2f;
  border-bottom-color: #3a3a3a;
}
html.dark .all-day-time-header {
  background: #232323;
  border-right-color: #3a3a3a;
}
html.dark .all-day-days-container {
  background: transparent;
}
html.dark .all-day-label {
  color: #9ca3af;
}
html.dark .all-day-events-row {
  border-color: #333;
}
html.dark .all-day-day-cell {
  border-right-color: #333;
}
html.dark .all-day-day-cell:hover {
  background: rgba(255, 255, 255, 0.05);
}
html.dark .all-day-day-cell.today {
  background: rgba(59, 130, 246, 0.15);
}
html.dark .all-day-day-cell.weekend {
  background: rgba(245, 101, 101, 0.1);
}
html.dark .all-day-event-list-placeholder {
  border-left-color: #3a3a3a;
}
</style>

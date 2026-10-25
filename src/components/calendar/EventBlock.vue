// 日历事件块：分层自适应布局 + 时间调整手柄 + 悬停详情
<script setup>
import { ElTooltip } from 'element-plus'
import { getEventType, formatTime, getEventDetailHtml } from '../../utils/eventTypes'

defineProps({
  event: { type: Object, required: true }
})

const emit = defineEmits(['mousedown', 'contextmenu', 'resize'])
</script>

<template>
  <el-tooltip
    :content="getEventDetailHtml(event)"
    raw-content
    placement="top"
    :show-after="200"
  >
    <div
      class="event-item-absolute"
      :style="{ ...event.style, background: getEventType(event.type).bgColor }"
      @mousedown.stop="emit('mousedown', $event, event)"
      @contextmenu.prevent.stop="emit('contextmenu', $event, event)"
    >
      <div class="resize-handle handle-top" title="拖拽调整开始时间" @mousedown.stop="emit('resize', $event, event, 'start')"></div>
      <div class="event-content">
        <span v-if="event.endTime - event.startTime >= 1" class="event-type-label">{{ getEventType(event.type).label }}</span>
        <div class="event-main">
          <span v-if="event.endTime - event.startTime >= 1" class="event-time-text">{{ formatTime(event.startTime) }} - {{ formatTime(event.endTime) }}（{{ event.endTime - event.startTime }}h）</span>
          <span class="event-title-line">{{ event.title }}</span>
          <div v-if="event.endTime - event.startTime >= 2 && event.description" class="event-desc-line">{{ event.description }}</div>
        </div>
      </div>
      <div class="resize-handle handle-bottom" title="拖拽调整结束时间" @mousedown.stop="emit('resize', $event, event, 'end')"></div>
    </div>
  </el-tooltip>
</template>

<style scoped>
.event-item-absolute {
  position: absolute;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  color: white;
  padding: 4px 5px 4px 8px;
  border-radius: 4px;
  font-size: 12px;
  cursor: pointer;
  transition: transform 0.2s, box-shadow 0.2s;
  overflow: hidden;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 2px;
  min-width: 0;
  box-sizing: border-box;
  border-left: 3px solid rgba(0, 0, 0, 0.22);
}

.event-item-absolute:hover {
  transform: translateY(-2px);
  box-shadow: 0 4px 12px rgba(102, 126, 234, 0.4);
  z-index: 10;
}

/* 分层内容（≥1小时：左上角类型，时间+标题一行垂直居中；短事件仅标题） */
.event-content {
  min-width: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  gap: 3px;
  height: 100%;
}

.event-type-label {
  align-self: center;
  flex-shrink: 0;
  font-size: 9px;
  background: rgba(255, 255, 255, 0.22);
  padding: 1px 6px;
  border-radius: 8px;
  line-height: 1.2;
}

.event-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  justify-content: center;
  gap: 2px;
  min-width: 0;
  overflow: hidden;
}

.event-time-text {
  flex-shrink: 0;
  font-size: 10px;
  opacity: 0.92;
}

.event-title-line {
  font-size: 12px;
  font-weight: 600;
  line-height: 1.3;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  min-width: 0;
}

.event-desc-line {
  font-size: 10px;
  opacity: 0.75;
  line-height: 1.2;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 100%;
}

/* 时间调整手柄 */
.resize-handle {
  position: absolute;
  left: 3px;
  right: 3px;
  height: 6px;
  cursor: ns-resize;
  z-index: 5;
}
.resize-handle.handle-top { top: 0; }
.resize-handle.handle-bottom { bottom: 0; }
.resize-handle::after {
  content: '';
  position: absolute;
  left: 50%;
  transform: translateX(-50%);
  top: 2px;
  width: 24px;
  height: 2px;
  border-radius: 1px;
  background: rgba(255, 255, 255, 0.8);
  opacity: 0;
  transition: opacity 0.2s;
}
.resize-handle:hover::after { opacity: 1; }

/* Ctrl拖拽复制时禁用事件块指针, 让 elementFromPoint 命中格子 */
body.dragging-copy .event-item-absolute { pointer-events: none; }
</style>

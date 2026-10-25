// 事件编辑弹窗：表单(类型/名称/日期/时间/描述) + 保存/另存为副本/删除
<script setup>
import { computed, watch } from 'vue'
import { ElDialog, ElForm, ElFormItem, ElInput, ElDatePicker, ElButton, ElSelect, ElOption, ElSwitch } from 'element-plus'
import zhCn from 'element-plus/dist/locale/zh-cn.mjs'
import { preferences } from '../../store/settings'
import { eventTypes } from '../../utils/eventTypes'

const props = defineProps({
  show: { type: Boolean, default: false },
  // 表单对象(由父组件持有, 子组件就地编辑其字段)
  form: { type: Object, required: true }
})

const emit = defineEmits(['update:show', 'save', 'save-copy', 'delete'])

const timeSlots = computed(() => {
  const startHour = parseInt(preferences.value.dayStartTime?.split(':')[0] || 8)
  const endHour = parseInt(preferences.value.dayEndTime?.split(':')[0] || 23)
  const slots = []
  for (let hour = startHour; hour < endHour; hour++) {
    for (let half = 0; half < 2; half++) {
      slots.push({ hour, half, value: hour + half * 0.5 })
    }
  }
  slots.push({ hour: endHour, half: 0, value: endHour })
  return slots
})

const formatTime = (hour) => {
  const h = Math.floor(hour)
  const m = (hour % 1) * 60
  return `${h.toString().padStart(2, '0')}:${m === 0 ? '00' : m.toString()}`
}

// 可选结束时间(必须大于开始时间)
const availableEndTimes = computed(() => {
  if (!props.form.startTime || props.form.startTime === '') {
    return timeSlots.value
  }
  return timeSlots.value.filter(slot => slot.value > props.form.startTime)
})

// 监听开始时间变化，自动调整结束时间
watch(() => props.form.startTime, (newStartTime) => {
  if (props.form.endTime && props.form.endTime <= newStartTime) {
    props.form.endTime = newStartTime + 1
  }
})
</script>

<template>
  <el-dialog
    :model-value="show"
    title="编辑事件"
    width="500px"
    @update:model-value="emit('update:show', $event)"
  >
    <el-form :model="form" label-width="80px">
      <el-form-item label="事件类型">
        <el-radio-group v-model="form.type">
          <el-radio v-for="type in eventTypes" :key="type.value" :value="type.value">
            <span :style="{ color: type.color, fontWeight: 500 }">● {{ type.label }}</span>
          </el-radio>
        </el-radio-group>
      </el-form-item>
      <el-form-item label="事件名称">
        <el-input v-model="form.title" placeholder="输入事件名称" />
      </el-form-item>
      <el-form-item label="全天事件">
        <el-switch v-model="form.isAllDay" />
      </el-form-item>
      <el-form-item label="事件周期" v-if="form.isAllDay">
        <el-date-picker
          v-model="form.date"
          type="daterange"
          range-separator="至"
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
      <el-button type="danger" @click="emit('delete')">删除</el-button>
      <el-button @click="emit('update:show', false)">取消</el-button>
      <el-button @click="emit('save-copy')">另存为副本</el-button>
      <el-button type="primary" @click="emit('save')">保存</el-button>
    </template>
  </el-dialog>
</template>

<style scoped>
/* 单选框排版(紧凑胶囊) */
:deep(.el-radio-group) {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 8px;
}

:deep(.el-radio) {
  margin-right: 0;
  height: 24px;
  padding: 0 6px;
  border-radius: 12px;
}

:deep(.el-radio__label) {
  font-size: 12px;
  padding-left: 5px;
}

:deep(.el-radio__inner) {
  width: 13px;
  height: 13px;
}
</style>

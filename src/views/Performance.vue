<script setup>
import { ref, computed, onMounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { invoke } from '@tauri-apps/api/core'

const PERFORMANCE_STORAGE_KEY = 'performance.items'
const DECOMPOSITION_STORAGE_KEY = 'performance.decompositions'

const activeTab = ref('kpis')
const items = ref([])
const decompositions = ref([])
const kpiInput = ref('')
const okrObjective = ref('')
const okrKeyResults = ref([''])
const newItemType = ref('kpi')
const showDecomposeDialog = ref(false)
const selectedDecomposeTarget = ref(null)
const decomposeEvents = ref('')

const loadItems = async () => {
  try {
    const raw = await invoke('db_get_kv', { key: PERFORMANCE_STORAGE_KEY })
    if (raw) {
      items.value = JSON.parse(raw)
    }
  } catch (e) {
    console.error('加载绩效项失败:', e)
  }
  loadDecompositions()
}

const saveItems = () => {
  invoke('db_set_kv', { key: PERFORMANCE_STORAGE_KEY, value: JSON.stringify(items.value) }).catch(e => console.error('保存绩效项失败:', e))
}

const loadDecompositions = async () => {
  try {
    const raw = await invoke('db_get_kv', { key: DECOMPOSITION_STORAGE_KEY })
    if (raw) {
      decompositions.value = JSON.parse(raw)
    }
  } catch (e) {
    console.error('加载分解记录失败:', e)
  }
}

const saveDecompositions = () => {
  invoke('db_set_kv', { key: DECOMPOSITION_STORAGE_KEY, value: JSON.stringify(decompositions.value) }).catch(e => console.error('保存分解记录失败:', e))
}

// KPI 管理
const addKPI = () => {
  const text = kpiInput.value.trim()
  if (!text) return
  items.value.push({
    id: Date.now(),
    type: 'kpi',
    text,
    value: '',
    unit: '',
    target: '',
    status: 'pending',
    createdAt: new Date().toISOString()
  })
  kpiInput.value = ''
  saveItems()
}

const removeKPI = (id) => {
  items.value = items.value.filter(item => item.id !== id)
  saveItems()
}

// OKR 管理
const addOKR = () => {
  const objective = okrObjective.value.trim()
  if (!objective) return
  items.value.push({
    id: Date.now(),
    type: 'okr',
    objective,
    keyResults: okrKeyResults.value.filter(kr => kr.trim()),
    status: 'pending',
    createdAt: new Date().toISOString()
  })
  okrObjective.value = ''
  okrKeyResults.value = ['']
  saveItems()
}

const removeOKR = (id) => {
  items.value = items.value.filter(item => item.id !== id)
  saveItems()
}

const addKeyResult = () => {
  okrKeyResults.value.push('')
}

const removeKeyResult = (idx) => {
  okrKeyResults.value.splice(idx, 1)
}

// 更新状态
const updateStatus = (id, status) => {
  const target = items.value.find(item => item.id === id)
  if (target) {
    target.status = status
    saveItems()
    ElMessage.success('状态已更新')
  }
}

// 绩效分解为任务
const openDecomposeDialog = (item) => {
  selectedDecomposeTarget.value = item
  const existing = decompositions.value.find(d => d.itemId === item.id)
  if (existing && existing.events) {
    decomposeEvents.value = existing.events.join('\n')
  } else {
    decomposeEvents.value = ''
  }
  showDecomposeDialog.value = true
}

const saveDecomposition = () => {
  if (!selectedDecomposeTarget.value) return
  const events = decomposeEvents.value.split('\n').filter(line => line.trim()).map(line => line.trim())
  const idx = decompositions.value.findIndex(d => d.itemId === selectedDecomposeTarget.value.id)
  if (idx >= 0) {
    decompositions.value[idx] = { itemId: selectedDecomposeTarget.value.id, events }
  } else {
    decompositions.value.push({ itemId: selectedDecomposeTarget.value.id, events })
  }
  saveDecompositions()
  showDecomposeDialog.value = false
  ElMessage.success('已将绩效拆解为具体任务')
}

const stats = computed(() => {
  const kpis = items.value.filter(item => item.type === 'kpi')
  const okrs = items.value.filter(item => item.type === 'okr')
  return {
    total: items.value.length,
    kpis: kpis.length,
    okrs: okrs.length,
    completed: items.value.filter(item => item.status === 'completed').length,
    inProgress: items.value.filter(item => item.status === 'in-progress').length,
    pending: items.value.filter(item => item.status === 'pending').length
  }
})

onMounted(() => {
  loadItems()
})
</script>

<template>
  <div class="performance-page">
    <header class="performance-header">
      <div>
        <h2 class="performance-title">绩效管理</h2>
        <p class="performance-subtitle">KPI 与 OKR 混合管理，支持拆解为具体任务。</p>
      </div>
    </header>

    <section class="performance-stats">
      <div class="stat-item">
        <span class="stat-num">{{ stats.total }}</span>
        <span class="stat-label">总绩效</span>
      </div>
      <div class="stat-item">
        <span class="stat-num stat-num--kpi">{{ stats.kpis }}</span>
        <span class="stat-label">KPI</span>
      </div>
      <div class="stat-item">
        <span class="stat-num stat-num--okr">{{ stats.okrs }}</span>
        <span class="stat-label">OKR</span>
      </div>
      <div class="stat-item">
        <span class="stat-num stat-num--done">{{ stats.completed }}</span>
        <span class="stat-label">已完成</span>
      </div>
      <div class="stat-item">
        <span class="stat-num stat-num--progress">{{ stats.inProgress }}</span>
        <span class="stat-label">进行中</span>
      </div>
      <div class="stat-item">
        <span class="stat-num stat-num--pending">{{ stats.pending }}</span>
        <span class="stat-label">待启动</span>
      </div>
    </section>

    <el-tabs v-model="activeTab" class="performance-tabs">
      <el-tab-pane label="KPI 模式" name="kpis">
        <div class="tab-content">
          <div class="input-group">
            <el-input
              v-model="kpiInput"
              placeholder="输入 KPI 目标（如：季度销售额、月活跃用户等）"
              class="kpi-input"
              @keyup.enter="addKPI"
            />
            <el-button type="primary" @click="addKPI">添加 KPI</el-button>
          </div>

          <div v-if="items.filter(item => item.type === 'kpi').length === 0" class="empty">
            <div class="empty__icon">📊</div>
            <div class="empty__text">暂无 KPI 目标</div>
          </div>

          <div v-else class="kpi-list">
            <div
              v-for="item in items.filter(item => item.type === 'kpi')"
              :key="item.id"
              class="kpi-card"
            >
              <div class="kpi-card__header">
                <span class="kpi-card__text">{{ item.text }}</span>
                <el-select v-model="item.status" size="small" @change="updateStatus(item.id, item.status)">
                  <el-option label="待启动" value="pending" />
                  <el-option label="进行中" value="in-progress" />
                  <el-option label="已完成" value="completed" />
                </el-select>
              </div>
              <div class="kpi-card__detail">
                <el-input v-model="item.target" placeholder="目标值" size="small" />
                <el-input v-model="item.value" placeholder="当前值" size="small" />
                <el-input v-model="item.unit" placeholder="单位" size="small" />
              </div>
              <div class="kpi-card__actions">
                <el-button text @click="openDecomposeDialog(item)">拆解为任务</el-button>
                <el-button text type="danger" @click="removeKPI(item.id)">删除</el-button>
              </div>
            </div>
          </div>
        </div>
      </el-tab-pane>

      <el-tab-pane label="OKR 模式" name="okrs">
        <div class="tab-content">
          <div class="input-group">
            <el-input
              v-model="okrObjective"
              placeholder="输入 O（目标）"
              class="okr-objective"
              @keyup.enter="addOKR"
            />
            <el-button type="primary" @click="addOKR">添加 OKR</el-button>
          </div>
          <div class="key-results-group">
            <div v-for="(kr, idx) in okrKeyResults" :key="idx" class="key-result-item">
              <el-input
                v-model="okrKeyResults[idx]"
                :placeholder="`关键结果 ${idx + 1}`"
                size="small"
                @keyup.enter="addKeyResult"
              />
              <el-button v-if="okrKeyResults.length > 1" text @click="removeKeyResult(idx)" size="small">删除</el-button>
            </div>
            <el-button text @click="addKeyResult" class="add-kr-btn">+ 关键结果</el-button>
          </div>

          <div v-if="items.filter(item => item.type === 'okr').length === 0" class="empty">
            <div class="empty__icon">🎯</div>
            <div class="empty__text">暂无 OKR 目标</div>
          </div>

          <div v-else class="okr-list">
            <div
              v-for="item in items.filter(item => item.type === 'okr')"
              :key="item.id"
              class="okr-card"
            >
              <div class="okr-card__objective">O：{{ item.objective }}</div>
              <div class="okr-card__krs">
                <div v-for="(kr, idx) in item.keyResults" :key="idx" class="kr-item">
                  KR{{ idx + 1 }}：{{ kr }}
                </div>
              </div>
              <div class="okr-card__actions">
                <el-select v-model="item.status" size="small" @change="updateStatus(item.id, item.status)">
                  <el-option label="待启动" value="pending" />
                  <el-option label="进行中" value="in-progress" />
                  <el-option label="已完成" value="completed" />
                </el-select>
                <el-button text @click="openDecomposeDialog(item)">拆解为任务</el-button>
                <el-button text type="danger" @click="removeOKR(item.id)">删除</el-button>
              </div>
            </div>
          </div>
        </div>
      </el-tab-pane>
    </el-tabs>

    <!-- 绩效拆解对话框 -->
    <el-dialog
      v-model="showDecomposeDialog"
      title="拆解为任务事件"
      width="500px"
      class="decompose-dialog"
    >
      <div v-if="selectedDecomposeTarget">
        <p class="decompose-tip">将「{{ selectedDecomposeTarget.text || selectedDecomposeTarget.objective }}」拆解为可执行的具体任务，每行一个：</p>
        <el-input
          v-model="decomposeEvents"
          type="textarea"
          :rows="8"
          placeholder="例如：&#10;完成竞品调研&#10;设计产品原型&#10;制定推广方案"
        />
      </div>
      <template #footer>
        <el-button @click="showDecomposeDialog = false">取消</el-button>
        <el-button type="primary" @click="saveDecomposition">保存拆解</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.performance-page {
  display: flex;
  flex-direction: column;
  gap: 24px;
  padding: 24px;
  height: 100%;
  overflow-y: auto;
  box-sizing: border-box;
}

.performance-header {
  text-align: center;
  margin-bottom: 8px;
}

.performance-title {
  margin: 0 0 6px;
  font-size: 24px;
  font-weight: 600;
  color: #111827;
}

.performance-subtitle {
  margin: 0;
  color: #6b7280;
  font-size: 14px;
}

.performance-stats {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 16px;
  margin-bottom: 20px;
}

.stat-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 16px;
  background: linear-gradient(135deg, #f8fafc, #f1f5f9);
  border: 1px solid #e2e8f0;
  border-radius: 12px;
}

.stat-num {
  font-size: 28px;
  font-weight: 700;
  color: #1f2937;
  line-height: 1;
}

.stat-num--kpi { color: #3b82f6; }
.stat-num--okr { color: #10b981; }
.stat-num--done { color: #059669; }
.stat-num--progress { color: #f59e0b; }
.stat-num--pending { color: #6b7280; }

.stat-label {
  font-size: 12px;
  color: #9ca3af;
  margin-top: 4px;
}

.performance-tabs {
  background: transparent;
}

.tab-content {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.input-group {
  display: flex;
  gap: 12px;
  align-items: stretch;
}

.kpi-input {
  flex: 1;
}

.key-results-group {
  margin-top: 12px;
  border-top: 1px dashed #d1d5db;
  padding-top: 12px;
}

.key-result-item {
  display: flex;
  gap: 8px;
  align-items: center;
  margin-bottom: 8px;
}

.key-result-item .el-input {
  flex: 1;
}

.add-kr-btn {
  margin-top: 8px;
}

.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 40px 0;
  gap: 8px;
}

.empty__icon {
  font-size: 32px;
}

.empty__text {
  font-size: 14px;
  color: #9ca3af;
}

.kpi-list, .okr-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.kpi-card {
  background: #ffffff;
  border: 1px solid #e5e7eb;
  border-radius: 12px;
  padding: 16px;
  transition: all 0.2s;
}

.kpi-card:hover {
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.06);
}

.kpi-card__header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}

.kpi-card__text {
  font-size: 16px;
  font-weight: 600;
  color: #1f2937;
  flex: 1;
}

.kpi-card__detail {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
  margin-bottom: 12px;
}

.kpi-card__actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

.okr-card {
  background: linear-gradient(135deg, #ffffff, #f0fdf4);
  border: 1px solid #bbf7d0;
  border-radius: 12px;
  padding: 16px;
  transition: all 0.2s;
}

.okr-card:hover {
  box-shadow: 0 4px 12px rgba(16, 185, 129, 0.1);
}

.okr-card__objective {
  font-size: 16px;
  font-weight: 600;
  color: #059669;
  margin-bottom: 12px;
}

.okr-card__krs {
  margin-bottom: 12px;
}

.kr-item {
  font-size: 14px;
  color: #374151;
  padding: 6px 8px;
  background: #ecfdf5;
  border-radius: 6px;
  margin-bottom: 6px;
}

.okr-card__actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

.decompose-dialog :deep(.el-dialog__body) {
  padding: 20px 24px 8px;
}

.decompose-tip {
  font-size: 14px;
  color: #374151;
  margin-bottom: 12px;
  line-height: 1.5;
}

@media (max-width: 768px) {
  .performance-stats {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
  .kpi-card__detail {
    grid-template-columns: 1fr;
  }
}
</style>
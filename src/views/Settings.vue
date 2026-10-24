<script setup>
import { ElMessage } from 'element-plus'
import { ref } from 'vue'
import {
  profile,
  preferences,
  notifications,
  privacy,
  storage,
  ai,
  addModel,
  removeModel,
  updateModel,
  resetSettings as storeReset
} from '../store/settings.js'

const saveSettings = () => {
  ElMessage.success('设置已保存')
}

const resetSettings = () => {
  storeReset()
  ElMessage.success('已恢复默认设置')
}

// AI模型管理
const currentEditingModel = ref(null)
const isEditingModel = ref(false)
const showAddModelDialog = ref(false)



const startAddModel = () => {
  currentEditingModel.value = {
    name: '',
    modelUrl: '',
    apiKey: '',
    modelName: '',
    maxTokens: 2000,
    temperature: 0.7,
    enabled: true
  }
  isEditingModel.value = false
  showAddModelDialog.value = true
}

const startEditModel = (model) => {
  currentEditingModel.value = { ...model }
  isEditingModel.value = true
  showAddModelDialog.value = true
}

const saveModel = () => {
  if (!currentEditingModel.value || !currentEditingModel.value.name) {
    ElMessage.warning('请填写模型名称')
    return
  }
  
  if (isEditingModel.value) {
    // 编辑现有模型
    updateModel(currentEditingModel.value.id, currentEditingModel.value)
    ElMessage.success('模型已更新')
  } else {
    // 添加新模型
    addModel(currentEditingModel.value)
    ElMessage.success('模型已添加')
  }
  showAddModelDialog.value = false
  currentEditingModel.value = null
  isEditingModel.value = false
}

const deleteModel = (modelId) => {
  if (ai.value.models.length <= 1) {
    ElMessage.warning('至少需要保留一个模型')
    return
  }
  removeModel(modelId)
  ElMessage.success('模型已删除')
}

const cancelEditModel = () => {
  showAddModelDialog.value = false
  currentEditingModel.value = null
  isEditingModel.value = false
}

// 处理对话框关闭
const handleDialogClose = (done) => {
  cancelEditModel()
  done()
}
</script>

<template>
  <div class="settings-page">
    <header class="settings-header">
      <div>
        <h2 class="settings-title">设置中心</h2>
        <p class="settings-subtitle">管理你的偏好、通知和数据策略。</p>
      </div>
      <div class="settings-actions">
        <el-button @click="resetSettings">恢复默认</el-button>
        <el-button type="primary" @click="saveSettings">保存设置</el-button>
      </div>
    </header>
    <div class="page-body">

    <section class="settings-grid">
      <el-card class="settings-card" shadow="never">
        <template #header>
          <div class="card-title">个人信息</div>
        </template>
        <el-form label-width="90px" class="settings-form">
          <el-form-item label="姓名">
            <el-input v-model="profile.name" placeholder="输入姓名" />
          </el-form-item>
          <el-form-item label="职位">
            <el-input v-model="profile.title" placeholder="输入职位" />
          </el-form-item>
          <el-form-item label="邮箱">
            <el-input v-model="profile.email" placeholder="输入邮箱" />
          </el-form-item>
        </el-form>
      </el-card>

      <el-card class="settings-card" shadow="never">
        <template #header>
          <div class="card-title">外观偏好</div>
        </template>
        <el-form label-width="90px" class="settings-form">
          <el-form-item label="主题">
            <el-radio-group v-model="preferences.theme">
              <el-radio-button label="light">浅色</el-radio-button>
              <el-radio-button label="dark">深色</el-radio-button>
              <el-radio-button label="system">跟随系统</el-radio-button>
            </el-radio-group>
          </el-form-item>
          <el-form-item label="密度">
            <el-select v-model="preferences.density" placeholder="选择密度">
              <el-option label="默认" value="default" />
              <el-option label="紧凑" value="compact" />
              <el-option label="宽松" value="comfortable" />
            </el-select>
          </el-form-item>
          <el-form-item label="语言">
            <el-select v-model="preferences.language" placeholder="选择语言">
              <el-option label="简体中文" value="zh-CN" />
              <el-option label="English" value="en-US" />
            </el-select>
          </el-form-item>
          <el-form-item label="周起始">
            <el-select v-model="preferences.weekStart" placeholder="周起始">
              <el-option label="周一" value="monday" />
              <el-option label="周日" value="sunday" />
            </el-select>
          </el-form-item>
          <el-form-item label="节假日API">
            <el-input v-model="preferences.holidayApiUrl" placeholder="国内节假日API地址" />
          </el-form-item>
          <el-form-item label="天时间范围">
            <div style="display: flex; align-items: center; gap: 8px;">
              <el-select v-model="preferences.dayStartTime" placeholder="开始" style="width: 100px;">
                <el-option 
                  v-for="hour in 24" 
                  :key="hour - 1" 
                  :label="String(hour - 1).padStart(2, '0') + ':00'" 
                  :value="String(hour - 1).padStart(2, '0') + ':00'"
                />
              </el-select>
              <span>至</span>
              <el-select v-model="preferences.dayEndTime" placeholder="结束" style="width: 100px;">
                <el-option 
                  v-for="hour in 24" 
                  :key="hour - 1" 
                  :label="String(hour - 1).padStart(2, '0') + ':00'" 
                  :value="String(hour - 1).padStart(2, '0') + ':00'"
                />
              </el-select>
            </div>
          </el-form-item>
        </el-form>
      </el-card>

      <el-card class="settings-card" shadow="never">
        <template #header>
          <div class="card-title">通知管理</div>
        </template>
        <div class="switch-list">
          <div class="switch-item">
            <div>
              <div class="switch-title">桌面通知</div>
              <div class="switch-desc">事件即将开始时提醒你</div>
            </div>
            <el-switch v-model="notifications.desktop" />
          </div>
          <div class="switch-item">
            <div>
              <div class="switch-title">邮件提醒</div>
              <div class="switch-desc">每日汇总发送至邮箱</div>
            </div>
            <el-switch v-model="notifications.email" />
          </div>
          <div class="switch-item">
            <div>
              <div class="switch-title">周报推送</div>
              <div class="switch-desc">每周一生成周报草稿</div>
            </div>
            <el-switch v-model="notifications.weeklyReport" />
          </div>
          <div class="switch-item">
            <div>
              <div class="switch-title">每日摘要</div>
              <div class="switch-desc">在仪表盘顶部显示摘要</div>
            </div>
            <el-switch v-model="notifications.dailyDigest" />
          </div>
        </div>
      </el-card>

      <el-card class="settings-card" shadow="never">
        <template #header>
          <div style="display: flex; justify-content: space-between; align-items: center;">
            <div class="card-title">大模型配置 - OpenAI Compatible</div>
            <el-button size="small" type="primary" @click="startAddModel">添加模型</el-button>
          </div>
        </template>
        
        <!-- 模型选择器 -->
        <div style="margin-bottom: 20px;">
          <el-form-item label="当前模型">
            <el-select v-model="ai.selectedModelId" placeholder="选择模型" style="width: 100%;">
              <el-option 
                v-for="model in ai.models.filter(m => m.enabled)" 
                :key="model.id"
                :label="model.name"
                :value="model.id"
              />
            </el-select>
          </el-form-item>
        </div>

        <!-- 模型列表 -->
        <div class="model-list">
          <div 
            v-for="model in ai.models" 
            :key="model.id"
            class="model-item"
            :class="{ 'is-selected': model.id === ai.selectedModelId }"
          >
            <div class="model-info">
              <div class="model-name">{{ model.name }}</div>
              <div class="model-details">
                <span>{{ model.modelName || '未指定模型' }}</span>
                <el-tag v-if="!model.enabled" type="info" size="small">已禁用</el-tag>
              </div>
            </div>
            <div class="model-actions">
              <el-button size="small" @click="startEditModel(model)">编辑</el-button>
              <el-button 
                size="small" 
                type="danger" 
                @click="deleteModel(model.id)"
                :disabled="ai.models.length <= 1"
              >删除</el-button>
            </div>
          </div>
        </div>
      </el-card>

      <!-- 添加/编辑模型对话框 -->
      <el-dialog 
        :title="isEditingModel ? '编辑模型' : '添加模型'" 
        v-model="showAddModelDialog"
        width="500px"
        :before-close="handleDialogClose"
      >
        <el-form :model="currentEditingModel" label-width="100px" v-if="currentEditingModel">
          <el-form-item label="模型名称" required>
            <el-input 
              v-model="currentEditingModel.name" 
              placeholder="输入模型显示名称"
            />
          </el-form-item>
          <el-form-item label="API地址">
            <el-input 
              v-model="currentEditingModel.modelUrl" 
              placeholder="输入大模型API地址"
            />
          </el-form-item>
          <el-form-item label="API密钥">
            <el-input 
              v-model="currentEditingModel.apiKey" 
              type="password" 
              placeholder="输入API密钥" 
              show-password 
            />
          </el-form-item>
          <el-form-item label="模型标识">
            <el-input 
              v-model="currentEditingModel.modelName" 
              placeholder="如：gpt-3.5-turbo"
            />
          </el-form-item>
          <el-form-item label="最大Token">
            <el-input-number 
              v-model="currentEditingModel.maxTokens" 
              :min="100" 
              :max="8000" 
              :step="100" 
            />
          </el-form-item>
          <el-form-item label="Temperature">
            <el-slider 
              v-model="currentEditingModel.temperature" 
              :min="0" 
              :max="2" 
              :step="0.1" 
              show-input 
            />
          </el-form-item>
          <el-form-item label="启用状态">
            <el-switch v-model="currentEditingModel.enabled" />
          </el-form-item>
        </el-form>
        
        <template #footer>
          <el-button @click="cancelEditModel">取消</el-button>
          <el-button type="primary" @click="saveModel">保存</el-button>
        </template>
      </el-dialog>

      <el-card class="settings-card" shadow="never">
        <template #header>
          <div class="card-title">隐私与数据</div>
        </template>
        <div class="switch-list">
          <div class="switch-item">
            <div>
              <div class="switch-title">展示统计结果</div>
              <div class="switch-desc">在统计面板中展示你的效率数据</div>
            </div>
            <el-switch v-model="privacy.showInStats" />
          </div>
          <div class="switch-item">
            <div>
              <div class="switch-title">共享专注时长</div>
              <div class="switch-desc">用于团队专注排行</div>
            </div>
            <el-switch v-model="privacy.shareFocusTime" />
          </div>
        </div>
        <el-divider class="settings-divider" />

        <el-form label-width="100px" class="settings-form">
          <el-form-item label="自动备份">
            <el-switch v-model="storage.autoBackup" />
          </el-form-item>
          <el-form-item label="备份频率">
            <el-select v-model="storage.backupFrequency" placeholder="选择频率">
              <el-option label="每日" value="daily" />
              <el-option label="每周" value="weekly" />
              <el-option label="每月" value="monthly" />
            </el-select>
          </el-form-item>
        </el-form>
      </el-card>

      <el-card class="settings-card settings-card--wide" shadow="never">
        <template #header>
          <div class="card-title">版本信息</div>
        </template>
        <div class="meta-list">
          <div class="meta-item">
            <span>当前版本</span>
            <strong>v1.2.0</strong>
          </div>
          <div class="meta-item">
            <span>最后更新</span>
            <strong>2026-02-25</strong>
          </div>
          <div class="meta-item">
            <span>数据位置</span>
            <strong>本地存储</strong>
          </div>
        </div>
      </el-card>
    </section>
    </div>
  </div>
</template>

<style scoped>
.settings-page {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
  box-sizing: border-box;
  background: transparent;
}

.page-body {
  flex: 1;
  overflow-y: auto;
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 20px;
  box-sizing: border-box;
  scrollbar-gutter: stable;
  scrollbar-width: thin;
  scrollbar-color: transparent transparent;
}

.page-body:hover {
  scrollbar-color: rgba(0, 0, 0, 0.35) transparent;
}

.page-body::-webkit-scrollbar {
  width: 8px;
}

.page-body::-webkit-scrollbar-track {
  background: transparent;
}

.page-body::-webkit-scrollbar-thumb {
  background: transparent;
  border-radius: 4px;
}

.page-body:hover::-webkit-scrollbar-thumb {
  background: rgba(0, 0, 0, 0.2);
}

.page-body::-webkit-scrollbar-thumb:hover {
  background: rgba(0, 0, 0, 0.3);
}

.settings-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  flex-shrink: 0;
  padding: 20px 30px;
  background: #fff;
  border-bottom: 1px solid #e2e8f0;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.05);
}

@media (prefers-color-scheme: dark) {
  .settings-header {
    background: #2f2f2f;
    border-bottom-color: #3a3a3a;
    box-shadow: none;
  }
  .settings-title { color: #e5e7eb; }
  .settings-subtitle { color: #9ca3af; }
  .settings-page {
    --el-bg-color: #2f2f2f;
    --el-card-bg-color: #2f2f2f;
    --el-border-color-light: #3a3a3a;
    --el-border-color-lighter: #333333;
    --el-text-color-primary: #e5e7eb;
    --el-text-color-regular: #cbd5e1;
    --el-fill-color-blank: #2f2f2f;
    --el-fill-color-light: #3a3a3a;
  }
}

.settings-title {
  margin: 0 0 6px;
  font-size: 22px;
  font-weight: 600;
  color: #1f2937;
}

.settings-subtitle {
  margin: 0;
  color: #6b7280;
  font-size: 14px;
}

.settings-actions {
  display: flex;
  gap: 8px;
  align-items: center;
  flex-wrap: wrap;
}

.settings-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 16px;
}

.settings-card {
  border-radius: 14px;
  border: 1px solid #eef2ff;
}

.settings-card--wide {
  grid-column: 1 / -1;
}

.card-title {
  font-size: 15px;
  font-weight: 600;
  color: #111827;
}

.settings-form :deep(.el-form-item) {
  margin-bottom: 14px;
}

.settings-divider {
  margin: 2px 0;
}

.switch-list {

  display: flex;
  flex-direction: column;
  gap: 14px;
}

.switch-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 12px;
  background: #f9fafb;
  border-radius: 10px;
}

.switch-title {
  font-size: 14px;
  font-weight: 600;
  color: #111827;
}

.switch-desc {
  font-size: 12px;
  color: #6b7280;
}

.meta-list {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
  gap: 12px;
}

.meta-item {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 12px;
  border-radius: 10px;
  background: #f9fafb;
  color: #374151;
}

.meta-item strong {
  font-size: 16px;
  color: #111827;
}

@media (max-width: 1024px) {
  .settings-grid {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 768px) {
  .settings-header {
    flex-direction: column;
    align-items: flex-start;
  }
}

/* AI模型列表样式 */
.model-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
  max-height: 300px;
  overflow-y: auto;
}

.model-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px;
  border: 1px solid #e5e7eb;
  border-radius: 8px;
  background: #f9fafb;
  transition: all 0.2s ease;
}

.model-item:hover {
  border-color: #667eea;
  background: #f3f4f6;
}

.model-item.is-selected {
  border-color: #667eea;
  background: #ede9fe;
}

.model-info {
  flex: 1;
}

.model-name {
  font-weight: 600;
  color: #111827;
  margin-bottom: 4px;
}

.model-details {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: #6b7280;
}

.model-actions {
  display: flex;
  gap: 8px;
}
</style>


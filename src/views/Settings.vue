<script setup>
import { ref } from 'vue'
import { ElMessage } from 'element-plus'

const profile = ref({
  name: '林溪',
  title: '项目经理',
  email: 'linxi@example.com'
})

const preferences = ref({
  theme: 'system',
  density: 'default',
  language: 'zh-CN',
  weekStart: 'monday'
})

const notifications = ref({
  desktop: true,
  email: true,
  weeklyReport: true,
  dailyDigest: false
})

const privacy = ref({
  showInStats: true,
  shareFocusTime: false
})

const storage = ref({
  autoBackup: true,
  backupFrequency: 'weekly'
})

const saveSettings = () => {
  ElMessage.success('设置已保存')
}

const resetSettings = () => {
  profile.value = { name: '林溪', title: '项目经理', email: 'linxi@example.com' }
  preferences.value = { theme: 'system', density: 'default', language: 'zh-CN', weekStart: 'monday' }
  notifications.value = { desktop: true, email: true, weeklyReport: true, dailyDigest: false }
  privacy.value = { showInStats: true, shareFocusTime: false }
  storage.value = { autoBackup: true, backupFrequency: 'weekly' }
  ElMessage.success('已恢复默认设置')
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
        <el-divider />
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
</template>

<style scoped>
.settings-page {
  display: flex;
  flex-direction: column;
  gap: 20px;
  padding: 20px;
  height: 100%;
  overflow-y: auto;
  box-sizing: border-box;
}

.settings-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
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
</style>


<template>
  <div class="ai-chat-page">
    <header class="chat-header">
      <div>
        <h2 class="chat-title">AI 助手</h2>
        <p class="chat-subtitle">测试大模型对话功能</p>
      </div>
      <div class="model-selector">
        <el-select v-model="selectedModelId" placeholder="选择模型" style="width: 200px;">
          <el-option 
            v-for="model in enabledModels" 
            :key="model.id"
            :label="model.name"
            :value="model.id"
          />
        </el-select>
      </div>
    </header>

    <main class="chat-container">
      <div class="messages-container" ref="messagesContainer">
        <div v-if="messages.length === 0" class="empty-state">
          <div class="empty-icon">💬</div>
          <div class="empty-text">开始与 AI 助手对话</div>
        </div>
        
        <div 
          v-for="message in messages" 
          :key="message.id"
          class="message"
          :class="{ 'message-user': message.role === 'user', 'message-assistant': message.role === 'assistant' }"
        >
          <div class="message-avatar">
            <span v-if="message.role === 'user'">👤</span>
            <span v-else>🤖</span>
          </div>
          <div class="message-content">
            <div class="message-text">{{ message.content }}</div>
            <div class="message-time">{{ formatTime(message.timestamp) }}</div>
          </div>
        </div>
        
        <div v-if="isLoading" class="message message-assistant">
          <div class="message-avatar">🤖</div>
          <div class="message-content">
            <div class="typing-indicator">
              <span></span>
              <span></span>
              <span></span>
            </div>
          </div>
        </div>
      </div>
    </main>

    <footer class="chat-input-container">
      <div class="input-wrapper">
        <el-input
          v-model="inputMessage"
          type="textarea"
          :rows="3"
          placeholder="输入消息..."
          :disabled="isLoading || !selectedModelId"
          @keydown.enter.exact.prevent="sendMessage"
          @keydown.shift.enter.exact="inputMessage += '\n'"
          class="message-input"
        />
        <el-button 
          type="primary" 
          @click="sendMessage"
          :disabled="isLoading || !selectedModelId || !inputMessage.trim()"
          class="send-button"
        >
          发送
        </el-button>
      </div>
      <div class="input-hints">
        <span class="hint">按 Enter 发送，Shift+Enter 换行</span>
      </div>
    </footer>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, nextTick } from 'vue'
import { ElMessage } from 'element-plus'
import { ai, getEnabledModels } from '../store/settings.js'

// 响应式数据
const messages = ref([])
const inputMessage = ref('')
const isLoading = ref(false)
const selectedModelId = ref('')
const messagesContainer = ref(null)

// 计算属性
const enabledModels = computed(() => getEnabledModels())

// 发送消息
const sendMessage = async () => {
  if (!inputMessage.value.trim() || isLoading.value || !selectedModelId.value) {
    return
  }

  const userMessage = {
    id: Date.now(),
    role: 'user',
    content: inputMessage.value.trim(),
    timestamp: new Date()
  }

  messages.value.push(userMessage)
  inputMessage.value = ''
  
  // 滚动到底部
  await nextTick()
  scrollToBottom()

  // 开始加载
  isLoading.value = true

  try {
    // 获取选中的模型配置
    const selectedModel = enabledModels.value.find(model => model.id === selectedModelId.value)
    if (!selectedModel) {
      throw new Error('未找到选中的模型')
    }

    // 构建 API 请求
    const response = await callAIModel(selectedModel, userMessage.content)
    
    const assistantMessage = {
      id: Date.now() + 1,
      role: 'assistant',
      content: response,
      timestamp: new Date()
    }

    messages.value.push(assistantMessage)
    
    // 滚动到底部
    await nextTick()
    scrollToBottom()
    
  } catch (error) {
    console.error('AI 调用失败:', error)
    ElMessage.error(`AI 调用失败: ${error.message}`)
    
    // 添加错误消息
    const errorMessage = {
      id: Date.now() + 1,
      role: 'assistant',
      content: `抱歉，调用 AI 服务时出错：${error.message}`,
      timestamp: new Date()
    }
    messages.value.push(errorMessage)
  } finally {
    isLoading.value = false
  }
}

// 调用 AI 模型
const callAIModel = async (model, message) => {
  if (!model.modelUrl) {
    throw new Error('未配置模型 API 地址')
  }

  const requestBody = {
    model: model.modelName,
    messages: [
      {
        role: 'user',
        content: message
      }
    ],
    max_tokens: model.maxTokens,
    temperature: model.temperature
  }

  const response = await fetch(model.modelUrl, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      ...(model.apiKey && { 'Authorization': `Bearer ${model.apiKey}` })
    },
    body: JSON.stringify(requestBody)
  })

  if (!response.ok) {
    const errorData = await response.text()
    throw new Error(`HTTP ${response.status}: ${errorData}`)
  }

  const data = await response.json()
  
  // 尝试不同格式的响应解析
  if (data.choices && data.choices[0] && data.choices[0].message) {
    // OpenAI 格式
    return data.choices[0].message.content
  } else if (data.response) {
    // 其他格式
    return data.response
  } else if (data.content) {
    // 简单格式
    return data.content
  } else {
    throw new Error('无法解析 AI 响应格式')
  }
}

// 滚动到底部
const scrollToBottom = () => {
  if (messagesContainer.value) {
    messagesContainer.value.scrollTop = messagesContainer.value.scrollHeight
  }
}

// 格式化时间
const formatTime = (date) => {
  return new Date(date).toLocaleTimeString('zh-CN', {
    hour: '2-digit',
    minute: '2-digit'
  })
}

// 组件挂载时设置默认选中模型
onMounted(() => {
  if (enabledModels.value.length > 0 && ai.value.selectedModelId) {
    selectedModelId.value = ai.value.selectedModelId
  }
})
</script>

<style scoped>
.ai-chat-page {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: #f5f5f5;
}

.chat-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 20px;
  background: white;
  border-bottom: 1px solid #e5e7eb;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
}

.chat-title {
  margin: 0 0 4px;
  font-size: 24px;
  font-weight: 600;
  color: #111827;
}

.chat-subtitle {
  margin: 0;
  color: #6b7280;
  font-size: 14px;
}

.chat-container {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.messages-container {
  flex: 1;
  overflow-y: auto;
  padding: 20px;
  scroll-behavior: smooth;
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 200px;
  color: #9ca3af;
}

.empty-icon {
  font-size: 48px;
  margin-bottom: 16px;
}

.empty-text {
  font-size: 16px;
}

.message {
  display: flex;
  margin-bottom: 20px;
  max-width: 800px;
}

.message-user {
  margin-left: auto;
  flex-direction: row-reverse;
}

.message-assistant {
  margin-right: auto;
}

.message-avatar {
  width: 40px;
  height: 40px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 20px;
  background: #f3f4f6;
  margin: 0 12px;
  flex-shrink: 0;
}

.message-user .message-avatar {
  background: #667eea;
  color: white;
}

.message-assistant .message-avatar {
  background: #10b981;
  color: white;
}

.message-content {
  flex: 1;
  min-width: 0;
}

.message-text {
  padding: 12px 16px;
  border-radius: 12px;
  white-space: pre-wrap;
  word-wrap: break-word;
  line-height: 1.5;
}

.message-user .message-text {
  background: #667eea;
  color: white;
  border-bottom-right-radius: 4px;
}

.message-assistant .message-text {
  background: white;
  color: #111827;
  border: 1px solid #e5e7eb;
  border-bottom-left-radius: 4px;
}

.message-time {
  font-size: 12px;
  color: #9ca3af;
  margin-top: 4px;
  padding: 0 12px;
}

.message-user .message-time {
  text-align: right;
}

.typing-indicator {
  display: flex;
  gap: 4px;
  padding: 12px 16px;
  background: white;
  border: 1px solid #e5e7eb;
  border-radius: 12px;
  border-bottom-left-radius: 4px;
  width: 60px;
}

.typing-indicator span {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #9ca3af;
  animation: typing 1.4s infinite ease-in-out;
}

.typing-indicator span:nth-child(1) {
  animation-delay: -0.32s;
}

.typing-indicator span:nth-child(2) {
  animation-delay: -0.16s;
}

@keyframes typing {
  0%, 80%, 100% {
    transform: scale(0.8);
    opacity: 0.5;
  }
  40% {
    transform: scale(1);
    opacity: 1;
  }
}

.chat-input-container {
  background: white;
  border-top: 1px solid #e5e7eb;
  padding: 20px;
}

.input-wrapper {
  display: flex;
  gap: 12px;
  align-items: flex-end;
}

.message-input {
  flex: 1;
}

.send-button {
  height: 76px;
  padding: 0 24px;
}

.input-hints {
  margin-top: 8px;
  text-align: center;
}

.hint {
  font-size: 12px;
  color: #9ca3af;
}

/* 滚动条样式 */
.messages-container::-webkit-scrollbar {
  width: 6px;
}

.messages-container::-webkit-scrollbar-track {
  background: transparent;
}

.messages-container::-webkit-scrollbar-thumb {
  background: #d1d5db;
  border-radius: 3px;
}

.messages-container::-webkit-scrollbar-thumb:hover {
  background: #9ca3af;
}

@media (max-width: 768px) {
  .chat-header {
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
  }
  
  .model-selector {
    width: 100%;
  }
  
  .message {
    max-width: none;
  }
  
  .input-wrapper {
    flex-direction: column;
    align-items: stretch;
  }
  
  .send-button {
    height: auto;
    width: 100%;
  }
}
</style>
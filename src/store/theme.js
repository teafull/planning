import { watch } from 'vue'
import { preferences } from './settings'

// 主题应用：html.dark 类控制全应用深浅色
// 偏好: light=浅色 / dark=深色 / system=跟随系统(实时响应系统切换)
const media = window.matchMedia('(prefers-color-scheme: dark)')

const isDarkNow = () => {
  const theme = preferences.value.theme
  if (theme === 'dark') return true
  if (theme === 'light') return false
  return media.matches // system
}

const applyTheme = () => {
  document.documentElement.classList.toggle('dark', isDarkNow())
}

let inited = false

export const initTheme = () => {
  if (inited) return
  inited = true

  applyTheme()

  // 用户在设置页切换主题 → 立即生效
  watch(() => preferences.value.theme, applyTheme)

  // 系统主题变化 → 仅"跟随系统"模式下响应
  media.addEventListener('change', () => {
    if (preferences.value.theme === 'system') applyTheme()
  })
}

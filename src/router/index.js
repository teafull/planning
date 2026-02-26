import { createRouter, createWebHashHistory } from 'vue-router'
import Calendar from '../views/Calendar.vue'
import Tasks from '../views/Tasks.vue'
import Stats from '../views/Stats.vue'
import Settings from '../views/Settings.vue'
import Performance from '../views/Performance.vue'
import AIChat from '../views/AIChat.vue'
import MonthView from '../views/MonthView.vue'

const routes = [
  { path: '/', redirect: '/calendar' },
  { path: '/calendar', name: 'calendar', component: Calendar },
  { path: '/tasks', name: 'tasks', component: Tasks },
  { path: '/stats', name: 'stats', component: Stats },
  { path: '/settings', name: 'settings', component: Settings },
  { path: '/performance', name: 'performance', component: Performance },
  { path: '/ai-chat', name: 'ai-chat', component: AIChat },
  { path: '/month', name: 'month', component: MonthView }
]

export default createRouter({
  history: createWebHashHistory(),
  routes
})

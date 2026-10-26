import { createApp } from "vue";
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
// Element Plus 深色主题变量(响应 html.dark 类, 用于主题切换)
import 'element-plus/theme-chalk/dark/css-vars.css'
// 引入 Element 的中文语言包
import zhCn from 'element-plus/dist/locale/zh-cn.mjs'
import App from "./App.vue";
import router from './router'
import { initStores } from './store/init'
import { initRaftListener } from './store/events'
import { initTheme } from './store/theme'

// 先注册 raft 事件监听（在集群启动前, 防丢失启动窗口期的复制事件）,
// 再水合存储与主题, 最后挂载应用
initRaftListener().then(() => initStores()).then(() => {
  initTheme()
  createApp(App).use(ElementPlus, {
    locale: zhCn
  }).use(router).mount("#app")
})


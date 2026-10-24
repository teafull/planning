import { createApp } from "vue";
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
// Element Plus 深色主题变量(响应 html.dark 类, 用于主题切换)
import 'element-plus/theme-chalk/dark/css-vars.css'
// 引入 Element 的中文语言包
import zhCn from 'element-plus/dist/locale/zh-cn.mjs'
import App from "./App.vue";
import router from './router'

createApp(App).use(ElementPlus, {
  locale: zhCn
}).use(router).mount("#app");


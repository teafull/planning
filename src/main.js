import { createApp } from "vue";
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
// 引入 Element 的中文语言包
import zhCn from 'element-plus/dist/locale/zh-cn.mjs'
import App from "./App.vue";
import router from './router'

createApp(App).use(ElementPlus, {
  locale: zhCn
}).use(router).mount("#app");


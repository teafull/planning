<script setup>
import { useRoute } from 'vue-router'
import { initTheme } from './store/theme'

initTheme()

const route = useRoute()
</script>

<template>
  <div class="app-layout">
    <aside class="side-menu">
      <el-menu
        class="side-menu__menu"
        :default-active="route.path"
        router
      >
        <el-menu-item index="/calendar">日历</el-menu-item>
        <el-menu-item index="/month">月历</el-menu-item>
        <el-menu-item index="/tasks">任务</el-menu-item>
        <el-menu-item index="/stats">统计</el-menu-item>
        <!-- <el-menu-item index="/performance">绩效</el-menu-item>
        <el-menu-item index="/ai-chat">AI助手</el-menu-item> -->
        <el-menu-item index="/settings" class="side-menu__bottom-item">设置</el-menu-item>
      </el-menu>

    </aside>
    <main class="app-content">
      <router-view />
    </main>
  </div>
</template>

<style scoped>
.app-layout {
  display: flex;
  height: 100vh;
  background: #f8fafc;
}

  html.dark .app-layout {
    background: #1a1a1a;
  }


.side-menu {
  width: 70px;
  flex: 0 0 70px;
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: #ffffff;
  border-right: 1px solid #e2e8f0;
  color: #4b5563;
  box-sizing: border-box;
  --el-menu-bg-color: #ffffff;
  --el-menu-text-color: #4b5563;
  --el-menu-active-color: #4f46e5;
  --el-menu-hover-bg-color: #f1f5f9;
  --el-menu-item-height: 44px;
}

  html.dark .side-menu {
    background: #1f2329;
    border-right-color: #3a3a3a;
    color: #fff;
    --el-menu-bg-color: #1f2329;
    --el-menu-text-color: #d6dbe1;
    --el-menu-active-color: #ffffff;
    --el-menu-hover-bg-color: rgba(255, 255, 255, 0.12);
  }

.side-menu__menu {
  border-right: none;
  background-color: transparent;
  display: flex;
  flex-direction: column;
  flex: 1;
}

.side-menu__bottom-item {
  margin-top: auto;
}


.app-content {
  flex: 1;
  height: 100vh;
  overflow: hidden;
  box-sizing: border-box;
}

</style>





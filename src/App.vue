<script setup>
import { useRoute } from 'vue-router'

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
        <el-menu-item index="/tasks">任务</el-menu-item>
        <el-menu-item index="/stats">统计</el-menu-item>
        <!-- <el-menu-item index="/performance">绩效</el-menu-item> -->
        <!-- <el-menu-item index="/ai-chat">AI</el-menu-item> -->
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
  background: #f7f7f8;
}


.side-menu {
  width: 70px;
  flex: 0 0 70px;
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: #1f2329;
  color: #fff;
  box-sizing: border-box;
  --el-menu-bg-color: #1f2329;
  --el-menu-text-color: #d6dbe1;
  --el-menu-active-color: #ffffff;
  --el-menu-hover-bg-color: rgba(255, 255, 255, 0.12);
  --el-menu-item-height: 44px;
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





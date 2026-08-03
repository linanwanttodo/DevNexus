<script setup>
import TitleBar from "./components/TitleBar.vue";
import Sidebar from "./components/Sidebar.vue";
import ErrorBoundary from "./components/ErrorBoundary.vue";
</script>

<template>
  <div class="app-shell">
    <!-- Custom title bar -->
    <TitleBar />

    <div class="app-body">
      <Sidebar />
      <main class="app-main">
        <router-view v-slot="{ Component }">
          <ErrorBoundary>
            <component :is="Component" />
          </ErrorBoundary>
        </router-view>
      </main>
    </div>
  </div>
</template>

<style scoped>
.app-shell {
  display: flex;
  flex-direction: column;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  background-color: var(--color-bg-1, #161616);
}

.app-body {
  display: flex;
  flex: 1;
  min-height: 0;
}

.app-main {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
  background-color: var(--color-fill-2, transparent);
}

.app-main :deep(.page) {
  padding: 20px 24px;
  max-width: 1400px;
  margin: 0 auto;
}
</style>

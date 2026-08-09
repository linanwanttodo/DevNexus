<script setup>
import TitleBar from "./components/TitleBar.vue";
import Sidebar from "./components/Sidebar.vue";
import ErrorBoundary from "./components/ErrorBoundary.vue";
import ConfirmDialog from "./components/ConfirmDialog.vue";
import Sonner from "./components/ui/sonner/Sonner.vue";
import { getTheme } from "./lib/stores.js";

const theme = getTheme();
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

    <ConfirmDialog />
    <Sonner :theme="theme" position="bottom-right" rich-colors />
  </div>
</template>

<style scoped>
.app-shell {
  display: flex;
  flex-direction: column;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  background-color: var(--color-background);
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
  background-color: transparent;
}

.app-main :deep(.page) {
  padding: var(--nx-page-py, 12px) 24px;
  max-width: var(--nx-page-max-width);
  margin: 0 auto;
}
</style>

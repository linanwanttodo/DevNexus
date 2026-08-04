<script setup>
import { ref, computed, onMounted, onBeforeUnmount } from "vue";
import { useRouter } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { t } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";

const router = useRouter();

const systemInfo = ref(null);
const resourceUsage = ref(null);
const loading = ref(true);
const error = ref(null);
const environments = ref([]);

let timer = null;

function formatUptime(seconds) {
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  return `${days}d ${hours}h ${minutes}m`;
}

async function loadSystemInfo() {
  try {
    loading.value = true;
    error.value = null;
    systemInfo.value = await invoke("get_system_info");
    resourceUsage.value = await invoke("get_resource_usage");
  } catch (err) {
    error.value = friendlyError(err);
  } finally {
    loading.value = false;
  }
}

async function refreshResourceUsage() {
  try {
    resourceUsage.value = await invoke("get_resource_usage");
  } catch (err) {
    console.error("Failed to refresh resource usage:", err);
  }
}

async function loadEnvironments() {
  try {
    environments.value = await invoke("list_environments");
  } catch (err) {
    console.error("Failed to load environments:", err);
  }
}

function progressColor(val) {
  if (val > 80) return "rgb(var(--red-6))";
  if (val > 60) return "rgb(var(--orange-6))";
  return "rgb(var(--green-6))";
}

const stats = computed(() => [
  {
    id: "cpu",
    tkey: "dashboard.cpu_cores",
    value: systemInfo.value ? String(systemInfo.value.cpu_cores) : "--",
    sub: systemInfo.value?.cpu_model
      ? systemInfo.value.cpu_model.split(" ").slice(0, 2).join(" ")
      : "",
    percent: resourceUsage.value ? resourceUsage.value.cpu_usage : null,
  },
  {
    id: "memory",
    tkey: "dashboard.memory",
    value: resourceUsage.value
      ? `${resourceUsage.value.memory_percent.toFixed(0)}%`
      : "--",
    sub: resourceUsage.value
      ? `${resourceUsage.value.memory_used_gb}GB / ${resourceUsage.value.memory_total_gb}GB`
      : "",
    percent: resourceUsage.value ? resourceUsage.value.memory_percent : null,
  },
  {
    id: "disk",
    tkey: "dashboard.disk",
    value: resourceUsage.value
      ? `${resourceUsage.value.disk_percent.toFixed(0)}%`
      : "--",
    sub: resourceUsage.value
      ? `${resourceUsage.value.disk_used_gb}GB / ${resourceUsage.value.disk_total_gb}GB`
      : "",
    percent: resourceUsage.value ? resourceUsage.value.disk_percent : null,
  },
]);

const recentEnvs = computed(() =>
  environments.value.slice(0, 5).map((env) => ({
    name: env.name,
    version: env.version,
    status: env.status === "Active" ? t("dashboard.running") : t("dashboard.stopped"),
    statusColor: env.status === "Active" ? "running" : "stopped",
  }))
);

onMounted(() => {
  loadSystemInfo();
  loadEnvironments();
  timer = setInterval(refreshResourceUsage, 10000);
});

onBeforeUnmount(() => {
  if (timer) clearInterval(timer);
});
</script>

<template>
  <div class="page">
    <!-- Header -->
    <div class="page-header">
      <div>
        <h1 class="page-title">{{ t("dashboard.overview") }}</h1>
        <p class="page-desc">{{ t("dashboard.status_at_a_glance") }}</p>
      </div>
      <a-button type="primary" @click="router.push('/environments')">
        <template #icon><icon-plus /></template>
        {{ t("dashboard.new_environment") }}
      </a-button>
    </div>

    <!-- Stats Cards -->
    <a-row :gutter="16" class="mb-5">
      <a-col v-for="stat in stats" :key="stat.id" :span="8">
        <a-card :bordered="true" class="stat-card">
          <div class="stat-head">
            <span class="stat-label">{{ t(stat.tkey) }}</span>
          </div>
          <div class="stat-value">{{ stat.value }}</div>
          <div class="stat-sub">{{ stat.sub }}</div>
          <a-progress
            v-if="stat.percent !== null"
            :percent="Math.min(stat.percent, 100)"
            :color="progressColor(stat.percent)"
            :show-text="false"
            class="stat-progress"
          />
        </a-card>
      </a-col>
    </a-row>

    <!-- Bottom Section -->
    <a-row :gutter="16">
      <!-- Recently Used -->
      <a-col :span="16">
        <a-card class="section-card" :bordered="true" title=" ">
          <template #title>
            <div class="card-title-row">
              <span>{{ t("dashboard.recently_used") }}</span>
              <a-tag size="small">{{ recentEnvs.length }}</a-tag>
            </div>
          </template>

          <a-empty v-if="recentEnvs.length === 0" :description="t('environments.no_data')">
            <template #image>
              <icon-code />
            </template>
          </a-empty>

          <a-table
            v-else
            :data="recentEnvs"
            :pagination="false"
            :bordered="{ wrapper: false, cell: false }"
            size="small"
          >
            <template #columns>
              <a-table-column title=" " data-index="name">
                <template #cell="{ record }">
                  <span class="env-name">{{ record.name }}</span>
                </template>
              </a-table-column>
              <a-table-column :title="t('version')" data-index="version">
                <template #cell="{ record }">
                  <a-typography-text code>{{ record.version }}</a-typography-text>
                </template>
              </a-table-column>
              <a-table-column :title="t('software.status')" data-index="status">
                <template #cell="{ record }">
                  <span class="env-status">
                    <span class="status-dot" :class="record.statusColor"></span>
                    {{ record.status }}
                  </span>
                </template>
              </a-table-column>
            </template>
          </a-table>
        </a-card>
      </a-col>

      <!-- System Health -->
      <a-col :span="8">
        <a-card class="section-card" :bordered="true">
          <template #title>{{ t("dashboard.system_health") }}</template>

          <a-spin v-if="loading" :loading="true" class="health-spin">
            <div style="height: 120px"></div>
          </a-spin>

          <div v-else-if="error" class="text-muted">{{ error }}</div>

          <template v-else-if="systemInfo && resourceUsage">
            <div class="health-block">
              <div class="health-label">{{ t("dashboard.operating_system") }}</div>
              <div class="health-value">
                {{ systemInfo.os_name }} {{ systemInfo.os_version }}
              </div>
              <div class="health-mono">{{ systemInfo.kernel_version }}</div>
            </div>
            <a-divider class="health-divider" />
            <div class="health-block">
              <div class="health-label">{{ t("dashboard.cpu_usage") }}</div>
              <div class="health-value">{{ resourceUsage.cpu_usage.toFixed(1) }}%</div>
              <a-progress
                :percent="Math.min(resourceUsage.cpu_usage, 100)"
                :color="progressColor(resourceUsage.cpu_usage)"
                :show-text="false"
                class="health-progress"
              />
            </div>
            <a-divider class="health-divider" />
            <div class="health-block">
              <div class="health-label">{{ t("dashboard.system_uptime") }}</div>
              <div class="health-value">{{ formatUptime(resourceUsage.uptime_secs) }}</div>
            </div>
          </template>
        </a-card>
      </a-col>
    </a-row>
  </div>
</template>

<style scoped>
.stat-card {
  border-radius: 10px;
}
.stat-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 6px;
}
.stat-label {
  font-size: 12px;
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--color-text-2);
}
.stat-value {
  font-size: 26px;
  font-weight: 600;
  color: var(--color-text-1);
  letter-spacing: -0.02em;
}
.stat-sub {
  margin-top: 2px;
  font-size: 12px;
  color: var(--color-text-3);
}
.stat-progress {
  margin-top: 12px;
}

.card-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
}
.env-name {
  font-weight: 500;
  color: var(--color-text-1);
}
.env-status {
  display: inline-flex;
  align-items: center;
  font-size: 12px;
  color: var(--color-text-2);
}

.health-spin {
  display: block;
}
.health-block {
  margin-bottom: 2px;
}
.health-label {
  font-size: 10px;
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--color-text-3);
  margin-bottom: 2px;
}
.health-value {
  font-size: 14px;
  color: var(--color-text-1);
}
.health-mono {
  margin-top: 2px;
  font-size: 12px;
  font-family: "JetBrains Mono", monospace;
  color: var(--color-text-3);
}
.health-divider {
  margin: 14px 0;
}
.health-progress {
  margin-top: 8px;
}
</style>

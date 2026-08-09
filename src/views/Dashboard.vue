<script setup>
import { ref, computed, onMounted, onBeforeUnmount } from "vue";
import { useRouter } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import AppIcon from "../components/AppIcon.vue";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyMedia,
} from "@/components/ui/empty";
import {
  Alert,
  AlertDescription,
  AlertTitle,
} from "@/components/ui/alert";
import { Skeleton } from "@/components/ui/skeleton";
import { t } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";

const router = useRouter();

const systemInfo = ref(null);
const resourceUsage = ref(null);
const hardwareStatus = ref(null);
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
    await loadHardwareStatus();
  } catch (err) {
    error.value = friendlyError(err);
  } finally {
    loading.value = false;
  }
}

async function loadHardwareStatus() {
  try {
    hardwareStatus.value = await invoke("get_hardware_status");
  } catch (err) {
    // 硬件状态为附加信息，失败不阻塞整页加载
    console.error("Failed to load hardware status:", err);
  }
}

async function refreshResourceUsage() {
  try {
    resourceUsage.value = await invoke("get_resource_usage");
  } catch (err) {
    console.error("Failed to refresh resource usage:", err);
  }
  loadHardwareStatus();
}

async function loadEnvironments() {
  try {
    environments.value = await invoke("list_environments");
  } catch (err) {
    console.error("Failed to load environments:", err);
  }
}

// 进度条指示器颜色：Tailwind arbitrary variant 覆盖 Progress 内部 indicator
function progressColorClass(val) {
  if (val > 80) return "[&>div]:bg-danger";
  if (val > 60) return "[&>div]:bg-warning";
  return "[&>div]:bg-success";
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
  {
    id: "temp",
    tkey: "dashboard.cpu_temp",
    value:
      hardwareStatus.value?.cpu_temp_c != null
        ? `${hardwareStatus.value.cpu_temp_c.toFixed(0)}°C`
        : "--",
    sub: "",
    percent: null,
  },
  {
    id: "gpu",
    tkey: "dashboard.gpu",
    value:
      hardwareStatus.value?.gpu_memory_total_mb &&
      hardwareStatus.value?.gpu_memory_used_mb != null
        ? `${((hardwareStatus.value.gpu_memory_used_mb / hardwareStatus.value.gpu_memory_total_mb) * 100).toFixed(0)}%`
        : "--",
    sub: hardwareStatus.value
      ? [
          hardwareStatus.value.gpu_name,
          hardwareStatus.value.gpu_memory_total_mb
            ? `${(hardwareStatus.value.gpu_memory_used_mb / 1024).toFixed(1)}GB / ${(hardwareStatus.value.gpu_memory_total_mb / 1024).toFixed(0)}GB`
            : "",
        ]
          .filter(Boolean)
          .join(" · ")
      : "",
    percent:
      hardwareStatus.value?.gpu_memory_total_mb &&
      hardwareStatus.value?.gpu_memory_used_mb != null
        ? (hardwareStatus.value.gpu_memory_used_mb /
            hardwareStatus.value.gpu_memory_total_mb) *
          100
        : null,
  },
  {
    id: "battery",
    tkey: "dashboard.battery",
    value:
      hardwareStatus.value?.battery_percent != null
        ? `${hardwareStatus.value.battery_percent.toFixed(0)}%`
        : "--",
    sub: hardwareStatus.value?.battery_status
      ? batteryStatusLabel(hardwareStatus.value.battery_status)
      : "",
    percent:
      hardwareStatus.value?.battery_percent != null
        ? hardwareStatus.value.battery_percent
        : null,
  },
]);

function batteryStatusLabel(status) {
  const key = {
    Charging: "dashboard.battery_charging",
    Discharging: "dashboard.battery_discharging",
    Full: "dashboard.battery_full",
  }[status];
  return key ? t(key) : status;
}

const recentEnvs = computed(() =>
  environments.value.slice(0, 5).map((env) => ({
    name: env.name,
    version: env.version,
    status: env.status === "Active" ? t("dashboard.running") : t("dashboard.stopped"),
    running: env.status === "Active",
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
      <Button @click="router.push('/environments')">
        <AppIcon name="plus" />
        {{ t("dashboard.new_environment") }}
      </Button>
    </div>

    <!-- Stats Cards -->
    <div class="grid grid-cols-1 gap-4 md:grid-cols-3 mb-3">
      <Card
        v-for="stat in stats"
        :key="stat.id"
        class="shadow-sm transition-shadow hover:shadow-md"
      >
        <CardContent class="pt-6">
          <div class="stat-label">{{ t(stat.tkey) }}</div>
          <div class="stat-value">{{ stat.value }}</div>
          <div class="stat-sub">{{ stat.sub }}</div>
          <Progress
            v-if="stat.percent !== null"
            :model-value="Math.min(stat.percent, 100)"
            :class="['stat-progress', progressColorClass(stat.percent)]"
          />
        </CardContent>
      </Card>
    </div>

    <!-- Bottom Section -->
    <div class="grid grid-cols-1 gap-4 lg:grid-cols-3">
      <!-- Recently Used -->
      <Card class="shadow-sm lg:col-span-2">
        <CardHeader class="flex-row items-center justify-between space-y-0">
          <CardTitle class="text-base font-medium">
            {{ t("dashboard.recently_used") }}
          </CardTitle>
          <Badge variant="secondary">{{ recentEnvs.length }}</Badge>
        </CardHeader>
        <CardContent class="pb-2">
          <Empty v-if="recentEnvs.length === 0" class="px-4 py-6 md:px-4 md:py-6">
            <EmptyMedia>
              <AppIcon name="code" class="size-10 text-muted-foreground/60" />
            </EmptyMedia>
            <EmptyContent>
              <EmptyDescription>
                {{ t("environments.no_data") }}
              </EmptyDescription>
            </EmptyContent>
          </Empty>

          <Table v-else class="recent-table">
            <TableHeader>
              <TableRow>
                <TableHead class="h-8">{{ t("nav.environments") }}</TableHead>
                <TableHead class="h-8">{{ t("version") }}</TableHead>
                <TableHead class="h-8 min-w-20 text-right whitespace-nowrap">{{ t("software.status") }}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-for="env in recentEnvs" :key="env.name">
                <TableCell class="font-medium">{{ env.name }}</TableCell>
                <TableCell>
                  <code class="rounded bg-muted px-1.5 py-0.5 font-mono text-xs">
                    {{ env.version }}
                  </code>
                </TableCell>
                <TableCell class="text-right whitespace-nowrap">
                  <span class="inline-flex items-center gap-1.5 text-sm text-muted-foreground whitespace-nowrap">
                    <span
                      class="status-dot"
                      :class="env.running ? 'running' : 'stopped'"
                    ></span>
                    {{ env.status }}
                  </span>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      <!-- System Health -->
      <Card class="shadow-sm">
        <CardHeader>
          <CardTitle class="text-base font-medium">
            {{ t("dashboard.system_health") }}
          </CardTitle>
        </CardHeader>
        <CardContent>
          <!-- Loading -->
          <div v-if="loading" class="space-y-3 py-2">
            <Skeleton class="h-4 w-32" />
            <Skeleton class="h-4 w-40" />
            <Skeleton class="h-2 w-full" />
            <Skeleton class="h-4 w-24" />
          </div>

          <!-- Error -->
          <Alert v-else-if="error" variant="destructive">
            <AppIcon name="close-circle-fill" class="size-4" />
            <AlertTitle>{{ t("error.title") }}</AlertTitle>
            <AlertDescription>{{ error }}</AlertDescription>
          </Alert>

          <!-- Data -->
          <template v-else-if="systemInfo && resourceUsage">
            <div class="space-y-3">
              <div>
                <div class="health-label">{{ t("dashboard.operating_system") }}</div>
                <div class="health-value">
                  {{ systemInfo.os_name }} {{ systemInfo.os_version }}
                </div>
                <div class="health-mono">{{ systemInfo.kernel_version }}</div>
              </div>

              <Separator />

              <div>
                <div class="health-label">{{ t("dashboard.cpu_usage") }}</div>
                <div class="health-value">
                  {{ resourceUsage.cpu_usage.toFixed(1) }}%
                </div>
                <Progress
                  :model-value="Math.min(resourceUsage.cpu_usage, 100)"
                  :class="['mt-2 h-2', progressColorClass(resourceUsage.cpu_usage)]"
                />
              </div>

              <Separator />

              <div>
                <div class="health-label">{{ t("dashboard.system_uptime") }}</div>
                <div class="health-value">
                  {{ formatUptime(resourceUsage.uptime_secs) }}
                </div>
              </div>
            </div>
          </template>
        </CardContent>
      </Card>
    </div>
  </div>
</template>

<style scoped>
.stat-label {
  font-size: 12px;
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--color-muted-foreground);
  margin-bottom: 6px;
}

.stat-value {
  font-size: 26px;
  font-weight: 600;
  color: var(--color-foreground);
  letter-spacing: -0.02em;
}

.stat-sub {
  margin-top: 2px;
  font-size: 12px;
  color: var(--color-muted-foreground);
}

.stat-progress {
  margin-top: 12px;
}

.health-label {
  font-size: 10px;
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--color-muted-foreground);
  margin-bottom: 2px;
}

.health-value {
  font-size: 14px;
  color: var(--color-foreground);
}

.health-mono {
  margin-top: 2px;
  font-size: 12px;
  font-family: "JetBrains Mono", monospace;
  color: var(--color-muted-foreground);
}

</style>
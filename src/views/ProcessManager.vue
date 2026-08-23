<script setup>
import { ref, computed, onMounted, onBeforeUnmount, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { showToast } from "../lib/toast.js";
import { showConfirm } from "../lib/confirm.js";
import { t } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";
import AppIcon from "../components/AppIcon.vue";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Badge } from "@/components/ui/badge";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
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
import { Spinner } from "@/components/ui/spinner";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

const groups = ref([]);
const total = ref(0);
const loading = ref(true);
const error = ref(null);
const search = ref("");
const sortBy = ref("memory");
const sortAsc = ref(false);
const autoRefresh = ref(false);
const killing = ref(null); // PID being killed
const expanded = ref(new Set());
let loadInProgress = false;
let refreshInterval = null;

async function loadProcesses() {
  if (loadInProgress) return;
  loadInProgress = true;
  try {
    loading.value = true;
    error.value = null;
    const result = await invoke("list_processes");
    groups.value = result.groups;
    total.value = result.total;
  } catch (err) {
    error.value = friendlyError(err);
  } finally {
    loading.value = false;
    loadInProgress = false;
  }
}

async function killProcess(pid) {
  if (!(await showConfirm(t("process.kill_force_confirm").replace("{pid}", pid)))) return;
  killing.value = pid;
  try {
    const msg = await invoke("kill_process_force", { pid });
    showToast(msg, "success");
    await loadProcesses();
  } catch (err) {
    showToast(`${t("process.kill_failed")}: ${friendlyError(err)}`, "error");
  } finally {
    killing.value = null;
  }
}

async function terminateProcess(pid) {
  if (
    !(await showConfirm(
      t("process.kill_confirm").replace("{name}", `PID ${pid}`).replace("{count}", "1")
    ))
  )
    return;
  killing.value = pid;
  try {
    const msg = await invoke("kill_process", { pid });
    showToast(msg, "success");
    await loadProcesses();
  } catch (err) {
    showToast(`${t("process.kill_failed")}: ${friendlyError(err)}`, "error");
  } finally {
    killing.value = null;
  }
}

async function killGroup(name, count) {
  if (
    !(await showConfirm(t("process.kill_confirm").replace("{name}", name).replace("{count}", count)))
  )
    return;
  try {
    const group = groups.value.find((g) => g.name === name);
    if (!group) return;
    let killErrors = [];
    for (const entry of group.entries) {
      try {
        await invoke("kill_process_force", { pid: entry.pid });
      } catch {
        killErrors.push(entry.pid);
      }
    }
    if (killErrors.length > 0) {
      showToast(
        t("common.error_msg").replace("{error}", `PID(s): ${killErrors.join(", ")}`),
        "warning"
      );
    } else {
      showToast(t("process.kill_success").replace("{name}", name), "success");
    }
    await loadProcesses();
  } catch (err) {
    showToast(`${t("process.kill_failed")}: ${friendlyError(err)}`, "error");
  }
}

async function killPortAction(port) {
  if (!(await showConfirm(t("ports.kill_confirm").replace("{port}", port)))) return;
  try {
    const result = await invoke("kill_port", { port });
    showToast(result, "success");
    await loadProcesses();
  } catch (err) {
    showToast(`${t("process.kill_failed")}: ${friendlyError(err)}`, "error");
  }
}

function toggleExpand(name) {
  const next = new Set(expanded.value);
  if (next.has(name)) next.delete(name);
  else next.add(name);
  expanded.value = next;
}

function toggleSort(field) {
  if (sortBy.value === field) {
    sortAsc.value = !sortAsc.value;
  } else {
    sortBy.value = field;
    sortAsc.value = field === "name";
  }
}

function toggleAutoRefresh() {
  autoRefresh.value = !autoRefresh.value;
  if (autoRefresh.value) {
    refreshInterval = setInterval(() => {
      if (document.hidden) return;
      loadProcesses();
    }, 5000);
  } else if (refreshInterval) {
    clearInterval(refreshInterval);
    refreshInterval = null;
  }
}

function formatTime(secs) {
  if (!secs) return "—";
  const now = Date.now() / 1000;
  const diff = now - secs;
  if (diff < 60) return `${Math.floor(diff)}s`;
  if (diff < 3600) return `${Math.floor(diff / 60)}m`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h`;
  return `${Math.floor(diff / 86400)}d`;
}

function formatMemory(bytes) {
  if (bytes >= 1073741824) return `${(bytes / 1073741824).toFixed(1)} GB`;
  if (bytes >= 1048576) return `${(bytes / 1048576).toFixed(1)} MB`;
  return `${(bytes / 1024).toFixed(1)} KB`;
}

function cpuClass(cpu) {
  if (cpu > 50) return "cpu-high";
  if (cpu > 20) return "cpu-mid";
  return "cpu-low";
}

const debouncedSearch = ref(search.value);
let searchTimer = null;
watch(search, (v) => {
  clearTimeout(searchTimer);
  searchTimer = setTimeout(() => { debouncedSearch.value = v; }, 150);
});
const filtered = computed(() =>
  debouncedSearch.value.trim()
    ? groups.value.filter(
        (g) =>
          g.name.toLowerCase().includes(debouncedSearch.value.toLowerCase()) ||
          g.ports.some((p) => p.toString().includes(debouncedSearch.value)) ||
          g.entries.some((e) => e.pid.toString().includes(debouncedSearch.value))
      )
    : groups.value
);

const sorted = computed(() => {
  const arr = [...filtered.value];
  const cmp = (a, b) => {
    let va, vb;
    switch (sortBy.value) {
      case "memory":
        va = a.total_memory_bytes;
        vb = b.total_memory_bytes;
        break;
      case "cpu":
        va = a.total_cpu;
        vb = b.total_cpu;
        break;
      case "name":
        va = a.name;
        vb = b.name;
        break;
      case "count":
        va = a.count;
        vb = b.count;
        break;
      default:
        va = a.total_memory_bytes;
        vb = b.total_memory_bytes;
    }
    if (typeof va === "string") return sortAsc.value ? va.localeCompare(vb) : vb.localeCompare(va);
    return sortAsc.value ? va - vb : vb - va;
  };
  return arr.sort(cmp);
});

const pageSize = 50;
const visibleCount = ref(pageSize);
const visibleSorted = computed(() => sorted.value.slice(0, visibleCount.value));
function loadMoreProcesses() { visibleCount.value += pageSize; }

onMounted(() => {
  loadProcesses();
  document.addEventListener("visibilitychange", handleVis);
});

onBeforeUnmount(() => {
  if (refreshInterval) clearInterval(refreshInterval);
  document.removeEventListener("visibilitychange", handleVis);
});

function handleVis() {
  if (!document.hidden && autoRefresh.value) loadProcesses();
}
</script>

<template>
  <div class="page process-page">
    <!-- 标题栏 -->
    <div class="page-header">
      <div>
        <h1 class="page-title">{{ t("process.title") }} [LIVE]</h1>
        <p class="page-desc">{{ t("process.desc") }}</p>
      </div>
      <div class="flex gap-2 items-center">
        <Button variant="outline" size="sm" @click="toggleAutoRefresh">
          <AppIcon v-if="autoRefresh" name="pause" />
          <AppIcon v-else name="play-arrow" />
          {{ t("process.auto_refresh") }}
        </Button>
        <Button variant="outline" :disabled="loading" @click="loadProcesses">
          <AppIcon name="refresh" :spin="loading" />
          {{ t("process.refresh") }}
        </Button>
      </div>
    </div>

    <!-- 搜索栏 + 排序 -->
    <div class="mb-4 flex items-center gap-3">
      <div class="relative flex-1">
        <AppIcon
          name="search"
          class="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
        />
        <Input v-model="search" :placeholder="t('process.search_placeholder')" class="pl-8" />
        <button
          v-if="search"
          class="absolute right-2.5 top-1/2 -translate-y-1/2 text-muted-foreground transition-colors hover:text-foreground"
          @click="search = ''"
        >
          <AppIcon name="close" class="size-3.5" />
        </button>
      </div>
      <Select v-model="sortBy">
        <SelectTrigger class="w-[140px]">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="memory">{{ t("process.sort_memory") }}</SelectItem>
          <SelectItem value="cpu">{{ t("process.sort_cpu") }}</SelectItem>
          <SelectItem value="count">{{ t("process.sort_count") }}</SelectItem>
          <SelectItem value="name">{{ t("process.sort_name") }}</SelectItem>
        </SelectContent>
      </Select>
      <Button variant="outline" size="icon" @click="sortAsc = !sortAsc">
        <AppIcon v-if="sortAsc" name="arrow-up" />
        <AppIcon v-else name="arrow-down" />
      </Button>
    </div>

    <!-- 进程表格 -->
    <!-- 加载态 -->
    <div v-if="loading && groups.length === 0" class="space-y-3 py-2">
      <Skeleton class="h-10 w-full" />
      <Skeleton class="h-10 w-full" />
      <Skeleton class="h-10 w-full" />
    </div>

    <!-- 错误 -->
    <Alert v-else-if="error" variant="destructive" class="py-4">
      <AppIcon name="close-circle-fill" class="size-4" />
      <AlertTitle>{{ t("error.title") }}</AlertTitle>
      <AlertDescription>{{ error }}</AlertDescription>
      <Button variant="outline" size="sm" class="mt-2" @click="loadProcesses">
        {{ t("common.retry") }}
      </Button>
    </Alert>

    <!-- 空 -->
    <Empty v-else-if="filtered.length === 0" class="py-14">
      <EmptyMedia>
        <AppIcon name="code" class="size-10 text-muted-foreground/60" />
      </EmptyMedia>
      <EmptyContent>
        <EmptyDescription>
          {{ search ? t('process.no_matching') : t('process.no_processes') }}
        </EmptyDescription>
      </EmptyContent>
    </Empty>

    <Card v-else class="shadow-sm">
      <CardContent class="p-0">
        <TooltipProvider>
          <Table class="process-table" style="table-layout: auto; width: 100%">
            <TableHeader>
              <TableRow>
                <TableHead class="col-name" style="width: 100%">{{ t("process.name") }}</TableHead>
                <TableHead class="text-right" style="white-space: nowrap; min-width: 70px">{{ t("process.instances") }}</TableHead>
                <TableHead class="text-right" style="white-space: nowrap; min-width: 60px">CPU</TableHead>
                <TableHead class="text-right" style="white-space: nowrap; min-width: 80px">{{ t("process.memory") }}</TableHead>
                <TableHead style="white-space: nowrap; min-width: 80px">{{ t("process.ports") }}</TableHead>
                <TableHead class="text-right" style="white-space: nowrap; min-width: 80px">{{ t("process.elapsed") }}</TableHead>
                <TableHead class="text-right" style="white-space: nowrap; min-width: 90px">{{ t("process.actions") }}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <template v-for="record in visibleSorted" :key="record.name">
                <TableRow class="proc-row" :class="{ open: expanded.has(record.name) }">
                <TableCell class="col-name" style="width: 100%">
                  <div class="proc-name-row" @click="toggleExpand(record.name)">
                    <AppIcon
                      v-if="!expanded.has(record.name)"
                      name="right"
                      class="chevron size-3.5 text-muted-foreground"
                    />
                    <AppIcon
                      v-else
                      name="down"
                      class="chevron size-3.5 text-muted-foreground"
                    />
                    <span class="proc-name">{{ record.name }}</span>
                    <Badge
                      v-if="record.count > 1"
                      variant="secondary"
                      class="h-5 px-1.5 py-0 text-[10px]"
                    >
                      ×{{ record.count }}
                    </Badge>
                  </div>
                </TableCell>
                <TableCell class="text-right" style="white-space: nowrap; min-width: 70px">
                  <span class="num">{{ record.count }}</span>
                </TableCell>
                <TableCell class="text-right" style="white-space: nowrap; min-width: 60px">
                  <span class="cpu-num" :class="cpuClass(record.total_cpu)">
                    {{ record.total_cpu.toFixed(1) }}%
                  </span>
                </TableCell>
                <TableCell class="text-right" style="white-space: nowrap; min-width: 80px">
                  <Tooltip>
                    <TooltipTrigger as-child>
                      <span class="num cursor-default" style="white-space: nowrap">
                        {{ formatMemory(Math.round(record.total_memory_bytes / record.count)) }}
                      </span>
                    </TooltipTrigger>
                    <TooltipContent>
                      {{ t('process.memory_total') }} {{ formatMemory(record.total_memory_bytes) }}
                    </TooltipContent>
                  </Tooltip>
                </TableCell>
                <TableCell style="white-space: nowrap; min-width: 80px">
                  <div v-if="record.ports.length > 0" class="ports-row">
                    <Badge
                      v-for="port in record.ports"
                      :key="port"
                      variant="outline"
                      class="h-5 gap-1 rounded-full px-2 py-0 font-mono text-xs font-normal"
                    >
                      {{ port }}
                      <button
                        class="text-muted-foreground transition-colors hover:text-destructive"
                        @click="killPortAction(port)"
                      >
                        <AppIcon name="close" class="size-3" />
                      </button>
                    </Badge>
                  </div>
                  <span v-else class="muted">—</span>
                </TableCell>
                <TableCell class="text-right" style="white-space: nowrap; min-width: 80px">
                  <span class="muted">{{ formatTime(record.earliest_start) }}</span>
                </TableCell>
                <TableCell class="text-right" style="white-space: nowrap; min-width: 90px">
                  <Button
                    variant="destructive"
                    size="sm"
                    @click="killGroup(record.name, record.count)"
                  >
                    {{ t("process.kill_all") }}
                  </Button>
                </TableCell>
                </TableRow>

                <!-- 展开的子进程详情：表格内插入行，紧贴所属分组 -->
                <TableRow v-if="expanded.has(record.name)" class="child-tr">
                  <TableCell colspan="7" class="child-cell">
                    <div class="child-panel">
                      <div v-for="entry in record.entries" :key="entry.pid" class="child-row">
                        <span class="child-pid">PID {{ entry.pid }}</span>
                        <span class="child-cpu">{{ entry.cpu_usage.toFixed(1) }}%</span>
                        <span class="child-mem">{{ formatMemory(entry.memory_bytes) }}</span>
                        <span class="child-time">{{ formatTime(entry.start_time_secs) }}</span>
                        <div class="child-actions">
                          <Button
                            variant="outline"
                            size="sm"
                            :disabled="killing === entry.pid"
                            @click="terminateProcess(entry.pid)"
                          >
                            <Spinner v-if="killing === entry.pid" class="size-3.5" />
                            {{ t("process.kill") }}
                          </Button>
                          <Button
                            variant="destructive"
                            size="sm"
                            :disabled="killing === entry.pid"
                            @click="killProcess(entry.pid)"
                          >
                            <Spinner v-if="killing === entry.pid" class="size-3.5" />
                            {{ t("process.kill_force") }}
                          </Button>
                        </div>
                      </div>
                    </div>
                  </TableCell>
                </TableRow>
              </template>
            </TableBody>
          </Table>
        </TooltipProvider>

        <!-- 表格底部 -->
        <div class="table-footer">
          <span class="footer-text">
            {{ filtered.length }} {{ t("process.groups") }} · {{ total }} {{ t("process.total_processes") }} · 显示 {{ visibleSorted.length }}/{{ sorted.length }}
          </span>
          <div class="flex gap-2">
            <Button v-if="visibleSorted.length < sorted.length" variant="outline" size="sm" @click="loadMoreProcesses">{{ t("common.load_more") || "Load more" }}</Button>
            <Button variant="outline" size="sm" @click="loadProcesses">
              {{ t("process.refresh") }}
            </Button>
          </div>
        </div>
      </CardContent>
    </Card>
  </div>
</template>

<style scoped>
/* 兜底：强制表格紧凑、不竖排（HMR 失效时重启仍生效） */
.process-table {
  table-layout: auto;
  width: 100%;
}
.process-table :deep(th),
.process-table :deep(td) {
  padding: 6px 10px !important;
  white-space: nowrap !important;
  word-break: keep-all !important;
  writing-mode: horizontal-tb !important;
  text-orientation: mixed !important;
  overflow: hidden;
  text-overflow: ellipsis;
}
.process-table :deep(th) {
  height: 32px !important;
}
.process-table :deep(.col-name) {
  width: 100%;
  min-width: 200px;
}
/* 给右侧短列一个最小宽度，避免被压成竖排 */
.process-table :deep(th):not(.col-name),
.process-table :deep(td):not(.col-name) {
  min-width: 70px;
}
.proc-name-row {
  display: flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
  user-select: none;
}

.proc-name-row:hover .proc-name {
  color: var(--color-primary);
}

.chevron {
  transition: transform 0.15s ease;
}

.proc-name {
  font-weight: 500;
  color: var(--color-foreground);
}

.proc-row.open {
  background-color: var(--color-muted);
}

/* 展开详情行：去掉单元格内边距与行边框，整体作为父行的延伸 */
.child-tr :deep(td) {
  padding: 0;
  border-bottom-width: 0;
  background-color: var(--color-muted);
}

.child-tr:hover {
  background-color: transparent;
}

.child-panel {
  padding: 4px 16px 10px 40px;
}

.child-row {
  display: flex;
  align-items: center;
  gap: 24px;
  padding: 5px 0;
  font-size: 12px;
  color: var(--color-muted-foreground);
  font-family: "JetBrains Mono", monospace;
  border-top: 1px dashed var(--color-border);
}

.child-row:first-child {
  border-top: none;
}
.num {
  font-family: "JetBrains Mono", monospace;
  font-size: 13px;
  color: var(--color-muted-foreground);
}
.cpu-num {
  font-family: "JetBrains Mono", monospace;
  font-size: 13px;
}
.cpu-low {
  color: var(--color-muted-foreground);
}
.cpu-mid {
  color: var(--color-warning);
}
.cpu-high {
  color: var(--color-danger);
}
.ports-row {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.muted {
  color: var(--color-muted-foreground);
  font-size: 12px;
}

.child-pid {
  width: 120px;
}
.child-cpu {
  width: 70px;
}
.child-mem {
  width: 100px;
}
.child-time {
  flex: 1;
}
.child-actions {
  display: flex;
  gap: 8px;
}
.table-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 12px;
  border-top: 1px solid var(--color-border);
}
.footer-text {
  font-size: 12px;
  color: var(--color-muted-foreground);
}
</style>
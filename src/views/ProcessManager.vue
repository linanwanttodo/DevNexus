<script setup>
import { ref, computed, onMounted, onBeforeUnmount } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { showToast } from "../lib/toast.js";
import { showConfirm } from "../lib/confirm.js";
import { t } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";

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
    refreshInterval = setInterval(loadProcesses, 3000);
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

const filtered = computed(() =>
  search.value.trim()
    ? groups.value.filter(
        (g) =>
          g.name.toLowerCase().includes(search.value.toLowerCase()) ||
          g.ports.some((p) => p.toString().includes(search.value)) ||
          g.entries.some((e) => e.pid.toString().includes(search.value))
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

onMounted(() => {
  loadProcesses();
});

onBeforeUnmount(() => {
  if (refreshInterval) clearInterval(refreshInterval);
});
</script>

<template>
  <div class="page process-page">
    <!-- 标题栏 -->
    <div class="page-header">
      <div>
        <h1 class="page-title">{{ t("process.title") }}</h1>
        <p class="page-desc">{{ t("process.desc") }}</p>
      </div>
      <div class="flex gap-2 items-center">
        <a-button size="small" @click="toggleAutoRefresh">
          <template #icon>
            <icon-pause v-if="autoRefresh" />
            <icon-play-arrow v-else />
          </template>
          {{ t("process.auto_refresh") }}
        </a-button>
        <a-button :loading="loading" @click="loadProcesses">
          <template #icon><icon-refresh /></template>
          {{ t("process.refresh") }}
        </a-button>
      </div>
    </div>

    <!-- 搜索栏 + 排序 -->
    <div class="toolbar">
      <a-input v-model="search" :placeholder="t('process.search_placeholder')" allow-clear class="search-input">
        <template #prefix><icon-search /></template>
      </a-input>
      <a-select v-model="sortBy" style="width: 140px">
        <a-option value="memory">{{ t("process.sort_memory") }}</a-option>
        <a-option value="cpu">{{ t("process.sort_cpu") }}</a-option>
        <a-option value="count">{{ t("process.sort_count") }}</a-option>
        <a-option value="name">{{ t("process.sort_name") }}</a-option>
      </a-select>
      <a-button @click="sortAsc = !sortAsc">
        <template #icon>
          <icon-arrow-up v-if="sortAsc" />
          <icon-arrow-down v-else />
        </template>
      </a-button>
    </div>

    <!-- 进程表格 -->
    <a-spin :loading="loading && groups.length === 0" style="width: 100%">
      <a-result v-if="error" status="error" :title="error" style="padding: 40px 0">
        <template #extra>
          <a-button type="primary" @click="loadProcesses">{{ t("common.retry") }}</a-button>
        </template>
      </a-result>

      <a-empty
        v-else-if="filtered.length === 0"
        style="padding: 56px 0"
        :description="search ? t('process.no_matching') : t('process.no_processes')"
      />

      <a-card v-else :bordered="true" class="process-card">
        <a-table
          :data="sorted"
          :pagination="false"
          :bordered="false"
          :row-key="'name'"
          size="small"
          class="process-table"
        >
          <template #columns>
            <a-table-column data-index="name">
              <template #cell="{ record }">
                <div class="proc-name-row" @click="toggleExpand(record.name)">
                  <icon-right v-if="!expanded.has(record.name)" class="chevron" />
                  <icon-down v-else class="chevron" />
                  <span class="proc-name">{{ record.name }}</span>
                  <a-tag v-if="record.count > 1" size="mini">×{{ record.count }}</a-tag>
                </div>
              </template>
            </a-table-column>
            <a-table-column :title="t('process.instances')" data-index="count" align="right">
              <template #cell="{ record }">
                <span class="num">{{ record.count }}</span>
              </template>
            </a-table-column>
            <a-table-column title="CPU" align="right">
              <template #cell="{ record }">
                <span class="cpu-num" :class="cpuClass(record.total_cpu)">
                  {{ record.total_cpu.toFixed(1) }}%
                </span>
              </template>
            </a-table-column>
            <a-table-column :title="t('process.memory')" align="right">
              <template #cell="{ record }">
                <a-tooltip :content="`${t('process.memory_total')} ${formatMemory(record.total_memory_bytes)}`">
                  <span class="num">{{ formatMemory(Math.round(record.total_memory_bytes / record.count)) }}</span>
                </a-tooltip>
              </template>
            </a-table-column>
            <a-table-column :title="t('process.ports')">
              <template #cell="{ record }">
                <div v-if="record.ports.length > 0" class="ports-row">
                  <a-tag v-for="port in record.ports" :key="port" size="mini" color="arcoblue" closable @close="killPortAction(port)">
                    {{ port }}
                  </a-tag>
                </div>
                <span v-else class="muted">—</span>
              </template>
            </a-table-column>
            <a-table-column :title="t('process.elapsed')" align="right">
              <template #cell="{ record }">
                <span class="muted">{{ formatTime(record.earliest_start) }}</span>
              </template>
            </a-table-column>
            <a-table-column :title="t('process.actions')" align="right" :width="110">
              <template #cell="{ record }">
                <a-button size="mini" status="danger" @click="killGroup(record.name, record.count)">
                  {{ t("process.kill_all") }}
                </a-button>
              </template>
            </a-table-column>
          </template>
        </a-table>

        <!-- 展开的子进程详情 -->
        <template v-for="group in sorted" :key="group.name">
          <div v-if="expanded.has(group.name)" class="child-panel">
            <div class="child-head">
              <span class="child-title">{{ group.name }}</span>
            </div>
            <div v-for="entry in group.entries" :key="entry.pid" class="child-row">
              <span class="child-pid">PID {{ entry.pid }}</span>
              <span class="child-cpu">{{ entry.cpu_usage.toFixed(1) }}%</span>
              <span class="child-mem">{{ formatMemory(entry.memory_bytes) }}</span>
              <span class="child-time">{{ formatTime(entry.start_time_secs) }}</span>
              <div class="child-actions">
                <a-button
                  size="mini"
                  @click="terminateProcess(entry.pid)"
                  :loading="killing === entry.pid"
                >
                  {{ t("process.kill") }}
                </a-button>
                <a-button
                  size="mini"
                  status="danger"
                  @click="killProcess(entry.pid)"
                  :loading="killing === entry.pid"
                >
                  {{ t("process.kill_force") }}
                </a-button>
              </div>
            </div>
          </div>
        </template>

        <template #footer>
          <div class="table-footer">
            <span class="footer-text">
              {{ filtered.length }} {{ t("process.groups") }} · {{ total }} {{ t("process.total_processes") }}
            </span>
            <a-button size="small" @click="loadProcesses">{{ t("process.refresh") }}</a-button>
          </div>
        </template>
      </a-card>
    </a-spin>
  </div>
</template>

<style scoped>
.process-page {
  padding: 20px 24px;
  max-width: 1100px;
  margin: 0 auto;
}
.toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
}
.search-input {
  flex: 1;
}
.proc-name-row {
  display: flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
}
.chevron {
  color: var(--color-text-4);
  font-size: 14px;
  transition: transform 0.2s ease;
}
.proc-name {
  font-weight: 500;
  color: var(--color-text-1);
}
.num {
  font-family: "JetBrains Mono", monospace;
  font-size: 13px;
  color: var(--color-text-2);
}
.cpu-num {
  font-family: "JetBrains Mono", monospace;
  font-size: 13px;
}
.cpu-low {
  color: var(--color-text-2);
}
.cpu-mid {
  color: rgb(var(--orange-6));
}
.cpu-high {
  color: rgb(var(--red-6));
}
.ports-row {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.muted {
  color: var(--color-text-3);
  font-size: 12px;
}
.child-panel {
  border-top: 1px solid var(--color-border);
  background-color: var(--color-fill-1);
  padding: 8px 16px;
}
.child-head {
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--color-text-3);
  margin-bottom: 6px;
}
.child-row {
  display: flex;
  align-items: center;
  gap: 24px;
  padding: 4px 0;
  font-size: 12px;
  color: var(--color-text-3);
  font-family: "JetBrains Mono", monospace;
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
  padding-top: 8px;
}
.footer-text {
  font-size: 12px;
  color: var(--color-text-3);
}
</style>

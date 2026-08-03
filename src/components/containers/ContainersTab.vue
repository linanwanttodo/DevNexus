<script setup>
import { computed } from "vue";
import { t } from "../../lib/i18n.js";
import ContainerIcons from "../../icons/ContainerIcons.vue";

const props = defineProps({
  items: { type: Array, default: () => [] },
  loading: { type: Boolean, default: false },
  error: { type: String, default: null },
  search: { type: String, default: "" },
  showAll: { type: Boolean, default: false },
  actionLoading: { type: String, default: "" },
});

const emit = defineEmits([
  "show-all-change",
  "refresh",
  "action",
  "logs",
  "terminal",
]);

function shortId(id) {
  return id ? id.substring(0, 12) : "";
}

const statusIcon = (status) =>
  status === "running"
    ? "container-running"
    : status === "paused"
      ? "container-paused"
      : "container-exited";

const columns = computed(() => [
  { title: "", slotName: "status", width: 40 },
  { title: t("docker.name"), slotName: "name" },
  { title: t("docker.image"), slotName: "image" },
  { title: t("docker.ports"), slotName: "ports" },
  { title: t("docker.created"), slotName: "created", width: 130 },
  { title: t("docker.actions"), slotName: "actions", align: "right", width: 320 },
]);
</script>

<template>
  <div>
    <div class="flex items-center justify-between mb-4">
      <a-checkbox :model-value="showAll" @change="(c) => emit('show-all-change', c)">
        {{ t("docker.show_all") }}
      </a-checkbox>
      <a-button size="small" @click="emit('refresh')" :disabled="loading">
        <template #icon>
          <icon-refresh :spin="loading" />
        </template>
        {{ t("common.refresh") }}
      </a-button>
    </div>

    <a-card :bordered="true" class="section-card">
      <a-spin :loading="loading && items.length === 0" class="w-full">
        <a-alert
          v-if="error"
          type="error"
          :message="error"
          action=" "
        >
          <template #action>
            <a-button size="mini" @click="emit('refresh')">{{ t("common.retry") }}</a-button>
          </template>
        </a-alert>

        <a-empty
          v-else-if="items.length === 0"
          :description="search ? t('docker.no_matching') : t('docker.no_containers')"
        >
          <template #image>
            <ContainerIcons name="container" :size="36" class="empty-icon" />
          </template>
        </a-empty>

        <a-table
          v-else
          :data="items"
          :columns="columns"
          :pagination="false"
          :bordered="{ wrapper: false, cell: false }"
          :loading="loading"
          size="small"
          :row-key="(r) => r.id"
        >
          <template #status="{ record }">
            <ContainerIcons :name="statusIcon(record.status)" :size="16" />
          </template>
          <template #name="{ record }">
            <div class="cell-stack">
              <span class="cell-name">{{ record.name }}</span>
              <span class="cell-mono">{{ shortId(record.id) }}</span>
            </div>
          </template>
          <template #image="{ record }">
            <span class="cell-mono">{{ record.image }}</span>
          </template>
          <template #ports="{ record }">
            <span class="cell-mono cell-muted">{{ record.ports || "-" }}</span>
          </template>
          <template #created="{ record }">
            <span class="cell-muted">{{ record.created || "-" }}</span>
          </template>
          <template #actions="{ record }">
            <div class="actions-row">
              <a-button
                v-if="record.status === 'running'"
                size="mini"
                status="warning"
                :disabled="actionLoading === record.name"
                @click="emit('action', record.name, 'pause')"
              >{{ t("docker.pause") }}</a-button>
              <a-button
                v-if="record.status === 'running'"
                size="mini"
                status="danger"
                :disabled="actionLoading === record.name"
                @click="emit('action', record.name, 'stop')"
              >{{ t("docker.stop") }}</a-button>
              <a-button
                v-else-if="record.status === 'paused'"
                size="mini"
                :disabled="actionLoading === record.name"
                @click="emit('action', record.name, 'unpause')"
              >{{ t("docker.unpause") }}</a-button>
              <a-button
                v-else
                size="mini"
                type="primary"
                status="success"
                :disabled="actionLoading === record.name"
                @click="emit('action', record.name, 'start')"
              >{{ t("docker.start") }}</a-button>
              <a-button
                size="mini"
                :disabled="actionLoading === record.name"
                @click="emit('action', record.name, 'restart')"
              >{{ t("docker.restart") }}</a-button>
              <a-button size="mini" @click="emit('logs', record.name)">
                <template #icon><icon-file /></template>
              </a-button>
              <a-button size="mini" @click="emit('terminal', record.name)">
                <template #icon><icon-code-square /></template>
              </a-button>
              <a-button
                size="mini"
                status="danger"
                :disabled="actionLoading === record.name"
                @click="emit('action', record.name, 'rm')"
              >{{ t("docker.delete") }}</a-button>
            </div>
          </template>
        </a-table>
        <div v-if="items.length" class="table-footer">
          <span>{{ items.length }} {{ t("docker.containers_count") }}</span>
        </div>
      </a-spin>
    </a-card>
  </div>
</template>

<style scoped>
.section-card {
  border-radius: 10px;
}
.cell-stack {
  display: flex;
  flex-direction: column;
}
.cell-name {
  font-weight: 500;
  color: var(--color-text-1);
}
.cell-mono {
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
  color: var(--color-text-2);
}
.cell-muted {
  color: var(--color-text-3);
}
.actions-row {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 6px;
  flex-wrap: wrap;
}
.table-footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  padding: 8px 4px 0;
  font-size: 12px;
  color: var(--color-text-3);
  border-top: 1px solid var(--color-border-2);
  margin-top: 8px;
}
.empty-icon {
  color: var(--color-text-3);
}
</style>

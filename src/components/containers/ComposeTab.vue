<script setup>
import { computed } from "vue";
import { t } from "../../lib/i18n.js";
import ContainerIcons from "../../icons/ContainerIcons.vue";

const props = defineProps({
  file: { type: String, default: "" },
  project: { type: String, default: "" },
  loading: { type: Boolean, default: false },
  error: { type: String, default: null },
  containers: { type: Array, default: () => [] },
  logs: { type: String, default: "" },
});

const emit = defineEmits([
  "file-input",
  "project-input",
  "up",
  "down",
  "ps",
  "logs",
  "clear-logs",
]);

function statusLabel(status) {
  const map = {
    running: t("docker.status_running"),
    exited: t("docker.status_exited"),
    paused: t("docker.status_paused"),
    created: t("docker.status_created"),
  };
  return map[status] || status;
}

const columns = computed(() => [
  { title: t("docker.name"), slotName: "name" },
  { title: t("docker.image"), slotName: "image" },
  { title: t("docker.status"), slotName: "status" },
  { title: t("docker.ports"), slotName: "ports" },
]);
</script>

<template>
  <div>
    <div class="grid grid-cols-2 gap-3 mb-4">
      <div>
        <label class="field-label">{{ t("docker.compose_file") }}</label>
        <a-input
          :model-value="file"
          placeholder="docker-compose.yml"
          @input="(v) => emit('file-input', v)"
        />
      </div>
      <div>
        <label class="field-label">{{ t("docker.compose_project") }}</label>
        <a-input
          :model-value="project"
          :placeholder="t('docker.compose_project_ph')"
          @input="(v) => emit('project-input', v)"
        />
      </div>
    </div>

    <div class="flex items-center gap-2 mb-4">
      <a-button type="primary" status="success" size="small" :disabled="loading" @click="emit('up')">
        <template #icon><icon-play-arrow /></template>
        {{ t("docker.compose_up") }}
      </a-button>
      <a-button status="danger" size="small" :disabled="loading" @click="emit('down')">
        <template #icon><icon-stop /></template>
        {{ t("docker.compose_down") }}
      </a-button>
      <a-button size="small" :disabled="loading" @click="emit('ps')">
        <template #icon><icon-menu /></template>
        {{ t("docker.compose_ps") }}
      </a-button>
      <a-button size="small" :disabled="loading" @click="emit('logs')">
        <template #icon><icon-file /></template>
        {{ t("docker.compose_logs") }}
      </a-button>
    </div>

    <a-alert v-if="error" type="error" class="mb-4">
      <pre class="error-pre">{{ error }}</pre>
    </a-alert>

    <a-card v-if="containers.length > 0" :bordered="true" class="section-card mb-4">
      <template #title>
        <span class="section-title">{{ t("docker.compose_services") }}</span>
      </template>
      <a-table
        :data="containers"
        :columns="columns"
        :pagination="false"
        :bordered="{ wrapper: false, cell: false }"
        size="small"
      >
        <template #name="{ record }">
          <span class="cell-name">{{ record.name }}</span>
        </template>
        <template #image="{ record }">
          <span class="cell-mono">{{ record.image }}</span>
        </template>
        <template #status="{ record }">
          <span class="status-inline" :class="record.status === 'running' ? 'ok' : ''">
            <ContainerIcons
              :name="record.status === 'running' ? 'container-running' : 'container-exited'"
              :size="12"
            />
            {{ statusLabel(record.status) }}
          </span>
        </template>
        <template #ports="{ record }">
          <span class="cell-mono cell-muted">{{ record.ports || "-" }}</span>
        </template>
      </a-table>
    </a-card>

    <a-card v-if="logs" :bordered="true" class="section-card">
      <template #title>
        <div class="card-title-row">
          <span class="section-title">{{ t("docker.logs") }}</span>
          <a-button size="mini" @click="emit('clear-logs')">
            <template #icon><icon-close /></template>
          </a-button>
        </div>
      </template>
      <pre class="logs-pre">{{ logs }}</pre>
    </a-card>
  </div>
</template>

<style scoped>
.grid-cols-2 {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}
.field-label {
  display: block;
  margin-bottom: 6px;
  font-size: 12px;
  color: var(--color-text-3);
}
.section-card {
  border-radius: 10px;
}
.section-title {
  font-size: 12px;
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--color-text-3);
}
.card-title-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
}
.error-pre {
  margin: 0;
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
  white-space: pre-wrap;
  color: rgb(var(--red-6));
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
.status-inline {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  color: var(--color-text-3);
}
.status-inline.ok {
  color: rgb(var(--green-6));
}
.logs-pre {
  margin: 0;
  max-height: 400px;
  overflow: auto;
  padding: 12px 0;
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
  color: var(--color-text-2);
  white-space: pre-wrap;
}
</style>

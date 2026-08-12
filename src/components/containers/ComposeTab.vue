<script setup>
import { t } from "../../lib/i18n.js";
import ContainerIcons from "../../icons/ContainerIcons.vue";
import AppIcon from "../AppIcon.vue";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import {
  Alert,
  AlertDescription,
  AlertTitle,
} from "@/components/ui/alert";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

/**
 * @typedef {Object} ComposeContainerInfo
 * @property {string} name
 * @property {string} image
 * @property {string} status
 * @property {Array<{publicPort?: number, privatePort?: number, type?: string}>} [ports]
 */

const props = defineProps({
  file: { type: String, default: "" },
  project: { type: String, default: "" },
  loading: { type: Boolean, default: false },
  error: { type: String, default: null },
  /** @type {import('vue').PropType<ComposeContainerInfo[]>} */
  containers: Array,
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
</script>

<template>
  <div>
    <div class="grid grid-cols-2 gap-3 mb-4">
      <div>
        <label class="field-label">{{ t("docker.compose_file") }}</label>
        <Input
          :model-value="file"
          placeholder="docker-compose.yml"
          @update:model-value="(v) => emit('file-input', v)"
        />
      </div>
      <div>
        <label class="field-label">{{ t("docker.compose_project") }}</label>
        <Input
          :model-value="project"
          :placeholder="t('docker.compose_project_ph')"
          @update:model-value="(v) => emit('project-input', v)"
        />
      </div>
    </div>

    <div class="flex items-center gap-2 mb-4">
      <Button
        :disabled="loading"
        @click="emit('up')"
      >
        <AppIcon name="play-arrow" class="size-4" />
        {{ t("docker.compose_up") }}
      </Button>
      <Button
        variant="destructive"
        :disabled="loading"
        @click="emit('down')"
      >
        <AppIcon name="stop" class="size-4" />
        {{ t("docker.compose_down") }}
      </Button>
      <Button variant="outline" :disabled="loading" @click="emit('ps')">
        <AppIcon name="menu" class="size-4" />
        {{ t("docker.compose_ps") }}
      </Button>
      <Button variant="outline" :disabled="loading" @click="emit('logs')">
        <AppIcon name="file" class="size-4" />
        {{ t("docker.compose_logs") }}
      </Button>
      <Spinner v-if="loading" class="size-4 text-muted-foreground" />
    </div>

    <Alert v-if="error" variant="destructive" class="mb-4">
      <AlertTitle>{{ t("error.title") }}</AlertTitle>
      <AlertDescription>
        <pre class="error-pre">{{ error }}</pre>
      </AlertDescription>
    </Alert>

    <Card v-if="containers.length > 0" class="section-card shadow-sm mb-4">
      <CardHeader class="py-3">
        <CardTitle class="section-title">
          {{ t("docker.compose_services") }}
        </CardTitle>
      </CardHeader>
      <CardContent class="p-0">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>{{ t("docker.name") }}</TableHead>
              <TableHead>{{ t("docker.image") }}</TableHead>
              <TableHead>{{ t("docker.status") }}</TableHead>
              <TableHead>{{ t("docker.ports") }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="record in containers" :key="record.name">
              <TableCell>
                <span class="cell-name">{{ record.name }}</span>
              </TableCell>
              <TableCell>
                <span class="cell-mono">{{ record.image }}</span>
              </TableCell>
              <TableCell>
                <span
                  class="status-inline"
                  :class="record.status === 'running' ? 'ok' : ''"
                >
                  <ContainerIcons
                    :name="record.status === 'running' ? 'container-running' : 'container-exited'"
                    :size="12"
                  />
                  {{ statusLabel(record.status) }}
                </span>
              </TableCell>
              <TableCell>
                <span class="cell-mono cell-muted">{{ record.ports || "-" }}</span>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </CardContent>
    </Card>

    <Card v-if="logs" class="section-card shadow-sm">
      <CardHeader class="py-3">
        <div class="card-title-row">
          <CardTitle class="section-title">{{ t("docker.logs") }}</CardTitle>
          <Button size="icon" variant="ghost" @click="emit('clear-logs')">
            <AppIcon name="close" class="size-4" />
          </Button>
        </div>
      </CardHeader>
      <CardContent>
        <pre class="logs-pre">{{ logs }}</pre>
      </CardContent>
    </Card>
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
  color: var(--color-muted-foreground);
}
.section-card {
  border-radius: 10px;
}
.section-title {
  font-size: 12px;
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--color-muted-foreground);
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
  color: var(--color-destructive);
}
.cell-name {
  font-weight: 500;
  color: var(--color-foreground);
}
.cell-mono {
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
  color: var(--color-muted-foreground);
}
.cell-muted {
  color: var(--color-muted-foreground);
}
.status-inline {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  color: var(--color-muted-foreground);
}
.status-inline.ok {
  color: var(--color-success);
}
.logs-pre {
  margin: 0;
  max-height: 400px;
  overflow: auto;
  padding: 12px 0;
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
  color: var(--color-muted-foreground);
  white-space: pre-wrap;
}
</style>
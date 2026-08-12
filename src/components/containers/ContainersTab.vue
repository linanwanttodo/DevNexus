<script setup>
import { t } from "../../lib/i18n.js";
import ContainerIcons from "../../icons/ContainerIcons.vue";
import AppIcon from "../AppIcon.vue";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
import {
  Alert,
  AlertDescription,
  AlertTitle,
} from "@/components/ui/alert";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyMedia,
} from "@/components/ui/empty";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

/**
 * @typedef {Object} ContainerInfo
 * @property {string} id
 * @property {string} name
 * @property {string} image
 * @property {string} status
 * @property {string} [created]
 * @property {Array<{publicPort?: number, privatePort?: number, type?: string}>} [ports]
 */

const props = defineProps({
  /** @type {import('vue').PropType<ContainerInfo[]>} */
  items: Array,
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
</script>

<template>
  <div>
    <div class="flex items-center justify-between mb-4">
      <label class="flex cursor-pointer items-center gap-2 text-sm">
        <Checkbox
          :model-value="showAll"
          @update:model-value="(c) => emit('show-all-change', !!c)"
        />
        {{ t("docker.show_all") }}
      </label>
      <Button
        size="sm"
        variant="outline"
        :disabled="loading"
        @click="emit('refresh')"
      >
        <AppIcon name="refresh" :spin="loading" class="size-4" />
        {{ t("common.refresh") }}
      </Button>
    </div>

    <Card class="section-card shadow-sm">
      <CardContent class="p-0">
        <!-- Loading -->
        <div v-if="loading && items.length === 0" class="space-y-3 p-4">
          <Skeleton class="h-8 w-full" />
          <Skeleton class="h-8 w-full" />
          <Skeleton class="h-8 w-full" />
        </div>

        <!-- Error -->
        <Alert v-else-if="error" variant="destructive" class="m-4">
          <AlertTitle>{{ t("error.title") }}</AlertTitle>
          <AlertDescription>{{ error }}</AlertDescription>
          <Button
            variant="outline"
            size="sm"
            class="mt-3"
            @click="emit('refresh')"
          >
            {{ t("common.retry") }}
          </Button>
        </Alert>

        <!-- Empty -->
        <Empty v-else-if="items.length === 0" class="py-5">
          <EmptyMedia>
            <ContainerIcons name="container" :size="36" class="empty-icon" />
          </EmptyMedia>
          <EmptyContent>
            <EmptyDescription>
              {{ search ? t("docker.no_matching") : t("docker.no_containers") }}
            </EmptyDescription>
          </EmptyContent>
        </Empty>

        <!-- Table -->
        <template v-else>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead class="w-10" />
                <TableHead>{{ t("docker.name") }}</TableHead>
                <TableHead>{{ t("docker.image") }}</TableHead>
                <TableHead>{{ t("docker.ports") }}</TableHead>
                <TableHead class="w-[130px]">{{ t("docker.created") }}</TableHead>
                <TableHead class="w-[320px] text-right">
                  {{ t("docker.actions") }}
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-for="record in items" :key="record.id">
                <TableCell>
                  <ContainerIcons :name="statusIcon(record.status)" :size="16" />
                </TableCell>
                <TableCell>
                  <div class="cell-stack">
                    <span class="cell-name">{{ record.name }}</span>
                    <span class="cell-mono">{{ shortId(record.id) }}</span>
                  </div>
                </TableCell>
                <TableCell>
                  <span class="cell-mono">{{ record.image }}</span>
                </TableCell>
                <TableCell>
                  <span class="cell-mono cell-muted">{{ record.ports || "-" }}</span>
                </TableCell>
                <TableCell>
                  <span class="cell-muted">{{ record.created || "-" }}</span>
                </TableCell>
                <TableCell>
                  <div class="actions-row">
                    <Button
                      v-if="record.status === 'running'"
                      size="sm"
                      variant="outline"
                      :disabled="actionLoading === record.name"
                      @click="emit('action', record.name, 'pause')"
                    >
                      {{ t("docker.pause") }}
                    </Button>
                    <Button
                      v-if="record.status === 'running'"
                      size="sm"
                      variant="destructive"
                      :disabled="actionLoading === record.name"
                      @click="emit('action', record.name, 'stop')"
                    >
                      {{ t("docker.stop") }}
                    </Button>
                    <Button
                      v-else-if="record.status === 'paused'"
                      size="sm"
                      variant="outline"
                      :disabled="actionLoading === record.name"
                      @click="emit('action', record.name, 'unpause')"
                    >
                      {{ t("docker.unpause") }}
                    </Button>
                    <Button
                      v-else
                      size="sm"
                      :disabled="actionLoading === record.name"
                      @click="emit('action', record.name, 'start')"
                    >
                      <Spinner
                        v-if="actionLoading === record.name"
                        class="size-3.5"
                      />
                      {{ t("docker.start") }}
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
                      :disabled="actionLoading === record.name"
                      @click="emit('action', record.name, 'restart')"
                    >
                      {{ t("docker.restart") }}
                    </Button>
                    <Button
                      size="icon"
                      variant="ghost"
                      @click="emit('logs', record.name)"
                    >
                      <AppIcon name="file" class="size-4" />
                    </Button>
                    <Button
                      size="icon"
                      variant="ghost"
                      @click="emit('terminal', record.name)"
                    >
                      <AppIcon name="code-square" class="size-4" />
                    </Button>
                    <Button
                      size="sm"
                      variant="destructive"
                      :disabled="actionLoading === record.name"
                      @click="emit('action', record.name, 'rm')"
                    >
                      {{ t("docker.delete") }}
                    </Button>
                  </div>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
          <div class="table-footer">
            <span>{{ items.length }} {{ t("docker.containers_count") }}</span>
          </div>
        </template>
      </CardContent>
    </Card>
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
  padding: 8px 16px 0;
  font-size: 12px;
  color: var(--color-muted-foreground);
  border-top: 1px solid var(--color-border);
  margin-top: 8px;
}
.empty-icon {
  color: var(--color-muted-foreground);
}
</style>
<script setup>
import { t } from "../../lib/i18n.js";
import ContainerIcons from "../../icons/ContainerIcons.vue";
import AppIcon from "../AppIcon.vue";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
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
 * @typedef {Object} VolumeInfo
 * @property {string} name
 * @property {string} [driver]
 * @property {string} [mountpoint]
 * @property {string} [created]
 */

const props = defineProps({
  /** @type {import('vue').PropType<VolumeInfo[]>} */
  items: Array,
  loading: { type: Boolean, default: false },
  error: { type: String, default: null },
  actionLoading: { type: String, default: "" },
});

const emit = defineEmits(["create", "refresh", "remove"]);
</script>

<template>
  <div>
    <div class="flex items-center justify-between mb-4">
      <Button size="sm" @click="emit('create')">
        <AppIcon name="plus" class="size-4" />
        {{ t("docker.create") }}
      </Button>
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
            <ContainerIcons name="volume" :size="36" class="empty-icon" />
          </EmptyMedia>
          <EmptyContent>
            <EmptyDescription>{{ t("docker.no_volumes") }}</EmptyDescription>
          </EmptyContent>
        </Empty>

        <!-- Table -->
        <Table v-else>
          <TableHeader>
            <TableRow>
              <TableHead>{{ t("docker.name") }}</TableHead>
              <TableHead class="w-[140px]">{{ t("docker.driver") }}</TableHead>
              <TableHead>{{ t("docker.mountpoint") }}</TableHead>
              <TableHead class="w-[130px]">{{ t("docker.created") }}</TableHead>
              <TableHead class="w-[90px] text-right">
                {{ t("docker.actions") }}
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="record in items" :key="record.name">
              <TableCell>
                <span class="cell-name">{{ record.name }}</span>
              </TableCell>
              <TableCell>
                <span class="cell-muted">{{ record.driver }}</span>
              </TableCell>
              <TableCell>
                <span
                  class="cell-mono cell-muted"
                  :title="record.mountpoint"
                >{{ record.mountpoint }}</span>
              </TableCell>
              <TableCell>
                <span class="cell-muted">{{ record.created || "-" }}</span>
              </TableCell>
              <TableCell class="text-right">
                <Button
                  size="sm"
                  variant="destructive"
                  :disabled="actionLoading === record.name"
                  @click="emit('remove', record.name)"
                >
                  {{ t("docker.delete") }}
                </Button>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  </div>
</template>

<style scoped>
.section-card {
  border-radius: 10px;
}
.cell-name {
  font-weight: 500;
  color: var(--color-foreground);
}
.cell-mono {
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
  max-width: 280px;
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.cell-muted {
  color: var(--color-muted-foreground);
}
.empty-icon {
  color: var(--color-muted-foreground);
}
</style>
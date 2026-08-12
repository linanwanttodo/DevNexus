<script setup>
import { t } from "../../lib/i18n.js";
import ContainerIcons from "../../icons/ContainerIcons.vue";
import AppIcon from "../AppIcon.vue";
import { Badge } from "@/components/ui/badge";
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
 * @typedef {Object} ImageInfo
 * @property {string} id
 * @property {string} repository
 * @property {string} tag
 * @property {string} [created]
 * @property {number} [size]
 */

const props = defineProps({
  /** @type {import('vue').PropType<ImageInfo[]>} */
  items: Array,
  loading: { type: Boolean, default: false },
  error: { type: String, default: null },
  search: { type: String, default: "" },
  actionLoading: { type: String, default: "" },
});

const emit = defineEmits(["pull", "build", "refresh", "push", "tag", "remove"]);

function shortId(id) {
  return id ? id.substring(0, 12) : "";
}
</script>

<template>
  <div>
    <div class="flex items-center justify-between mb-4">
      <div class="flex items-center gap-2">
        <Button size="sm" @click="emit('pull')">
          <AppIcon name="download" class="size-4" />
          {{ t("docker.pull") }}
        </Button>
        <Button size="sm" variant="outline" @click="emit('build')">
          <AppIcon name="build" class="size-4" />
          {{ t("docker.build") }}
        </Button>
      </div>
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
            <ContainerIcons name="image" :size="36" class="empty-icon" />
          </EmptyMedia>
          <EmptyContent>
            <EmptyDescription>
              {{ search ? t("docker.no_matching") : t("docker.no_images") }}
            </EmptyDescription>
          </EmptyContent>
        </Empty>

        <!-- Table -->
        <template v-else>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>{{ t("docker.repository") }}</TableHead>
                <TableHead class="w-[110px]">{{ t("docker.tag") }}</TableHead>
                <TableHead class="w-[120px]">{{ t("docker.image_id") }}</TableHead>
                <TableHead class="w-[130px]">{{ t("docker.created") }}</TableHead>
                <TableHead class="w-[100px] text-right">{{ t("docker.size") }}</TableHead>
                <TableHead class="w-[190px] text-right">
                  {{ t("docker.actions") }}
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-for="record in items" :key="record.id">
                <TableCell>
                  <span class="cell-name">{{ record.repository }}</span>
                </TableCell>
                <TableCell>
                  <Badge variant="secondary" class="mono-tag font-normal">
                    {{ record.tag }}
                  </Badge>
                </TableCell>
                <TableCell>
                  <span class="cell-mono cell-muted">{{ shortId(record.id) }}</span>
                </TableCell>
                <TableCell>
                  <span class="cell-muted">{{ record.created || "-" }}</span>
                </TableCell>
                <TableCell class="text-right">
                  <span class="cell-muted">{{ record.size || "-" }}</span>
                </TableCell>
                <TableCell>
                  <div class="actions-row">
                    <Button size="sm" variant="outline" @click="emit('push', record)">
                      {{ t("docker.push") }}
                    </Button>
                    <Button size="sm" variant="outline" @click="emit('tag', record)">
                      {{ t("docker.tag") }}
                    </Button>
                    <Button
                      size="sm"
                      variant="destructive"
                      :disabled="actionLoading === record.id"
                      @click="emit('remove', record.id, `${record.repository}:${record.tag}`)"
                    >
                      {{ t("docker.delete") }}
                    </Button>
                  </div>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
          <div class="table-footer">
            <span>{{ items.length }} {{ t("docker.images_count") }}</span>
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
.cell-name {
  font-weight: 500;
  color: var(--color-foreground);
}
.cell-mono {
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
}
.cell-muted {
  color: var(--color-muted-foreground);
}
.mono-tag {
  font-family: "JetBrains Mono", monospace;
}
.actions-row {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 6px;
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
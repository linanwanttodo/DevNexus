<script setup>
import { computed } from "vue";
import { t } from "../../lib/i18n.js";
import ContainerIcons from "../../icons/ContainerIcons.vue";

const props = defineProps({
  items: { type: Array, default: () => [] },
  loading: { type: Boolean, default: false },
  error: { type: String, default: null },
  search: { type: String, default: "" },
  actionLoading: { type: String, default: "" },
});

const emit = defineEmits(["pull", "build", "refresh", "push", "tag", "remove"]);

function shortId(id) {
  return id ? id.substring(0, 12) : "";
}

const columns = computed(() => [
  { title: t("docker.repository"), slotName: "repo" },
  { title: t("docker.tag"), slotName: "tag", width: 110 },
  { title: t("docker.image_id"), slotName: "id", width: 120 },
  { title: t("docker.created"), slotName: "created", width: 130 },
  { title: t("docker.size"), slotName: "size", align: "right", width: 100 },
  { title: t("docker.actions"), slotName: "actions", align: "right", width: 190 },
]);
</script>

<template>
  <div>
    <div class="flex items-center justify-between mb-4">
      <div class="flex items-center gap-2">
        <a-button size="small" @click="emit('pull')">
          <template #icon><icon-download /></template>
          {{ t("docker.pull") }}
        </a-button>
        <a-button size="small" @click="emit('build')">
          <template #icon><icon-build /></template>
          {{ t("docker.build") }}
        </a-button>
      </div>
      <a-button size="small" @click="emit('refresh')" :disabled="loading">
        <template #icon>
          <icon-refresh :spin="loading" />
        </template>
        {{ t("common.refresh") }}
      </a-button>
    </div>

    <a-card :bordered="true" class="section-card">
      <a-spin :loading="loading && items.length === 0" class="w-full">
        <a-alert v-if="error" type="error" :message="error">
          <template #action>
            <a-button size="mini" @click="emit('refresh')">{{ t("common.retry") }}</a-button>
          </template>
        </a-alert>

        <a-empty
          v-else-if="items.length === 0"
          :description="search ? t('docker.no_matching') : t('docker.no_images')"
        >
          <template #image>
            <ContainerIcons name="image" :size="36" class="empty-icon" />
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
          <template #repo="{ record }">
            <span class="cell-name">{{ record.repository }}</span>
          </template>
          <template #tag="{ record }">
            <a-tag size="small" class="mono-tag">{{ record.tag }}</a-tag>
          </template>
          <template #id="{ record }">
            <span class="cell-mono cell-muted">{{ shortId(record.id) }}</span>
          </template>
          <template #created="{ record }">
            <span class="cell-muted">{{ record.created || "-" }}</span>
          </template>
          <template #size="{ record }">
            <span class="cell-muted">{{ record.size || "-" }}</span>
          </template>
          <template #actions="{ record }">
            <div class="actions-row">
              <a-button size="mini" @click="emit('push', record)">{{ t("docker.push") }}</a-button>
              <a-button size="mini" @click="emit('tag', record)">{{ t("docker.tag") }}</a-button>
              <a-button
                size="mini"
                status="danger"
                :disabled="actionLoading === record.id"
                @click="emit('remove', record.id, `${record.repository}:${record.tag}`)"
              >{{ t("docker.delete") }}</a-button>
            </div>
          </template>
        </a-table>
        <div v-if="items.length" class="table-footer">
          <span>{{ items.length }} {{ t("docker.images_count") }}</span>
        </div>
      </a-spin>
    </a-card>
  </div>
</template>

<style scoped>
.section-card {
  border-radius: 10px;
}
.cell-name {
  font-weight: 500;
  color: var(--color-text-1);
}
.cell-mono {
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
}
.cell-muted {
  color: var(--color-text-3);
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

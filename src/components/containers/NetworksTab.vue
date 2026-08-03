<script setup>
import { computed } from "vue";
import { t } from "../../lib/i18n.js";
import ContainerIcons from "../../icons/ContainerIcons.vue";

const props = defineProps({
  items: { type: Array, default: () => [] },
  loading: { type: Boolean, default: false },
  error: { type: String, default: null },
  actionLoading: { type: String, default: "" },
});

const emit = defineEmits(["create", "refresh", "remove"]);

const columns = computed(() => [
  { title: t("docker.name"), slotName: "name" },
  { title: t("docker.driver"), slotName: "driver", width: 160 },
  { title: t("docker.scope"), slotName: "scope", width: 140 },
  { title: t("docker.actions"), slotName: "actions", align: "right", width: 90 },
]);
</script>

<template>
  <div>
    <div class="flex items-center justify-between mb-4">
      <a-button size="small" @click="emit('create')">
        <template #icon><icon-plus /></template>
        {{ t("docker.create") }}
      </a-button>
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

        <a-empty v-else-if="items.length === 0" :description="t('docker.no_networks')">
          <template #image>
            <ContainerIcons name="network" :size="36" class="empty-icon" />
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
          :row-key="(r) => r.name"
        >
          <template #name="{ record }">
            <span class="cell-name">{{ record.name }}</span>
          </template>
          <template #driver="{ record }">
            <span class="cell-muted">{{ record.driver }}</span>
          </template>
          <template #scope="{ record }">
            <span class="cell-muted">{{ record.scope }}</span>
          </template>
          <template #actions="{ record }">
            <a-button
              size="mini"
              status="danger"
              :disabled="actionLoading === record.name"
              @click="emit('remove', record.name)"
            >{{ t("docker.delete") }}</a-button>
          </template>
        </a-table>
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
.cell-muted {
  color: var(--color-text-3);
}
.empty-icon {
  color: var(--color-text-3);
}
</style>

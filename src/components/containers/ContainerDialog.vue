<script setup>
import { t } from "../../lib/i18n.js";

/**
 * config: {
 *   title, icon, width,
 *   fields: [{ id, type, placeholder, value, onInput, enterSubmit }],
 *   loading, submitLabel, loadingLabel, canSubmit,
 *   onSubmit, onClose,
 * }
 */
const props = defineProps({
  config: { type: Object, required: true },
});
</script>

<template>
  <a-modal
    :visible="true"
    :title="config.title"
    :width="config.width || 400"
    :footer="false"
    :closable="false"
    :mask-closable="false"
    @cancel="config.onClose"
  >
    <template #title>
      <span class="dialog-title">
        <icon-edit v-if="config.icon === 'edit'" />
        <icon-download v-else-if="config.icon === 'download'" />
        <icon-build v-else-if="config.icon === 'construction'" />
        <icon-upload v-else-if="config.icon === 'upload'" />
        <icon-tags v-else-if="config.icon === 'sell'" />
        <icon-plus v-else-if="config.icon === 'add'" />
        <span>{{ config.title }}</span>
      </span>
    </template>

    <div class="dialog-body" :class="{ 'multi-field': config.fields.length > 1 }">
      <a-input
        v-for="f in config.fields"
        :key="f.id"
        :id="f.id"
        :type="f.type || 'text'"
        :model-value="f.value"
        :placeholder="f.placeholder"
        @input="(v) => f.onInput(v)"
        @press-enter="f.enterSubmit !== false && config.onSubmit()"
      />
    </div>

    <template #footer>
      <div class="dialog-footer">
        <a-button @click="config.onClose">{{ t("common.cancel") }}</a-button>
        <a-button
          type="primary"
          :loading="config.loading"
          :disabled="!config.canSubmit"
          @click="config.onSubmit"
        >
          {{ config.loading ? config.loadingLabel : config.submitLabel }}
        </a-button>
      </div>
    </template>
  </a-modal>
</template>

<style scoped>
.dialog-title {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
.dialog-body {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding-top: 8px;
  border-top: 1px solid var(--color-border-2);
  margin-top: 8px;
}
</style>

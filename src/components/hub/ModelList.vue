<script setup>
import { t } from "../../lib/i18n.js";

const props = defineProps({
  models: { type: Array, default: () => [] },
  selected: { type: Object, default: () => ({}) },
  aliases: { type: Object, default: () => ({}) },
  contexts: { type: Object, default: () => ({}) },
  onToggle: { type: Function, default: null },
  showCtx: { type: Boolean, default: true },
  maxH: { type: String, default: "max-h-60" },
  extraClass: { type: String, default: "" },
});

const emit = defineEmits(["toggle"]);

function toggle(id) {
  if (props.onToggle) props.onToggle(id);
  else emit("toggle", id);
}
</script>

<template>
  <div class="model-list" :class="[maxH, extraClass]">
    <div
      v-for="m in models"
      :key="m.id"
      class="model-row"
      role="option"
      :aria-selected="!!selected[m.id]"
      tabindex="0"
      @click="toggle(m.id)"
      @keydown.enter="toggle(m.id)"
    >
      <!-- Checkbox -->
      <div class="check-col">
        <div v-if="selected[m.id]" class="check-on">
          <icon-check class="check-icon" />
        </div>
        <div v-else class="check-off"></div>
      </div>

      <!-- Model info -->
      <div v-if="showCtx" class="model-info">
        <div class="model-id">{{ m.id }}</div>
        <div v-if="m.id !== m.name" class="model-name">{{ m.name }}</div>
      </div>
      <div v-else class="model-info">
        <div class="model-id">{{ m.id }}</div>
      </div>

      <!-- Alias + context inputs -->
      <template v-if="selected[m.id]">
        <a-input
          v-model="aliases[m.id]"
          size="mini"
          :placeholder="t('apiHub.alias')"
          class="alias-input"
          :class="showCtx ? 'w-24' : 'w-32'"
          @click.stop
          @keydown.stop
        />
        <template v-if="showCtx">
          <a-input-number
            v-model="contexts[m.id]"
            size="mini"
            class="ctx-input"
            :placeholder="'200000'"
            @click.stop
            @keydown.stop
          />
          <span class="ctx-label">ctx</span>
        </template>
      </template>

      <div v-if="showCtx && m.owned_by" class="owned-by">{{ m.owned_by }}</div>
    </div>
  </div>
</template>

<style scoped>
.model-list {
  overflow-y: auto;
  border: 1px solid var(--color-border-2);
  border-radius: 8px;
  background-color: var(--color-fill-1);
}
.model-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border-2);
  cursor: pointer;
  transition: background-color 0.15s;
}
.model-row:last-child {
  border-bottom: none;
}
.model-row:hover {
  background-color: var(--color-fill-2);
}
.check-col {
  width: 20px;
  display: flex;
  justify-content: center;
  flex-shrink: 0;
}
.check-on {
  width: 16px;
  height: 16px;
  border-radius: 4px;
  background-color: rgb(var(--primary-6));
  display: flex;
  align-items: center;
  justify-content: center;
}
.check-icon {
  color: #fff;
  font-size: 11px;
}
.check-off {
  width: 16px;
  height: 16px;
  border-radius: 4px;
  border: 1px solid var(--color-border-3);
}
.model-info {
  flex: 1;
  min-width: 0;
}
.model-id {
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
  color: var(--color-text-1);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.model-name {
  font-size: 10px;
  color: var(--color-text-3);
}
.alias-input {
  text-align: right;
}
.ctx-input {
  width: 80px;
  text-align: right;
}
.ctx-label {
  font-size: 10px;
  color: var(--color-text-3);
  flex-shrink: 0;
}
.owned-by {
  font-size: 10px;
  color: var(--color-text-3);
  flex-shrink: 0;
}
</style>

<script setup>
import { t } from "../../lib/i18n.js";
import AppIcon from "../AppIcon.vue";
import { Input } from "@/components/ui/input";

/**
 * @typedef {Object} ModelInfo
 * @property {string} id
 * @property {string} [name]
 * @property {string} [owned_by]
 */

const props = defineProps({
  /** @type {import('vue').PropType<ModelInfo[]>} */
  models: Array,
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
          <AppIcon name="check" class="check-icon size-3" />
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
        <Input
          v-model="aliases[m.id]"
          :placeholder="t('apiHub.alias')"
          class="alias-input h-6 text-xs"
          :class="showCtx ? 'w-24' : 'w-32'"
          @click.stop
          @keydown.stop
        />
        <template v-if="showCtx">
          <Input
            v-model="contexts[m.id]"
            type="number"
            class="ctx-input h-6 text-xs"
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
  border: 1px solid var(--color-border);
  border-radius: 8px;
  background-color: var(--color-muted);
}
.model-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border);
  cursor: pointer;
  transition: background-color 0.15s;
}
.model-row:last-child {
  border-bottom: none;
}
.model-row:hover {
  background-color: var(--color-accent);
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
  background-color: var(--color-primary);
  display: flex;
  align-items: center;
  justify-content: center;
}
.check-icon {
  color: var(--color-primary-foreground);
}
.check-off {
  width: 16px;
  height: 16px;
  border-radius: 4px;
  border: 1px solid var(--color-border);
}
.model-info {
  flex: 1;
  min-width: 0;
}
.model-id {
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
  color: var(--color-foreground);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.model-name {
  font-size: 10px;
  color: var(--color-muted-foreground);
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
  color: var(--color-muted-foreground);
  flex-shrink: 0;
}
.owned-by {
  font-size: 10px;
  color: var(--color-muted-foreground);
  flex-shrink: 0;
}
</style>
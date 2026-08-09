<script setup>
import { computed } from "vue";
import { t } from "../../lib/i18n.js";
import AppIcon from "../AppIcon.vue";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";

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

const dialogIcon = computed(() => {
  const map = {
    edit: "edit",
    download: "download",
    construction: "build",
    upload: "upload",
    sell: "tags",
    add: "plus",
  };
  return map[props.config.icon] || null;
});

// Tailwind 需静态类名，按 config.width 映射到预设宽度
const contentClass = computed(() => {
  const w = props.config.width || 400;
  if (w >= 700) return "sm:max-w-3xl";
  if (w >= 600) return "sm:max-w-2xl";
  if (w >= 500) return "sm:max-w-xl";
  if (w >= 400) return "sm:max-w-lg";
  return "sm:max-w-md";
});
</script>

<template>
  <Dialog
    :open="config.open !== undefined ? config.open : true"
    @update:open="(o) => !o && config.onClose()"
  >
    <DialogContent :class="contentClass">
      <DialogHeader>
        <DialogTitle class="dialog-title">
          <AppIcon v-if="dialogIcon" :name="dialogIcon" class="size-4" />
          {{ config.title }}
        </DialogTitle>
        <DialogDescription class="sr-only">{{ config.title }}</DialogDescription>
      </DialogHeader>

      <div
        class="dialog-body"
        :class="{ 'multi-field': config.fields.length > 1 }"
      >
        <Input
          v-for="f in config.fields"
          :key="f.id"
          :id="f.id"
          :type="f.type || 'text'"
          :model-value="f.value"
          :placeholder="f.placeholder"
          @update:model-value="(v) => f.onInput(v)"
          @keydown.enter="f.enterSubmit !== false && config.onSubmit()"
        />
      </div>

      <DialogFooter>
        <Button variant="outline" @click="config.onClose">
          {{ t("common.cancel") }}
        </Button>
        <Button
          :disabled="config.loading || !config.canSubmit"
          @click="config.onSubmit"
        >
          <Spinner v-if="config.loading" class="size-4" />
          {{ config.loading ? config.loadingLabel : config.submitLabel }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
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
</style>
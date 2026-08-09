<script setup>
// AppIcon — 统一图标组件：接收 Arco 图标名，渲染 @lucide/vue 图标。
// 迁移期所有 <icon-xxx> 替换为 <Icon name="xxx" />。
import { computed, useAttrs } from "vue";
import * as Icons from "@lucide/vue";
import { iconMap, FALLBACK_ICON } from "../lib/icon-map.js";

const props = defineProps({
  name: { type: String, required: true },
  size: { type: [Number, String], default: undefined },
  spin: { type: Boolean, default: false },
});

const attrs = useAttrs();
const Comp = computed(() => Icons[iconMap[props.name]] || Icons[FALLBACK_ICON]);

const mergedClass = computed(() => [attrs.class, props.spin && "animate-spin"]);
</script>

<template>
  <component :is="Comp" :size="size" :class="mergedClass" />
</template>
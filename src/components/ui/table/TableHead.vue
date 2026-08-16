<script setup>
import { cn } from "@/lib/utils";

const props = defineProps({
  class: { type: null, required: false },
});
</script>

<template>
  <th
    :class="
      cn(
        'h-9 whitespace-nowrap border-b border-border/70 bg-muted/40 px-3 text-left align-middle text-[11px] font-semibold uppercase tracking-wider text-muted-foreground/90 [&:has([role=checkbox])]:pr-0',
        props.class,
      )
    "
  >
    <slot />
  </th>
</template>

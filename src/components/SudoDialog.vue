<script setup>
import { ref, watch } from "vue";
import { t } from "../lib/i18n.js";
import { sudoState, sudoResolve } from "../lib/sudo.js";
import AppIcon from "./AppIcon.vue";
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

const input = ref("");

watch(
  () => sudoState.value,
  (v) => {
    input.value = "";
  }
);

function onOk() {
  const pw = input.value;
  sudoResolve(pw || null);
}
function onCancel() {
  sudoResolve(null);
}
</script>

<template>
  <Dialog :open="!!sudoState" @update:open="(o) => !o && onCancel()">
    <DialogContent class="sm:max-w-md">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <AppIcon name="shield" class="size-5 text-warning" />
          {{ t("sudo.title") }}
        </DialogTitle>
        <DialogDescription>
          {{ sudoState?.message || t("sudo.desc") }}
        </DialogDescription>
      </DialogHeader>
      <div class="space-y-2">
        <Input
          v-model="input"
          type="password"
          :placeholder="t('sudo.placeholder')"
          @keydown.enter="onOk"
          autofocus
        />
        <p class="text-[11px] text-muted-foreground">{{ t("sudo.hint") }}</p>
      </div>
      <DialogFooter>
        <Button variant="outline" @click="onCancel">{{ t("common.cancel") }}</Button>
        <Button @click="onOk" :disabled="!input.trim()">{{ t("common.confirm") }}</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
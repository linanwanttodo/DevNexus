<script setup>
// ConfirmDialog — 全局确认对话框，消费 lib/confirm.js 的 confirmState
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { confirmResolve, confirmState } from "../lib/confirm.js";
</script>

<template>
  <Dialog :open="!!confirmState" @update:open="(o) => !o && confirmResolve(false)">
    <DialogContent class="sm:max-w-md">
      <DialogHeader>
        <DialogTitle>{{ confirmState?.title }}</DialogTitle>
        <DialogDescription>{{ confirmState?.message }}</DialogDescription>
      </DialogHeader>
      <DialogFooter>
        <Button variant="outline" @click="confirmResolve(false)">
          {{ confirmState?.cancelText }}
        </Button>
        <Button
          :variant="confirmState?.danger ? 'destructive' : 'default'"
          @click="confirmResolve(true)"
        >
          {{ confirmState?.okText }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
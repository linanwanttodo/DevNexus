<script setup>
import { ref, watch } from "vue";
import { t } from "../lib/i18n.js";
import AppIcon from "./AppIcon.vue";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";

// mode "form": 渲染字段分组表单（add/edit）
// mode "view": 渲染密码查看详情
// groups: 字段分组（每组为一行；单字段组整行，多字段组 2 列网格）
// 每组字段: { id, labelKey, required, type, placeholder, textarea, value, onInput }
const props = defineProps({
  title: { type: String, default: "" },
  mode: { type: String, default: "form" }, // form | view
  groups: { type: Array, default: () => [] },
  submitLabel: { type: String, default: "" },
  password: { type: String, default: "" },
});

const emit = defineEmits(["submit", "close", "copy"]);

const visible = ref(true);

function onOk() {
  emit("submit");
  visible.value = false;
}
function onCancel() {
  visible.value = false;
}

// 供表单输入绑定的本地状态：值与 props.groups 中的 value 同步
function inputValue(f) {
  return f.value;
}
function setInputValue(f, v) {
  f.onInput(v);
}

watch(visible, (v) => {
  if (!v) emit("close");
});
</script>

<template>
  <Dialog :open="visible" @update:open="(v) => (visible = v)">
    <DialogContent class="max-h-[85vh] overflow-y-auto sm:max-w-md">
      <DialogHeader>
        <DialogTitle>{{ title }}</DialogTitle>
      </DialogHeader>

      <!-- 查看模式 -->
      <div v-if="mode === 'view'" class="view-box">
        <div class="view-label">{{ t("passwords.password") }}</div>
        <div class="view-password-row">
          <code class="view-password">{{ password }}</code>
          <Button variant="ghost" size="icon-sm" @click="emit('copy')">
            <AppIcon name="copy" class="size-4" />
          </Button>
        </div>
      </div>

      <!-- 表单模式 -->
      <div v-else class="space-y-3">
        <div
          v-for="(group, gi) in groups"
          :key="gi"
          class="grid grid-cols-1 gap-4"
          :class="group.length > 1 ? 'sm:grid-cols-2' : ''"
        >
          <div v-for="f in group" :key="f.id">
            <Label :for="f.id" class="mb-1.5 block">
              {{ `${t(f.labelKey)}${f.required ? " *" : ""}` }}
            </Label>
            <Textarea
              v-if="f.textarea"
              :id="f.id"
              :model-value="inputValue(f)"
              :placeholder="f.placeholder"
              :rows="2"
              @update:model-value="(v) => setInputValue(f, v)"
            />
            <Input
              v-else-if="f.type === 'password'"
              :id="f.id"
              type="password"
              :model-value="inputValue(f)"
              :placeholder="f.placeholder"
              @update:model-value="(v) => setInputValue(f, v)"
            />
            <Input
              v-else
              :id="f.id"
              :type="f.type || 'text'"
              :model-value="inputValue(f)"
              :placeholder="f.placeholder"
              @update:model-value="(v) => setInputValue(f, v)"
            />
          </div>
        </div>
      </div>

      <DialogFooter>
        <Button v-if="mode === 'form'" variant="outline" @click="onCancel">
          {{ t("passwords.cancel") }}
        </Button>
        <Button @click="onOk">
          {{ mode === "form" ? submitLabel : t("passwords.close") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<style scoped>
.view-box {
  border: 1px solid var(--color-border);
  border-radius: 8px;
  padding: 14px;
  background-color: var(--color-muted);
}
.view-label {
  font-size: 12px;
  color: var(--color-muted-foreground);
  margin-bottom: 8px;
}
.view-password-row {
  display: flex;
  align-items: center;
  gap: 8px;
}
.view-password {
  flex: 1;
  word-break: break-all;
  font-size: 13px;
  color: var(--color-foreground);
}
</style>
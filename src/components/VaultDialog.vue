<script setup>
import { ref, watch } from "vue";
import { t } from "../lib/i18n.js";

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
}
function onCancel() {
  emit("close");
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
  <a-modal
    v-model:visible="visible"
    :title="title"
    :ok-text="mode === 'form' ? submitLabel : t('passwords.close')"
    :cancel-text="t('passwords.cancel')"
    :hide-cancel="mode === 'view'"
    :on-before-ok="onOk"
    @cancel="onCancel"
  >
    <!-- 查看模式 -->
    <div v-if="mode === 'view'" class="view-box">
      <div class="view-label">{{ t("passwords.password") }}</div>
      <div class="view-password-row">
        <code class="view-password">{{ password }}</code>
        <a-button type="text" size="small" @click="emit('copy')">
          <template #icon><icon-copy /></template>
        </a-button>
      </div>
    </div>

    <!-- 表单模式 -->
    <a-form v-else layout="vertical">
      <a-row v-for="(group, gi) in groups" :key="gi" :gutter="12">
        <a-col v-for="f in group" :key="f.id" :span="group.length > 1 ? 12 : 24">
          <a-form-item :label="`${t(f.labelKey)}${f.required ? ' *' : ''}`">
            <a-textarea
              v-if="f.textarea"
              :model-value="inputValue(f)"
              :placeholder="f.placeholder"
              :rows="2"
              @update:model-value="(v) => setInputValue(f, v)"
            />
            <a-input-password
              v-else-if="f.type === 'password'"
              :model-value="inputValue(f)"
              :placeholder="f.placeholder"
              @update:model-value="(v) => setInputValue(f, v)"
            />
            <a-input
              v-else
              :model-value="inputValue(f)"
              :placeholder="f.placeholder"
              @update:model-value="(v) => setInputValue(f, v)"
            />
          </a-form-item>
        </a-col>
      </a-row>
    </a-form>
  </a-modal>
</template>

<style scoped>
.view-box {
  border: 1px solid var(--color-border);
  border-radius: 8px;
  padding: 14px;
  background-color: var(--color-fill-1);
}
.view-label {
  font-size: 12px;
  color: var(--color-text-3);
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
  color: var(--color-text-1);
}
</style>

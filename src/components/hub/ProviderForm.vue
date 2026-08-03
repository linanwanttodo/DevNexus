<script setup>
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { showToast } from "../../lib/toast.js";
import { t, tFormat } from "../../lib/i18n.js";
import ModelList from "./ModelList.vue";

const props = defineProps({
  mode: { type: String, default: "add" }, // "add" | "edit"
  title: { type: String, default: "" },
  subtitle: { type: String, default: "" },
  initial: { type: Object, default: null },
  protocolOptions: { type: Array, default: () => [] },
  onSave: { type: Function, required: true },
  onCancel: { type: Function, required: true },
});

const isEdit = computed(() => props.mode === "edit");

function createForm() {
  return {
    name: props.initial?.name || "",
    protocol: props.initial?.protocol || "openai_chat",
    base_url: props.initial?.base_url || "https://api.openai.com",
    api_key: props.initial?.api_key || "",
    model_aliases: { ...(props.initial?.model_aliases || {}) },
    model_context_lengths: { ...(props.initial?.model_context_lengths || {}) },
  };
}
function createSelected() {
  const map = {};
  for (const m of props.initial?.models || []) map[m] = true;
  return map;
}

const form = ref(createForm());
const fetchedModels = ref([]);
const selectedModels = ref(createSelected());
const fetchingModels = ref(false);
const addingManualModel = ref(false);
const manualModelId = ref("");

function onProtocolChange() {
  const opt = props.protocolOptions.find((p) => p.id === form.value.protocol);
  if (opt && !isEdit.value) form.value.base_url = opt.defaultUrl;
}

async function fetchModels() {
  if (!form.value.base_url || !form.value.protocol) {
    showToast(t("apiHub.errors.fillBaseUrl"), "error");
    return;
  }
  fetchingModels.value = true;
  fetchedModels.value = [];
  try {
    fetchedModels.value = await invoke("api_hub_fetch_models", {
      baseUrl: form.value.base_url,
      apiKey: form.value.api_key || "",
      protocol: form.value.protocol,
      providerId: props.initial?.id,
    });
    fetchedModels.value.forEach((m) => {
      if (!(m.id in selectedModels.value)) {
        selectedModels.value[m.id] = true;
        form.value.model_aliases[m.id] = m.name || m.id;
      }
    });
    showToast(
      tFormat("apiHub.toast.fetchedModels", { count: fetchedModels.value.length })
    );
  } catch (err) {
    showToast(tFormat("apiHub.toast.fetchFailed", { error: err.message }), "error");
  } finally {
    fetchingModels.value = false;
  }
}

function toggleModel(id) {
  selectedModels.value[id] = !selectedModels.value[id];
}

function confirmManualAdd() {
  const id = manualModelId.value.trim();
  if (!id) return;
  if (fetchedModels.value.find((m) => m.id === id)) {
    showToast(tFormat("apiHub.toast.modelExists", { id }), "error");
    return;
  }
  const model = { id, name: id, owned_by: t("apiHub.custom"), enabled: true };
  fetchedModels.value = [...fetchedModels.value, model];
  selectedModels.value[id] = true;
  form.value.model_aliases[id] = id;
  manualModelId.value = "";
  showToast(tFormat("apiHub.toast.modelAdded", { id }));
}

function selectAll() {
  fetchedModels.value.forEach((m) => (selectedModels.value[m.id] = true));
}
function deselectAll() {
  fetchedModels.value.forEach((m) => (selectedModels.value[m.id] = false));
}
function selectedCount() {
  return Object.values(selectedModels.value).filter(Boolean).length;
}

function submit() {
  const models = Object.keys(selectedModels.value).filter((m) => selectedModels.value[m]);
  if (models.length === 0) {
    showToast(t("apiHub.errors.selectModel"), "error");
    return;
  }
  const model_aliases = {};
  models.forEach((m) => {
    model_aliases[m] = form.value.model_aliases[m] || m;
  });
  const model_context_lengths = {};
  models.forEach((m) => {
    if (form.value.model_context_lengths[m]) {
      model_context_lengths[m] = Number(form.value.model_context_lengths[m]);
    }
  });
  const data = {
    id: props.initial?.id || crypto.randomUUID(),
    name: form.value.name,
    protocol: form.value.protocol,
    base_url: form.value.base_url,
    api_key: form.value.api_key,
    models,
    model_aliases,
    model_context_lengths,
    enabled: true,
    created_at: Math.floor(Date.now() / 1000),
  };
  props.onSave(data, isEdit.value);
}

const currentProtocol = computed(() =>
  props.protocolOptions.find((p) => p.id === form.value.protocol)
);
</script>

<template>
  <div class="provider-form" :class="{ 'is-edit': isEdit }">
    <!-- Header -->
    <div class="form-header">
      <div class="form-title-row">
        <icon-edit v-if="isEdit" class="header-icon" />
        <icon-plus-circle v-else class="header-icon" />
        <span class="form-title">
          {{ isEdit ? `${title} — ${subtitle}` : title }}
        </span>
      </div>
      <a-button type="text" @click="onCancel">
        <template #icon><icon-close /></template>
      </a-button>
    </div>

    <!-- Form fields -->
    <div class="form-grid">
      <div>
        <label class="field-label">{{ t("apiHub.name") }}</label>
        <a-input v-model="form.name" placeholder="My OpenAI" />
      </div>
      <div class="span-2">
        <label class="field-label">{{ t("apiHub.protocolLabel") }}</label>
        <a-select
          v-model="form.protocol"
          :disabled="isEdit"
          @change="onProtocolChange"
        >
          <a-option
            v-for="pt in protocolOptions"
            :key="pt.id"
            :value="pt.id"
            :label="pt.label"
          />
        </a-select>
        <p class="protocol-hint">
          <code>{{ currentProtocol?.endpoint || "" }}</code>
          — {{ currentProtocol?.desc || "" }}
        </p>
      </div>
      <div class="span-2">
        <label class="field-label">{{ t("apiHub.baseUrl") }}</label>
        <a-input v-model="form.base_url" placeholder="https://api.openai.com" />
      </div>
      <div class="span-2">
        <label class="field-label">
          {{ t("apiHub.apiKey") }}
          <span class="hint-inline">{{ isEdit ? t("apiHub.maskedHint") : t("apiHub.optional") }}</span>
        </label>
        <a-input-password
          v-model="form.api_key"
          :placeholder="isEdit ? t('apiHub.apiKeyReplacePlaceholder') : 'sk-...'"
        />
      </div>
    </div>

    <!-- Model fetching -->
    <div class="model-section">
      <div class="model-toolbar">
        <a-button
          type="primary"
          size="small"
          :loading="fetchingModels"
          @click="fetchModels"
        >
          <template #icon><icon-download /></template>
          {{ isEdit ? t("apiHub.refreshModels") : t("apiHub.fetchModels") }}
        </a-button>
        <span v-if="fetchedModels.length > 0" class="model-count">
          <template v-if="isEdit">
            {{ t("apiHub.models.selected") }} {{ selectedCount() }} / {{ fetchedModels.length }}
          </template>
          <template v-else>
            {{ tFormat("apiHub.models.fetched", { count: fetchedModels.length, selected: selectedCount() }) }}
          </template>
        </span>
        <div v-if="fetchedModels.length > 0 && !isEdit" class="toolbar-actions">
          <a-button size="mini" @click="selectAll">{{ t("apiHub.selectAll") }}</a-button>
          <a-button size="mini" @click="deselectAll">{{ t("apiHub.deselectAll") }}</a-button>
          <a-button size="mini" @click="addingManualModel = !addingManualModel" :title="t('apiHub.manualAdd')">
            <template #icon><icon-plus /></template>
          </a-button>
        </div>
      </div>

      <!-- Manual add row (add mode with models) -->
      <div v-if="fetchedModels.length > 0 && !isEdit && addingManualModel" class="manual-row">
        <a-input
          v-model="manualModelId"
          class="manual-input"
          :placeholder="t('apiHub.modelIdPlaceholder')"
          @press-enter="confirmManualAdd"
          @keydown.esc="addingManualModel = false"
        />
        <a-button type="primary" size="small" :disabled="!manualModelId.trim()" @click="confirmManualAdd">
          {{ t("apiHub.confirm") }}
        </a-button>
        <a-button size="small" @click="addingManualModel = false">{{ t("apiHub.cancel") }}</a-button>
      </div>

      <ModelList
        v-if="fetchedModels.length > 0"
        :models="fetchedModels"
        :selected="selectedModels"
        :aliases="form.model_aliases"
        :contexts="form.model_context_lengths"
        :show-ctx="!isEdit"
        :max-h="isEdit ? 'max-h-48' : 'max-h-60'"
        :extra-class="isEdit ? 'mb-3' : ''"
        @toggle="toggleModel"
      />

      <div v-else-if="!fetchingModels" class="model-empty">
        <template v-if="isEdit">
          <div class="muted-text mb-3">
            {{ tFormat("apiHub.models.existing", { count: (initial?.models || []).length }) }}
          </div>
        </template>
        <template v-else>
          <div class="empty-box">
            <icon-download class="empty-icon" />
            <div class="empty-hint">{{ t("apiHub.models.fetchHint") }}</div>
            <a-button size="mini" @click="addingManualModel = true">
              <template #icon><icon-plus /></template>
              {{ t("apiHub.manualAddHint") }}
            </a-button>
            <div v-if="addingManualModel" class="manual-row manual-center">
              <a-input
                v-model="manualModelId"
                class="manual-input"
                :placeholder="t('apiHub.modelIdPlaceholder')"
                @press-enter="confirmManualAdd"
                @keydown.esc="addingManualModel = false"
              />
              <a-button type="primary" size="small" :disabled="!manualModelId.trim()" @click="confirmManualAdd">
                {{ t("apiHub.confirm") }}
              </a-button>
            </div>
          </div>
        </template>
      </div>
    </div>

    <!-- Action buttons -->
    <div class="form-actions">
      <a-button @click="onCancel">{{ t("apiHub.cancel") }}</a-button>
      <a-button v-if="isEdit" type="primary" @click="submit">{{ t("apiHub.update") }}</a-button>
      <a-button
        v-else
        type="primary"
        :disabled="!form.name || !form.base_url || selectedCount() === 0"
        @click="submit"
      >
        {{ t("apiHub.add") }}
        <span class="count-badge">({{ selectedCount() }} {{ t("apiHub.models.countBadge") }})</span>
      </a-button>
    </div>
  </div>
</template>

<style scoped>
.provider-form {
  background-color: var(--color-bg-2);
  border: 1px solid var(--color-border-2);
  border-radius: 10px;
  padding: 16px;
  margin-bottom: 16px;
}
.provider-form.is-edit {
  margin-bottom: 0;
}
.form-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 16px;
}
.form-title-row {
  display: flex;
  align-items: center;
  gap: 6px;
}
.header-icon {
  color: rgb(var(--primary-6));
  font-size: 18px;
}
.form-title {
  font-size: 14px;
  font-weight: 500;
  color: var(--color-text-1);
}
.form-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
  margin-bottom: 16px;
}
.span-2 {
  grid-column: span 2;
}
.field-label {
  display: block;
  margin-bottom: 6px;
  font-size: 12px;
  color: var(--color-text-3);
}
.hint-inline {
  opacity: 0.6;
}
.protocol-hint {
  margin: 4px 0 0;
  font-size: 10px;
  color: var(--color-text-4);
  font-family: "JetBrains Mono", monospace;
}
.model-section {
  margin-bottom: 12px;
}
.model-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
}
.model-count {
  font-size: 12px;
  color: var(--color-text-3);
}
.toolbar-actions {
  margin-left: auto;
  display: flex;
  gap: 4px;
}
.manual-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--color-border-2);
  background-color: var(--color-fill-1);
}
.manual-center {
  justify-content: center;
  border-bottom: none;
  margin-top: 10px;
}
.manual-input {
  flex: 1;
  max-width: 400px;
}
.model-empty {
  font-size: 12px;
  color: var(--color-text-3);
}
.muted-text {
  color: var(--color-text-3);
}
.empty-box {
  border: 1px dashed var(--color-border-3);
  border-radius: 8px;
  padding: 20px;
  text-align: center;
}
.empty-icon {
  font-size: 24px;
  color: var(--color-text-4);
}
.empty-hint {
  margin: 6px 0 10px;
  font-size: 12px;
  color: var(--color-text-3);
}
.form-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding-top: 12px;
  border-top: 1px solid var(--color-border-2);
}
.count-badge {
  opacity: 0.6;
}
</style>

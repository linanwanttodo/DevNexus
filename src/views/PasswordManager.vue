<script setup>
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { showToast } from "../lib/toast.js";
import { showConfirm } from "../lib/confirm.js";
import { t } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";
import VaultDialog from "../components/VaultDialog.vue";

const locked = ref(true);
const hasMasterPassword = ref(false);
const masterPassword = ref("");
const setupPassword = ref("");
const setupPasswordConfirm = ref("");
const passwords = ref([]);
const loading = ref(true);
const showAddModal = ref(false);
const showPassword = ref(null);

// 表单数据
const entryName = ref("");
const username = ref("");
const password = ref("");
const url = ref("");
const notes = ref("");

// 编辑表单
const showEditModal = ref(false);
const editId = ref(null);
const editName = ref("");
const editUsername = ref("");
const editPassword = ref("");
const editUrl = ref("");
const editNotes = ref("");

async function checkState() {
  try {
    const lockedState = await invoke("is_locked");
    const hasPwd = await invoke("has_master_password");
    locked.value = lockedState;
    hasMasterPassword.value = hasPwd;
  } catch (err) {
    console.error("Failed to check password manager state:", err);
    showToast(t("common.error_msg").replace("{error}", friendlyError(err)), "error");
  }
}

async function setupMasterPassword() {
  if (!setupPassword.value || setupPassword.value.length < 4) {
    showToast(t("common.master_too_short"));
    return;
  }
  if (setupPassword.value !== setupPasswordConfirm.value) {
    showToast(t("common.no_match"));
    return;
  }
  try {
    await invoke("set_master_password", { masterPassword: setupPassword.value });
    locked.value = false;
    hasMasterPassword.value = true;
    setupPassword.value = "";
    setupPasswordConfirm.value = "";
    await loadPasswords();
    showToast(t("common.master_set_ok"));
  } catch (err) {
    showToast(t("common.set_master_failed").replace("{error}", friendlyError(err)));
  }
}

async function unlock() {
  if (!masterPassword.value) return;
  try {
    const success = await invoke("unlock", { masterPassword: masterPassword.value });
    if (success) {
      locked.value = false;
      masterPassword.value = "";
      await loadPasswords();
      showToast(t("common.unlocked"));
    } else {
      showToast(t("common.incorrect"));
    }
  } catch (err) {
    showToast(t("common.unlock_failed").replace("{error}", friendlyError(err)));
  }
}

async function lockVault() {
  try {
    await invoke("lock");
    locked.value = true;
    passwords.value = [];
    showToast(t("common.locked"));
  } catch (err) {
    console.error("Failed to lock:", err);
  }
}

async function loadPasswords() {
  try {
    loading.value = true;
    passwords.value = await invoke("list_passwords");
  } catch (err) {
    console.error("Failed to load passwords:", err);
    showToast(t("passwords.failed_load"));
  } finally {
    loading.value = false;
  }
}

async function addPassword() {
  if (!entryName.value || !username.value || !password.value) {
    showToast(t("passwords.fill_fields"));
    return;
  }
  try {
    await invoke("add_password", {
      name: entryName.value,
      username: username.value,
      password: password.value,
      url: url.value || null,
      notes: notes.value || null,
    });
    showAddModal.value = false;
    resetForm();
    await loadPasswords();
    showToast(t("passwords.add_success"));
  } catch (err) {
    showToast(t("passwords.add_failed").replace("{error}", friendlyError(err)));
  }
}

async function deletePassword(id) {
  if (!(await showConfirm(t("passwords.delete_confirm")))) return;
  try {
    await invoke("delete_password", { id });
    await loadPasswords();
  } catch (err) {
    showToast(t("passwords.delete_failed").replace("{error}", friendlyError(err)));
  }
}

function beginEdit(entry) {
  editId.value = entry.id;
  editName.value = entry.name;
  editUsername.value = entry.username;
  editPassword.value = "";
  editUrl.value = entry.url || "";
  editNotes.value = entry.notes || "";
  showEditModal.value = true;
}

async function saveEdit() {
  if (!editName.value || !editUsername.value) {
    showToast(t("passwords.fill_fields"));
    return;
  }
  try {
    await invoke("update_password", {
      id: editId.value,
      name: editName.value,
      username: editUsername.value,
      password: editPassword.value.trim() ? editPassword.value.trim() : null,
      url: editUrl.value.trim() ? editUrl.value.trim() : null,
      notes: editNotes.value.trim() ? editNotes.value.trim() : null,
    });
    showEditModal.value = false;
    await loadPasswords();
    showToast(t("passwords.edit_success"));
  } catch (err) {
    showToast(t("passwords.edit_failed").replace("{error}", friendlyError(err)));
  }
}

async function viewPassword(id) {
  try {
    const pwd = await invoke("get_password", { id });
    showPassword.value = { id, password: pwd };
  } catch (err) {
    showToast(t("passwords.view_failed").replace("{error}", friendlyError(err)));
  }
}

async function exportCSV() {
  try {
    const csvContent = await invoke("export_chrome_csv");
    const blob = new Blob([csvContent], { type: "text/csv" });
    const downloadUrl = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = downloadUrl;
    a.download = `passwords_export_${new Date().toISOString().split("T")[0]}.csv`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(downloadUrl);
    showToast(t("passwords.export_success"));
  } catch (err) {
    showToast(t("passwords.export_failed").replace("{error}", friendlyError(err)));
  }
}

async function importCSV({ file }) {
  const raw = file?.originFile || file?.file || file;
  if (!raw) return;
  try {
    const text = await raw.text();
    const count = await invoke("import_chrome_csv", { csvContent: text });
    await loadPasswords();
    showToast(t("passwords.import_success").replace("{count}", count));
  } catch (err) {
    showToast(t("passwords.import_failed").replace("{error}", friendlyError(err)));
  }
}

async function saveToFile() {
  const fileMasterPwd = prompt(t("passwords.master_pwd_save"));
  if (!fileMasterPwd) return;
  try {
    const filePath = prompt(t("passwords.file_path_save"));
    if (!filePath) return;
    await invoke("save_to_file", { filePath, masterPassword: fileMasterPwd });
    showToast(t("passwords.save_success"));
  } catch (err) {
    showToast(t("passwords.save_failed").replace("{error}", friendlyError(err)));
  }
}

async function loadFromFile() {
  const fileMasterPwd = prompt(t("passwords.master_pwd_load"));
  if (!fileMasterPwd) return;
  try {
    const filePath = prompt(t("passwords.file_path_load"));
    if (!filePath) return;
    const count = await invoke("load_from_file", { filePath, masterPassword: fileMasterPwd });
    await loadPasswords();
    showToast(t("passwords.load_success").replace("{count}", count));
  } catch (err) {
    showToast(t("passwords.load_failed").replace("{error}", friendlyError(err)));
  }
}

function resetForm() {
  entryName.value = "";
  username.value = "";
  password.value = "";
  url.value = "";
  notes.value = "";
}

function copyToClipboard(text) {
  navigator.clipboard
    .writeText(text)
    .then(() => {
      showToast(t("passwords.copied"));
    })
    .catch(() => {
      showToast(t("common.copy_failed"), "error");
    });
}

const addGroups = [
  [
    { id: "pm-name-add", labelKey: "passwords.name", required: true, value: entryName, onInput: (v) => (entryName.value = v), placeholder: "GitHub Account" },
    { id: "pm-username-add", labelKey: "passwords.username", required: true, value: username, onInput: (v) => (username.value = v), placeholder: "user@example.com" },
  ],
  [
    { id: "pm-password-add", labelKey: "passwords.password", required: true, type: "password", value: password, onInput: (v) => (password.value = v), placeholder: "••••••••" },
    { id: "pm-url-add", labelKey: "URL", type: "url", value: url, onInput: (v) => (url.value = v), placeholder: "https://github.com" },
  ],
  [
    { id: "pm-notes-add", labelKey: "passwords.notes", textarea: true, value: notes, onInput: (v) => (notes.value = v), placeholder: "Additional information..." },
  ],
];

const editGroups = [
  [
    { id: "pm-name-edit", labelKey: "passwords.name", required: true, value: editName, onInput: (v) => (editName.value = v) },
    { id: "pm-username-edit", labelKey: "passwords.username", required: true, value: editUsername, onInput: (v) => (editUsername.value = v) },
  ],
  [
    { id: "pm-password-edit", labelKey: "passwords.password", type: "password", value: editPassword, onInput: (v) => (editPassword.value = v), placeholder: t("passwords.keep_password") },
    { id: "pm-url-edit", labelKey: "URL", type: "url", value: editUrl, onInput: (v) => (editUrl.value = v) },
  ],
  [
    { id: "pm-notes-edit", labelKey: "passwords.notes", textarea: true, value: editNotes, onInput: (v) => (editNotes.value = v) },
  ],
];

onMounted(async () => {
  await checkState();
  if (!locked.value) {
    await loadPasswords();
  } else {
    loading.value = false;
  }
});
</script>

<template>
  <div class="page pm-page">
    <!-- Header -->
    <div class="page-header">
      <div>
        <h1 class="page-title">{{ t("passwords.title") }}</h1>
        <p class="page-desc">{{ t("passwords.desc") }}</p>
      </div>
      <div v-if="!locked" class="flex gap-2 items-center">
        <a-button @click="exportCSV">
          <template #icon><icon-download /></template>
          {{ t("passwords.export_csv") }}
        </a-button>
        <a-upload :show-file-list="false" :auto-upload="false" accept=".csv" @change="importCSV">
          <a-button>
            <template #icon><icon-upload /></template>
            {{ t("passwords.import_csv") }}
          </a-button>
        </a-upload>
        <a-button @click="saveToFile">
          <template #icon><icon-save /></template>
          {{ t("passwords.save_encrypted") }}
        </a-button>
        <a-button type="primary" @click="showAddModal = true">
          <template #icon><icon-plus /></template>
          {{ t("passwords.add") }}
        </a-button>
        <a-button status="danger" @click="lockVault">
          <template #icon><icon-lock /></template>
          {{ t("passwords.lock") }}
        </a-button>
      </div>
    </div>

    <!-- Lock/Setup Screen -->
    <div v-if="locked" class="lock-screen">
      <!-- Unlock -->
      <a-card v-if="hasMasterPassword" :bordered="true" class="lock-card">
        <div class="lock-icon"><icon-lock /></div>
        <h2 class="lock-title">{{ t("passwords.title_locked") }}</h2>
        <p class="lock-desc">{{ t("passwords.desc_locked") }}</p>
        <a-input-password
          v-model="masterPassword"
          :placeholder="t('passwords.master_password_placeholder')"
          size="large"
          class="lock-input"
          @press-enter="unlock"
        />
        <a-button type="primary" long size="large" :disabled="!masterPassword" @click="unlock">
          {{ t("passwords.unlock") }}
        </a-button>
      </a-card>

      <!-- Setup Master Password -->
      <a-card v-else :bordered="true" class="lock-card">
        <div class="lock-icon"><icon-lock /></div>
        <h2 class="lock-title">{{ t("passwords.title_setup") }}</h2>
        <p class="lock-desc">{{ t("passwords.desc_setup") }}</p>
        <a-input-password
          v-model="setupPassword"
          :placeholder="t('passwords.setup_password_placeholder')"
          size="large"
          class="lock-input"
        />
        <a-input-password
          v-model="setupPasswordConfirm"
          :placeholder="t('passwords.setup_password_confirm_placeholder')"
          size="large"
          class="lock-input"
          @press-enter="setupMasterPassword"
        />
        <a-button
          type="primary"
          long
          size="large"
          :disabled="!setupPassword || !setupPasswordConfirm"
          @click="setupMasterPassword"
        >
          {{ t("passwords.setup") }}
        </a-button>
      </a-card>
    </div>

    <a-spin v-else-if="loading" style="display: flex; justify-content: center; padding: 56px 0" />

    <a-empty
      v-else-if="passwords.length === 0"
      style="padding: 56px 0"
      :description="t('passwords.no_passwords')"
    >
      <template #description>
        <div>{{ t("passwords.no_passwords") }}</div>
        <div class="empty-hint">{{ t("passwords.no_passwords_desc") }}</div>
      </template>
    </a-empty>

    <a-card v-else :bordered="true">
      <a-table :data="passwords" :pagination="false" :row-key="'id'" :bordered="false" size="small">
        <template #columns>
          <a-table-column :title="t('passwords.name')">
            <template #cell="{ record }">
              <div class="name-cell">
                <icon-lock class="name-icon" />
                <span class="name-text">{{ record.name }}</span>
              </div>
            </template>
          </a-table-column>
          <a-table-column :title="t('passwords.username')" data-index="username">
            <template #cell="{ record }">
              <span class="username">{{ record.username }}</span>
            </template>
          </a-table-column>
          <a-table-column title="URL">
            <template #cell="{ record }">
              <a-link v-if="record.url" :href="record.url" target="_blank" class="url-link">
                {{ record.url }}
              </a-link>
              <span v-else class="muted">{{ t("passwords.no_url") }}</span>
            </template>
          </a-table-column>
          <a-table-column :title="t('passwords.created')" data-index="created_at">
            <template #cell="{ record }">
              <span class="muted">{{ record.created_at }}</span>
            </template>
          </a-table-column>
          <a-table-column :title="t('passwords.actions')" align="right" :width="190">
            <template #cell="{ record }">
              <div class="actions">
                <a-tooltip :content="t('passwords.title_view')">
                  <a-button type="text" size="mini" @click="viewPassword(record.id)">
                    <template #icon><icon-eye /></template>
                  </a-button>
                </a-tooltip>
                <a-tooltip :content="t('passwords.title_edit')">
                  <a-button type="text" size="mini" @click="beginEdit(record)">
                    <template #icon><icon-edit /></template>
                  </a-button>
                </a-tooltip>
                <a-tooltip :content="t('passwords.title_copy')">
                  <a-button type="text" size="mini" @click="copyToClipboard(record.username)">
                    <template #icon><icon-copy /></template>
                  </a-button>
                </a-tooltip>
                <a-tooltip :content="t('passwords.title_delete')">
                  <a-button type="text" size="mini" status="danger" @click="deletePassword(record.id)">
                    <template #icon><icon-delete /></template>
                  </a-button>
                </a-tooltip>
              </div>
            </template>
          </a-table-column>
        </template>
      </a-table>
    </a-card>

    <!-- Add Password Modal -->
    <VaultDialog
      v-if="showAddModal"
      :title="t('passwords.add')"
      :groups="addGroups"
      :submit-label="t('passwords.save')"
      @submit="addPassword"
      @close="showAddModal = false"
    />

    <!-- Edit Password Modal -->
    <VaultDialog
      v-if="showEditModal"
      :title="t('passwords.edit_title')"
      :groups="editGroups"
      :submit-label="t('passwords.save')"
      @submit="saveEdit"
      @close="showEditModal = false"
    />

    <!-- View Password Modal -->
    <VaultDialog
      v-if="showPassword"
      mode="view"
      :title="t('passwords.details')"
      :password="showPassword.password"
      @copy="copyToClipboard(showPassword.password)"
      @close="showPassword = null"
    />
  </div>
</template>

<style scoped>
.empty-hint {
  font-size: 12px;
  color: var(--color-text-3);
  margin-top: 4px;
}
.lock-screen {
  display: flex;
  justify-content: center;
  margin-top: 48px;
}
.lock-card {
  width: 100%;
  max-width: 420px;
  padding: 24px 8px;
  text-align: center;
  border-radius: 12px;
}
.lock-icon {
  font-size: 44px;
  color: var(--color-text-2);
  margin-bottom: 12px;
}
.lock-title {
  font-size: 18px;
  font-weight: 600;
  color: var(--color-text-1);
  margin: 0 0 4px;
}
.lock-desc {
  font-size: 13px;
  color: var(--color-text-3);
  margin: 0 0 20px;
}
.lock-input {
  margin-bottom: 12px;
}
.name-cell {
  display: flex;
  align-items: center;
  gap: 8px;
}
.name-icon {
  color: var(--color-text-2);
}
.name-text {
  font-weight: 500;
  color: var(--color-text-1);
}
.username {
  color: var(--color-text-2);
  font-size: 13px;
}
.url-link {
  font-size: 12px;
}
.muted {
  color: var(--color-text-3);
  font-size: 12px;
}
.actions {
  display: flex;
  justify-content: flex-end;
  gap: 2px;
}
</style>

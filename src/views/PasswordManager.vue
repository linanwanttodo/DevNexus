<script setup>
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { showToast } from "../lib/toast.js";
import { showConfirm } from "../lib/confirm.js";
import { t } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";
import VaultDialog from "../components/VaultDialog.vue";
import AppIcon from "../components/AppIcon.vue";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyMedia,
} from "@/components/ui/empty";

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

// a-upload 适配：原生 file input 的 change → importCSV({ file })
function onImportFile(e) {
  const file = e.target.files && e.target.files[0];
  if (file) importCSV({ file });
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
  await loadPasswords();
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
      <div class="flex gap-2 items-center">
        <Button variant="outline" @click="exportCSV">
          <AppIcon name="download" class="size-4" />
          {{ t("passwords.export_csv") }}
        </Button>
        <label class="cursor-pointer">
          <span
            class="inline-flex h-10 items-center gap-2 rounded-md border border-input bg-background px-4 text-sm font-medium text-foreground shadow-sm transition-colors hover:bg-accent hover:text-accent-foreground"
          >
            <AppIcon name="upload" class="size-4" />
            {{ t("passwords.import_csv") }}
          </span>
          <input type="file" accept=".csv" class="sr-only" @change="onImportFile" />
        </label>
        <Button @click="showAddModal = true">
          <AppIcon name="plus" class="size-4" />
          {{ t("passwords.add") }}
        </Button>
      </div>
    </div>

    <div v-if="loading" class="flex justify-center py-14">
      <Spinner />
    </div>

    <Empty v-else-if="passwords.length === 0" class="py-14">
      <EmptyMedia>
        <AppIcon name="lock" class="size-10 text-muted-foreground/60" />
      </EmptyMedia>
      <EmptyContent>
        <EmptyDescription>
          <div>{{ t("passwords.no_passwords") }}</div>
          <div class="empty-hint">{{ t("passwords.no_passwords_desc") }}</div>
        </EmptyDescription>
      </EmptyContent>
    </Empty>

    <Card v-else class="shadow-sm">
      <CardContent class="p-0">
        <TooltipProvider>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>{{ t("passwords.name") }}</TableHead>
                <TableHead>{{ t("passwords.username") }}</TableHead>
                <TableHead>URL</TableHead>
                <TableHead>{{ t("passwords.created") }}</TableHead>
                <TableHead class="text-right">{{ t("passwords.actions") }}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-for="record in passwords" :key="record.id">
                <TableCell>
                  <div class="name-cell">
                    <AppIcon name="lock" class="name-icon size-4" />
                    <span class="name-text">{{ record.name }}</span>
                  </div>
                </TableCell>
                <TableCell>
                  <span class="username">{{ record.username }}</span>
                </TableCell>
                <TableCell>
                  <a
                    v-if="record.url"
                    :href="record.url"
                    target="_blank"
                    class="url-link text-primary hover:underline"
                  >
                    {{ record.url }}
                  </a>
                  <span v-else class="muted">{{ t("passwords.no_url") }}</span>
                </TableCell>
                <TableCell>
                  <span class="muted">{{ record.created_at }}</span>
                </TableCell>
                <TableCell class="text-right">
                  <div class="actions">
                    <Tooltip>
                      <TooltipTrigger as-child>
                        <Button variant="ghost" size="icon-sm" @click="viewPassword(record.id)">
                          <AppIcon name="eye" class="size-4" />
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent>{{ t("passwords.title_view") }}</TooltipContent>
                    </Tooltip>
                    <Tooltip>
                      <TooltipTrigger as-child>
                        <Button variant="ghost" size="icon-sm" @click="beginEdit(record)">
                          <AppIcon name="edit" class="size-4" />
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent>{{ t("passwords.title_edit") }}</TooltipContent>
                    </Tooltip>
                    <Tooltip>
                      <TooltipTrigger as-child>
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          @click="copyToClipboard(record.username)"
                        >
                          <AppIcon name="copy" class="size-4" />
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent>{{ t("passwords.title_copy") }}</TooltipContent>
                    </Tooltip>
                    <Tooltip>
                      <TooltipTrigger as-child>
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          class="text-destructive hover:text-destructive"
                          @click="deletePassword(record.id)"
                        >
                          <AppIcon name="delete" class="size-4" />
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent>{{ t("passwords.title_delete") }}</TooltipContent>
                    </Tooltip>
                  </div>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </TooltipProvider>
      </CardContent>
    </Card>

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
  color: var(--color-muted-foreground);
  margin-top: 4px;
}
.name-cell {
  display: flex;
  align-items: center;
  gap: 8px;
}
.name-icon {
  color: var(--color-muted-foreground);
}
.name-text {
  font-weight: 500;
  color: var(--color-foreground);
}
.username {
  color: var(--color-muted-foreground);
  font-size: 13px;
}
.url-link {
  font-size: 12px;
}
.muted {
  color: var(--color-muted-foreground);
  font-size: 12px;
}
.actions {
  display: flex;
  justify-content: flex-end;
  gap: 2px;
}
</style>
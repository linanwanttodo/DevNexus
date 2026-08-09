<script setup>
import { ref, computed, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { showToast } from "../lib/toast.js";
import { t } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";
import AppIcon from "../components/AppIcon.vue";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardFooter } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Checkbox } from "@/components/ui/checkbox";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@/components/ui/tabs";
import {
  Alert,
  AlertDescription,
  AlertTitle,
} from "@/components/ui/alert";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyMedia,
} from "@/components/ui/empty";

const activeTab = ref("export");
const environments = ref([]);
const loading = ref(true);
const error = ref(null);

// 导出
const selectedEnvs = ref([]);
const versionsMap = ref({});
const loadingVersions = ref({});
const selectedVersions = ref({});

// 导入
const importManifest = ref(null);
const importPath = ref("");
const applyVersions = ref(true);
const importing = ref(false);
const importResult = ref(null);

const versionManagedTypes = ["python", "node", "java", "go", "rust", "cpp"];

async function loadEnvironments() {
  try {
    loading.value = true;
    error.value = null;
    environments.value = await invoke("list_environments");
  } catch (err) {
    error.value = friendlyError(err);
  } finally {
    loading.value = false;
  }
}

function toggleEnv(name) {
  if (selectedEnvs.value.includes(name)) {
    selectedEnvs.value = selectedEnvs.value.filter((n) => n !== name);
    versionsMap.value[name] = undefined;
    selectedVersions.value[name] = [];
  } else {
    selectedEnvs.value = [...selectedEnvs.value, name];
    loadVersions(name);
  }
}

async function loadVersions(name) {
  const env = environments.value.find((e) => e.name === name);
  if (!env || !versionManagedTypes.includes(env.lang_type)) return;
  loadingVersions.value = { ...loadingVersions.value, [name]: true };
  try {
    const vers = await invoke("list_versions", { langType: env.lang_type });
    versionsMap.value = { ...versionsMap.value, [name]: vers || [] };
    if (!selectedVersions.value[name]) selectedVersions.value[name] = [];
  } catch (err) {
    versionsMap.value = { ...versionsMap.value, [name]: [] };
    showToast(t("migration.versions_failed").replace("{error}", friendlyError(err)), "error");
  } finally {
    loadingVersions.value = { ...loadingVersions.value, [name]: false };
  }
}

function toggleVersion(name, ver) {
  const env = environments.value.find((e) => e.name === name);
  const snap = { lang_type: env.lang_type, version: ver.version };
  const arr = [...(selectedVersions.value[name] || [])];
  const idx = arr.findIndex((v) => v.version === ver.version);
  if (idx >= 0) arr.splice(idx, 1);
  else arr.push(snap);
  selectedVersions.value = { ...selectedVersions.value, [name]: arr };
}

const selectedVersionCount = computed(() =>
  Object.values(selectedVersions.value).reduce(
    (sum, arr) => sum + (arr ? arr.length : 0),
    0
  )
);

function isVersionSelected(name, ver) {
  return (selectedVersions.value[name] || []).some((v) => v.version === ver.version);
}

async function exportMigration() {
  if (selectedEnvs.value.length === 0) {
    showToast(t("migration.select_env"), "error");
    return;
  }
  const versions = Object.values(selectedVersions.value).flat();
  try {
    const json = await invoke("export_migration", {
      selected: { environments: selectedEnvs.value, versions },
    });
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `devnexus-migration-${new Date().toISOString().slice(0, 10)}.json`;
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
    showToast(t("migration.exported"));
  } catch (err) {
    showToast(t("migration.export_failed").replace("{error}", friendlyError(err)), "error");
  }
}

async function pickImportFile() {
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!selected) return;
    importPath.value = selected;
    importResult.value = null;
    importManifest.value = await invoke("load_migration_file", { path: importPath.value });
  } catch (err) {
    importManifest.value = null;
    showToast(t("migration.import_failed").replace("{error}", friendlyError(err)), "error");
  }
}

async function runImport() {
  if (!importManifest.value) {
    showToast(t("migration.empty_file"), "error");
    return;
  }
  importing.value = true;
  importResult.value = null;
  try {
    const json = JSON.stringify(importManifest.value);
    const result = await invoke("import_migration", {
      json,
      applyVersions: applyVersions.value,
    });
    importResult.value = result;
    showToast(
      t("migration.import_success")
        .replace("{switched}", result.switched)
        .replace("{skipped}", result.skipped)
        .replace("{failed}", result.failed)
    );
    await loadEnvironments();
  } catch (err) {
    showToast(t("migration.import_failed").replace("{error}", friendlyError(err)), "error");
  } finally {
    importing.value = false;
  }
}

onMounted(loadEnvironments);
</script>

<template>
  <div class="page migration-page">
    <!-- Header -->
    <div class="page-header">
      <div>
        <h1 class="page-title">{{ t("migration.title") }}</h1>
        <p class="page-desc">{{ t("migration.desc") }}</p>
      </div>
    </div>

    <!-- Tabs -->
    <Tabs v-model="activeTab" class="migration-tabs">
      <div class="mb-3 flex items-center justify-between gap-3">
        <TabsList>
          <TabsTrigger value="export">{{ t("migration.tab_export") }}</TabsTrigger>
          <TabsTrigger value="import">{{ t("migration.tab_import") }}</TabsTrigger>
        </TabsList>

        <!-- 当前 tab 的主操作：与 tabs 同行右侧 -->
        <Button
          v-if="activeTab === 'export'"
          :disabled="selectedEnvs.length === 0"
          @click="exportMigration"
        >
          <AppIcon name="download" class="size-4" />
          {{ t("migration.export") }}
        </Button>
        <Button v-else variant="default" @click="pickImportFile">
          <AppIcon name="folder-open" class="size-4" />
          {{ t("migration.import_pick") }}
        </Button>
      </div>

      <TabsContent value="export">
        <!-- Loading -->
        <div v-if="loading" class="space-y-3 py-4">
          <Skeleton class="h-10 w-full" />
          <Skeleton class="h-10 w-full" />
          <Skeleton class="h-10 w-full" />
        </div>

        <!-- Error -->
        <Alert v-else-if="error" variant="destructive" class="my-6">
          <AppIcon name="close-circle-fill" class="size-4" />
          <AlertTitle>{{ t("error.title") }}</AlertTitle>
          <AlertDescription>{{ error }}</AlertDescription>
          <Button variant="default" class="mt-3" @click="loadEnvironments">
            {{ t("common.retry") }}
          </Button>
        </Alert>

        <!-- Empty -->
        <Empty v-else-if="environments.length === 0" class="py-5">
          <EmptyMedia>
            <AppIcon name="code" class="size-10 text-muted-foreground/60" />
          </EmptyMedia>
          <EmptyContent>
            <EmptyDescription>
              {{ t("migration.no_envs") }}
            </EmptyDescription>
          </EmptyContent>
        </Empty>

        <!-- Env list -->
        <Card v-else class="shadow-sm">
          <CardContent class="pt-4">
            <div class="flex flex-col">
              <div
                v-for="env in environments"
                :key="env.name"
                class="border-b border-border py-3 last:border-b-0"
              >
                <label class="flex cursor-pointer items-start gap-3">
                  <Checkbox
                    :model-value="selectedEnvs.includes(env.name)"
                    class="mt-0.5"
                    @update:model-value="(v) => { if (v) { if (!selectedEnvs.includes(env.name)) toggleEnv(env.name); } else toggleEnv(env.name); }"
                  />
                  <div class="min-w-0 flex-1">
                    <div class="flex items-center gap-2">
                      <span class="text-sm font-medium text-foreground">{{ env.name }}</span>
                      <code class="rounded bg-muted px-1.5 py-0.5 font-mono text-xs text-muted-foreground">
                        v{{ env.version }}
                      </code>
                      <Badge variant="secondary">{{ env.lang_type }}</Badge>
                    </div>
                    <div class="mt-0.5 truncate font-mono text-xs text-muted-foreground">
                      {{ env.path }}
                    </div>
                  </div>
                </label>

                <div
                  v-if="selectedEnvs.includes(env.name) && versionManagedTypes.includes(env.lang_type)"
                  class="ml-7 mt-2"
                >
                  <div v-if="loadingVersions[env.name]" class="flex items-center text-xs text-muted-foreground">
                    <AppIcon name="refresh" :spin="true" class="mr-2 size-3.5" />
                    {{ t("common.loading") }}
                  </div>
                  <div
                    v-else-if="versionsMap[env.name] && versionsMap[env.name].length > 0"
                    class="flex flex-wrap gap-1.5"
                  >
                    <Button
                      v-for="ver in versionsMap[env.name]"
                      :key="ver.version"
                      :variant="isVersionSelected(env.name, ver) ? 'default' : 'secondary'"
                      size="sm"
                      class="h-6 rounded-full px-2.5 text-xs"
                      @click="toggleVersion(env.name, ver)"
                    >
                      {{ ver.version }}
                    </Button>
                  </div>
                  <div v-else class="flex items-center text-xs text-muted-foreground">
                    {{ t("migration.no_versions") }}
                  </div>
                </div>
              </div>
            </div>
          </CardContent>
          <CardFooter class="flex items-center justify-between">
            <span class="text-xs text-muted-foreground">
              {{
                t("migration.summary")
                  .replace("{envs}", selectedEnvs.length)
                  .replace("{versions}", selectedVersionCount)
              }}
            </span>
            <Button variant="outline" size="sm" :disabled="loading" @click="loadEnvironments">
              <AppIcon name="refresh" class="size-4" />
              {{ t("common.refresh") }}
            </Button>
          </CardFooter>
        </Card>
      </TabsContent>

      <TabsContent value="import">
        <Card class="shadow-sm">
          <CardContent class="pt-4">
            <p class="mb-3.5 text-xs text-muted-foreground">
              {{ t("migration.import_note") }}
            </p>

            <div class="flex flex-wrap items-center gap-3">
              <code
                v-if="importPath"
                class="max-w-[400px] truncate rounded bg-muted px-1.5 py-0.5 font-mono text-xs text-muted-foreground"
              >
                {{ importPath }}
              </code>
            </div>

            <div v-if="importManifest" class="mt-4 rounded-lg border border-border bg-muted p-3">
              <div class="mb-2 text-sm font-medium text-foreground">
                {{ t("migration.import_preview") }}
              </div>
              <div class="grid grid-cols-2 gap-1 text-xs text-muted-foreground">
                <div>{{ t("migration.exported_at") }}: {{ importManifest.meta?.exported_at || "—" }}</div>
                <div>{{ t("migration.meta_os") }}: {{ importManifest.meta?.source_os || "—" }}</div>
                <div>{{ t("migration.meta_host") }}: {{ importManifest.meta?.hostname || "—" }}</div>
                <div>DevNexus: {{ importManifest.meta?.devnexus_version || "—" }}</div>
              </div>
              <div class="mt-2.5 text-xs text-foreground">
                {{ importManifest.environments?.length || 0 }} envs ·
                {{ importManifest.versions?.length || 0 }} versions
              </div>
              <ul v-if="importManifest.environments?.length" class="mt-2 flex max-h-[150px] list-none flex-col gap-1 overflow-y-auto p-0">
                <li v-for="env in importManifest.environments" :key="env.name" class="flex items-center gap-2 text-xs">
                  <span class="font-medium text-foreground">{{ env.name }}</span>
                  <code class="rounded bg-muted px-1.5 py-0.5 font-mono text-xs text-muted-foreground">
                    {{ env.version }}
                  </code>
                  <Badge variant="secondary">{{ env.lang_type }}</Badge>
                </li>
              </ul>
              <div v-if="importManifest.versions?.length" class="mt-2 flex flex-wrap gap-1.5">
                <Badge
                  v-for="ver in importManifest.versions"
                  :key="ver.lang_type + '@' + ver.version"
                  variant="secondary"
                >
                  {{ ver.lang_type }}@{{ ver.version }}
                </Badge>
              </div>
            </div>

            <div v-if="importManifest" class="mt-4 flex items-center gap-4">
              <label class="flex cursor-pointer items-center gap-2 text-sm">
                <Checkbox v-model="applyVersions" />
                {{ t("migration.apply_versions") }}
              </label>
              <Button variant="default" :disabled="importing" @click="runImport">
                <Spinner v-if="importing" class="size-4" />
                <AppIcon v-else name="upload" class="size-4" />
                {{ t("migration.import") }}
              </Button>
            </div>

            <div v-if="importResult" class="mt-4 rounded-lg border border-border p-3">
              <div class="mb-2 text-[13px] font-medium text-foreground">
                switched {{ importResult.switched }} · skipped {{ importResult.skipped }} ·
                failed {{ importResult.failed }}
              </div>
              <ul class="flex max-h-[190px] list-none flex-col gap-1 overflow-y-auto p-0">
                <li v-for="(line, i) in importResult.details" :key="i" class="font-mono text-xs text-muted-foreground">
                  {{ line }}
                </li>
              </ul>
            </div>
          </CardContent>
        </Card>
      </TabsContent>
    </Tabs>
  </div>
</template>
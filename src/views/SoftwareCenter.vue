<script setup>
import { ref, computed, onMounted } from "vue";
import { useRouter } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { showToast } from "../lib/toast.js";
import { showConfirm } from "../lib/confirm.js";
import { t, tFormat } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";
import BrandIcons from "../icons/BrandIcons.vue";
import AppIcon from "../components/AppIcon.vue";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Checkbox } from "@/components/ui/checkbox";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
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
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbLink,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";

const router = useRouter();

const selectedCategory = ref("all");
const filterInstalled = ref(false);
const filterUpdates = ref(false);
const software = ref([]);
const loading = ref(true);
const error = ref(null);
const installing = ref(false);
const currentItem = ref(null);
const packageManagers = ref([]);
const pmChecking = ref(true);
const copiedCommand = ref("");

const categories = computed(() => [
  { id: "all", label: t("software.all") },
  { id: "ide", label: t("software.ide") },
  { id: "database", label: t("software.database") },
  { id: "cli", label: t("software.cli") },
  { id: "runtime", label: t("software.runtime") },
  { id: "cli-code", label: t("software.cli_code") },
]);

const cliCodeTools = [
  { name: "Claude Code", publisher: "Anthropic", command: "npm install -g @anthropic-ai/claude-code@latest", desc: "AI coding assistant by Anthropic" },
  { name: "Gemini Code", publisher: "Google", command: "npm install -g gemini-code@latest", desc: "AI coding assistant by Google" },
  { name: "OpenCode", publisher: "OpenCode", command: "npm install -g opencode-ai@latest", desc: "Open-source AI coding assistant" },
  { name: "Qoder Code", publisher: "Qoder", command: "npm install -g @qoder-ai/qodercli@latest", desc: "AI-powered coding assistant" },
  { name: "Reasonix", publisher: "Reasonix", command: "npm install -g reasonix@latest", desc: "DeepSeek-native coding agent" },
];

function brandIconName(sw) {
  const map = {
    "Visual Studio Code": "vscode",
    Neovim: "neovim",
    Vim: "vim",
    "Node.js": "nodejs",
    "Python 3": "python",
    Go: "go",
    Rust: "rust",
    Git: "git",
    "Docker Desktop": "docker",
    "Docker Engine": "docker",
    Redis: "redis",
    SQLite: "sqlite",
    "DBeaver Community": "dbeaver",
    "PostgreSQL Client": "postgresql",
    "Java (JDK)": "java",
    Ruby: "ruby",
    "IntelliJ IDEA Community": "intellij",
    "Sublime Text": "sublime",
    Zed: "zed",
    Postman: "postman",
    "MySQL Workbench": "mysql",
    TablePlus: "tableplus",
    GParted: "gparted",
    Homebrew: "homebrew",
  };
  return map[sw] || "default";
}

async function copyCommand(command, name) {
  try {
    await navigator.clipboard.writeText(command);
    copiedCommand.value = name;
    showToast(command, "success");
    setTimeout(() => {
      copiedCommand.value = "";
    }, 2000);
  } catch {
    showToast(t("common.copy_failed"), "error");
  }
}

async function loadSoftware() {
  try {
    loading.value = true;
    error.value = null;
    software.value = await invoke("list_software");
  } catch (err) {
    error.value = friendlyError(err);
    console.error("Error loading software:", err);
  } finally {
    loading.value = false;
  }
}

async function checkPackageManagers() {
  try {
    pmChecking.value = true;
    packageManagers.value = await invoke("list_package_managers");
  } catch (err) {
    console.error("Error checking package managers:", err);
    packageManagers.value = [];
  } finally {
    pmChecking.value = false;
  }
}

onMounted(() => {
  loadSoftware();
  checkPackageManagers();
});

async function handleAction(item) {
  if (!item.package_name) {
    showToast(t("common.no_package_name"));
    return;
  }

  if (item.action === "Install") {
    if (!(await showConfirm(tFormat("software.install_confirm", { name: item.name }) || `Install ${item.name}?`))) return;

    installing.value = true;
    currentItem.value = item;

    try {
      const result = await invoke("install_software", { packageName: item.package_name });
      showToast(result);
      await loadSoftware();
    } catch (err) {
      showToast(t("common.install_failed").replace("{error}", friendlyError(err)));
    } finally {
      installing.value = false;
      currentItem.value = null;
    }
  } else if (item.action === "Uninstall") {
    if (!(await showConfirm(tFormat("software.uninstall_confirm", { name: item.name }) || `Uninstall ${item.name}?`))) return;

    const removeData = await showConfirm(
      tFormat("software.uninstall_data_confirm", { name: item.name }) ||
        `Also remove config and data files for ${item.name}?`
    );

    installing.value = true;
    currentItem.value = item;

    try {
      let result;
      if (removeData) {
        result = await invoke("uninstall_software_deep", {
          packageName: item.package_name,
          appName: item.name,
        });
      } else {
        result = await invoke("uninstall_software", { packageName: item.package_name });
      }
      showToast(result);
      await loadSoftware();
    } catch (err) {
      showToast(t("common.uninstall_failed").replace("{error}", friendlyError(err)));
    } finally {
      installing.value = false;
      currentItem.value = null;
    }
  } else if (item.action === "Open") {
    showToast(t("common.opening").replace("{name}", item.name));
  }
}

const hasPackageManager = computed(() => packageManagers.value.length > 0);

const filteredSoftware = computed(() =>
  software.value.filter((s) => {
    if (selectedCategory.value !== "all" && s.category !== selectedCategory.value) return false;
    if (filterInstalled.value && s.status !== "installed") return false;
    if (filterUpdates.value && s.status !== "updates") return false;
    return true;
  })
);

function statusLabel(item) {
  if (item.status === "installed") return t("software.installed");
  if (item.status === "available") return t("software.available");
  return t("software.system");
}

// 虚拟截断：首屏仅渲染 40 项，降低 DOM 与图标开销
const visibleLimit = ref(40);
const visibleSoftware = computed(() => filteredSoftware.value.slice(0, visibleLimit.value));
function loadMore() { visibleLimit.value += 40; }

</script>

<template>
  <div class="page software-page">
    <!-- Header -->
    <div class="page-header">
      <div>
        <Breadcrumb class="mb-1">
          <BreadcrumbList>
            <BreadcrumbItem>
              <BreadcrumbLink as="button" @click="router.push('/dashboard')">
                {{ t("nav.dashboard") }}
              </BreadcrumbLink>
            </BreadcrumbItem>
            <BreadcrumbSeparator />
            <BreadcrumbItem>
              <BreadcrumbPage>{{ t("software.title") }}</BreadcrumbPage>
            </BreadcrumbItem>
          </BreadcrumbList>
        </Breadcrumb>
        <h1 class="page-title mt-2">{{ t("software.title") }}</h1>
      </div>
      <Button variant="outline" size="sm" @click="loadSoftware">
        <AppIcon name="refresh" class="size-4" />
        {{ t("common.refresh") }}
      </Button>
    </div>

    <!-- Category pills + filters bar -->
    <div class="filter-bar section-card">
      <div class="flex flex-wrap items-center gap-2">
        <Button
          v-for="cat in categories"
          :key="cat.id"
          size="sm"
          :variant="selectedCategory === cat.id ? 'default' : 'outline'"
          @click="selectedCategory = cat.id"
        >
          {{ cat.label }}
        </Button>
      </div>
      <div class="flex items-center gap-6">
        <label class="flex cursor-pointer items-center gap-2 text-sm text-foreground">
          <Checkbox v-model="filterInstalled" />
          {{ t("software.installed_filter") }}
        </label>
        <label class="flex cursor-pointer items-center gap-2 text-sm text-foreground">
          <Checkbox v-model="filterUpdates" />
          {{ t("software.updates_filter") }}
        </label>
      </div>
    </div>

    <!-- Content area -->
    <div class="content-area">
      <!-- CLI Code tools -->
      <div v-if="selectedCategory === 'cli-code'" class="tool-grid">
        <Card v-for="tool in cliCodeTools" :key="tool.name" class="tool-card shadow-sm">
          <CardContent class="flex flex-1 flex-col p-4">
            <div class="tool-icon">
              <AppIcon name="code-block" class="size-[18px]" />
            </div>
            <div class="tool-title">{{ tool.name }}</div>
            <div class="tool-publisher">{{ tool.publisher }}</div>
            <div class="tool-desc">{{ tool.desc }}</div>
            <div class="tool-command">
              <span class="cmd-text" :title="tool.command">{{ tool.command }}</span>
              <Button
                size="sm"
                class="shrink-0"
                :variant="copiedCommand === tool.name ? 'default' : 'ghost'"
                @click="copyCommand(tool.command, tool.name)"
              >
                <AppIcon v-if="copiedCommand === tool.name" name="check" class="size-3.5" />
                <AppIcon v-else name="copy" class="size-3.5" />
              </Button>
            </div>
          </CardContent>
        </Card>
      </div>

      <!-- Regular software -->
      <div v-else>
        <!-- No package manager -->
        <Card v-if="!hasPackageManager && !pmChecking && !error" class="no-pm-card shadow-sm">
          <CardContent class="py-5">
            <div class="no-pm-content">
              <AppIcon name="archive" class="no-pm-icon size-10" />
              <h2 class="no-pm-title">{{ t("software.no_pm_title") }}</h2>
              <p class="no-pm-desc">{{ t("software.no_pm_desc") }}</p>
              <div v-if="packageManagers.length > 0" class="no-pm-pills">
                <Badge v-for="pm in packageManagers" :key="pm.name" variant="secondary">
                  {{ pm.name }}
                </Badge>
              </div>
              <div v-else class="no-pm-suggest">
                <p class="suggest-title">{{ t("software.no_pm_suggest") }}</p>
                <ul class="suggest-list">
                  <li><strong>macOS:</strong> <a href="https://brew.sh" target="_blank" class="link">Homebrew</a></li>
                  <li><strong>Linux:</strong> apt, dnf, pacman, zypper, apk</li>
                  <li><strong>Windows:</strong> winget (Win 11/10 1809+)，<a href="https://chocolatey.org/install" target="_blank" class="link">Chocolatey</a></li>
                </ul>
              </div>
            </div>
          </CardContent>
        </Card>

        <!-- Loading -->
        <div
          v-else-if="loading"
          class="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4"
        >
          <Skeleton v-for="i in 8" :key="i" class="h-44 w-full rounded-xl" />
        </div>

        <!-- Error -->
        <Card v-else-if="error" class="shadow-sm">
          <CardContent class="py-4">
            <Alert variant="destructive">
              <AppIcon name="close-circle-fill" class="size-4" />
              <AlertTitle>{{ t("error.title") }}</AlertTitle>
              <AlertDescription>{{ error }}</AlertDescription>
            </Alert>
            <Button class="mt-3" @click="loadSoftware">
              {{ t("common.retry") }}
            </Button>
          </CardContent>
        </Card>

        <!-- Empty -->
        <Card v-else-if="filteredSoftware.length === 0" class="shadow-sm">
          <CardContent class="py-4">
            <Empty class="py-5">
              <EmptyMedia>
                <AppIcon name="apps" class="size-10 text-muted-foreground/60" />
              </EmptyMedia>
              <EmptyContent>
                <EmptyDescription>{{ t("software.none") }}</EmptyDescription>
              </EmptyContent>
            </Empty>
          </CardContent>
        </Card>

        <!-- Grid -->
        <div v-else class="soft-grid">
          <Card v-for="item in visibleSoftware" :key="item.name" class="soft-card shadow-sm">
            <CardContent class="flex flex-1 flex-col p-4">
              <div class="soft-head">
                <div class="soft-icon">
                  <BrandIcons :name="brandIconName(item.name)" :size="20" />
                </div>
                <Badge
                  variant="secondary"
                  :class="item.status === 'installed' ? 'bg-success/10 text-success' : ''"
                >
                  {{ statusLabel(item) }}
                </Badge>
              </div>
              <div class="soft-name">{{ item.name }}</div>
              <div class="soft-version">{{ item.version }}</div>
              <Button
                class="soft-action w-full"
                :variant="item.action === 'Install' ? 'default' : item.action === 'Uninstall' ? 'destructive' : 'outline'"
                :disabled="item.action === 'System Managed' || installing"
                @click="handleAction(item)"
              >
                <Spinner v-if="installing && currentItem?.name === item.name" class="size-4" />
                {{ installing && currentItem?.name === item.name ? t("software.processing") : item.action }}
              </Button>
            </CardContent>
          </Card>
        </div>
        <div v-if="visibleSoftware.length < filteredSoftware.length" class="flex justify-center mt-4">
          <Button variant="outline" @click="loadMore">{{ t("common.load_more") || "Load more" }} ({{ visibleSoftware.length }}/{{ filteredSoftware.length }})</Button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.software-page {
  display: flex;
  flex-direction: column;
}
.filter-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  flex-wrap: wrap;
}
.content-area {
  flex: 1;
  min-height: 0;
}
.tool-grid,
.soft-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: 0.75rem;
}
.tool-card,
.soft-card {
  display: flex;
  flex-direction: column;
}
.tool-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border-radius: 8px;
  background-color: var(--color-accent);
  color: var(--color-muted-foreground);
  font-size: 18px;
  margin-bottom: 12px;
}
.tool-title {
  font-size: 14px;
  font-weight: 500;
  color: var(--color-foreground);
}
.tool-publisher {
  font-size: 12px;
  color: var(--color-muted-foreground);
  margin: 2px 0 6px;
}
.tool-desc {
  font-size: 12px;
  color: var(--color-muted-foreground);
  line-height: 1.6;
  flex: 1;
  margin-bottom: 12px;
}
.tool-command {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  background-color: var(--color-muted);
}
.cmd-text {
  flex: 1;
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
  color: var(--color-muted-foreground);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.no-pm-card {
  text-align: center;
  padding: 20px;
}
.no-pm-icon {
  color: var(--color-muted-foreground);
}
.no-pm-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--color-foreground);
  margin: 14px 0 6px;
}
.no-pm-desc {
  font-size: 13px;
  color: var(--color-muted-foreground);
  line-height: 1.7;
  max-width: 480px;
  margin: 0 auto;
}
.no-pm-pills {
  margin-top: 18px;
  display: flex;
  justify-content: center;
  gap: 8px;
  flex-wrap: wrap;
}
.no-pm-suggest {
  margin-top: 18px;
  display: inline-block;
  text-align: left;
  font-size: 13px;
  color: var(--color-muted-foreground);
}
.suggest-title {
  font-weight: 500;
  margin-bottom: 6px;
}
.suggest-list {
  padding-left: 18px;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.link {
  color: var(--color-primary);
  text-decoration: underline;
}
.soft-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  margin-bottom: 12px;
}
.soft-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border-radius: 8px;
  background-color: var(--color-accent);
}
.soft-name {
  font-size: 14px;
  font-weight: 500;
  color: var(--color-foreground);
  margin-bottom: 4px;
}
.soft-version {
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
  color: var(--color-muted-foreground);
  margin-bottom: 16px;
}
.soft-action {
  margin-top: auto;
}
</style>
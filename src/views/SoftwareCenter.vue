<script setup>
import { ref, computed, onMounted } from "vue";
import { useRouter } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { showToast } from "../lib/toast.js";
import { showConfirm } from "../lib/confirm.js";
import { t, tFormat } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";
import BrandIcons from "../icons/BrandIcons.vue";

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
</script>

<template>
  <div class="software-page">
    <!-- Header with back button -->
    <div class="breadcrumb">
      <a-button type="text" size="small" @click="router.push('/dashboard')">
        <template #icon><icon-left /></template>
        {{ t("nav.dashboard") }}
      </a-button>
      <span class="crumb-sep">/</span>
      <span class="crumb-title">{{ t("software.title") }}</span>
    </div>

    <!-- Category pills + filters bar -->
    <div class="filter-bar">
      <div class="cat-pills">
        <a-button
          v-for="cat in categories"
          :key="cat.id"
          size="small"
          :type="selectedCategory === cat.id ? 'primary' : 'text'"
          @click="selectedCategory = cat.id"
        >
          {{ cat.label }}
        </a-button>
      </div>
      <div class="filter-actions">
        <a-checkbox v-model="filterInstalled">{{ t("software.installed_filter") }}</a-checkbox>
        <a-checkbox v-model="filterUpdates">{{ t("software.updates_filter") }}</a-checkbox>
        <a-button size="small" @click="loadSoftware">
          <template #icon><icon-refresh /></template>
        </a-button>
      </div>
    </div>

    <!-- Content area -->
    <div class="content-area">
      <!-- CLI Code tools -->
      <div v-if="selectedCategory === 'cli-code'" class="tool-grid">
        <a-card v-for="tool in cliCodeTools" :key="tool.name" :bordered="true" class="tool-card">
          <div class="tool-icon">
            <icon-terminal />
          </div>
          <div class="tool-title">{{ tool.name }}</div>
          <div class="tool-publisher">{{ tool.publisher }}</div>
          <div class="tool-desc">{{ tool.desc }}</div>
          <div class="tool-command">
            <span class="cmd-text" :title="tool.command">{{ tool.command }}</span>
            <a-button
              size="mini"
              :type="copiedCommand === tool.name ? 'success' : 'text'"
              @click="copyCommand(tool.command, tool.name)"
            >
              <template #icon>
                <icon-check v-if="copiedCommand === tool.name" />
                <icon-copy v-else />
              </template>
            </a-button>
          </div>
        </a-card>
      </div>

      <!-- Regular software -->
      <div v-else>
        <a-spin :loading="loading" style="width: 100%">
          <!-- No package manager -->
          <a-card v-if="!hasPackageManager && !pmChecking && !error" :bordered="true" class="no-pm-card">
            <div class="no-pm-content">
              <icon-package class="no-pm-icon" />
              <h2 class="no-pm-title">{{ t("software.no_pm_title") }}</h2>
              <p class="no-pm-desc">{{ t("software.no_pm_desc") }}</p>
              <div v-if="packageManagers.length > 0" class="no-pm-pills">
                <a-tag v-for="pm in packageManagers" :key="pm.name">{{ pm.name }}</a-tag>
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
          </a-card>

          <!-- Error -->
          <a-result v-else-if="error" status="error" :title="error" style="padding: 56px 0">
            <template #extra>
              <a-button type="primary" @click="loadSoftware">{{ t("common.retry") }}</a-button>
            </template>
          </a-result>

          <!-- Empty -->
          <a-empty
            v-else-if="filteredSoftware.length === 0"
            :description="t('software.none')"
            style="padding: 56px 0"
          />

          <!-- Grid -->
          <div v-else class="soft-grid">
            <a-card v-for="item in filteredSoftware" :key="item.name" :bordered="true" class="soft-card">
              <div class="soft-head">
                <div class="soft-icon">
                  <BrandIcons :name="brandIconName(item.name)" :size="20" />
                </div>
                <a-tag :color="item.status === 'installed' ? 'green' : ''" size="small">
                  {{ statusLabel(item) }}
                </a-tag>
              </div>
              <div class="soft-name">{{ item.name }}</div>
              <div class="soft-version">{{ item.version }}</div>
              <a-button
                class="soft-action"
                :type="item.action === 'Install' ? 'primary' : item.action === 'Uninstall' ? 'danger' : 'secondary'"
                :disabled="item.action === 'System Managed' || installing"
                :loading="installing && currentItem?.name === item.name"
                long
                @click="handleAction(item)"
              >
                {{ installing && currentItem?.name === item.name ? t("software.processing") : item.action }}
              </a-button>
            </a-card>
          </div>
        </a-spin>
      </div>
    </div>
  </div>
</template>

<style scoped>
.software-page {
  display: flex;
  flex-direction: column;
  height: 100%;
}
.breadcrumb {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 20px;
  border-bottom: 1px solid var(--color-border);
  flex-shrink: 0;
}
.crumb-sep {
  font-size: 12px;
  color: var(--color-text-3);
}
.crumb-title {
  font-size: 14px;
  font-weight: 500;
  color: var(--color-text-1);
}
.filter-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 20px;
  border-bottom: 1px solid var(--color-border);
  flex-shrink: 0;
  flex-wrap: wrap;
}
.cat-pills {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-wrap: wrap;
}
.filter-actions {
  display: flex;
  align-items: center;
  gap: 16px;
  flex-shrink: 0;
}
.content-area {
  flex: 1;
  overflow-y: auto;
  padding: 20px;
}
.tool-grid,
.soft-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 12px;
}
.tool-card,
.soft-card {
  border-radius: 10px;
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
  background-color: var(--color-fill-2);
  color: var(--color-text-2);
  font-size: 18px;
  margin-bottom: 12px;
}
.tool-title {
  font-size: 14px;
  font-weight: 500;
  color: var(--color-text-1);
}
.tool-publisher {
  font-size: 12px;
  color: var(--color-text-3);
  margin: 2px 0 6px;
}
.tool-desc {
  font-size: 12px;
  color: var(--color-text-2);
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
  background-color: var(--color-fill-1);
}
.cmd-text {
  flex: 1;
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
  color: var(--color-text-2);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.no-pm-card {
  border-radius: 10px;
  text-align: center;
  padding: 20px;
}
.no-pm-icon {
  font-size: 40px;
  color: var(--color-text-4);
}
.no-pm-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--color-text-1);
  margin: 14px 0 6px;
}
.no-pm-desc {
  font-size: 13px;
  color: var(--color-text-2);
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
  color: var(--color-text-2);
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
  color: var(--color-primary-6);
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
  background-color: var(--color-fill-2);
}
.soft-name {
  font-size: 14px;
  font-weight: 500;
  color: var(--color-text-1);
  margin-bottom: 4px;
}
.soft-version {
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
  color: var(--color-text-3);
  margin-bottom: 16px;
}
.soft-action {
  margin-top: auto;
}
</style>

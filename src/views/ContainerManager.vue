<script setup>
import { ref, computed, onMounted } from "vue";
import { useRouter } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { showToast } from "../lib/toast.js";
import { showConfirm } from "../lib/confirm.js";
import { t } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";
import AppIcon from "../components/AppIcon.vue";
import ContainerIcons from "../icons/ContainerIcons.vue";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@/components/ui/tabs";
import ContainerDialog from "../components/containers/ContainerDialog.vue";
import ContainersTab from "../components/containers/ContainersTab.vue";
import ImagesTab from "../components/containers/ImagesTab.vue";
import VolumesTab from "../components/containers/VolumesTab.vue";
import NetworksTab from "../components/containers/NetworksTab.vue";
import ComposeTab from "../components/containers/ComposeTab.vue";

const router = useRouter();

const activeTab = ref("containers");
const dockerStatus = ref({ installed: false, version: "", running: false });
const checking = ref(true);
const containers = ref([]);
const containersLoading = ref(false);
const containerError = ref(null);
const showAll = ref(false);
const images = ref([]);
const imagesLoading = ref(false);
const imageError = ref(null);
const volumes = ref([]);
const volumesLoading = ref(false);
const volumeError = ref(null);
const networks = ref([]);
const networksLoading = ref(false);
const networkError = ref(null);
const composeFile = ref("");
const composeProject = ref("");
const composeContainers = ref([]);
const composeLoading = ref(false);
const composeLogs = ref("");
const composeError = ref(null);
const showLogs = ref(false);
const logContainer = ref("");
const logContent = ref("");
const logLoading = ref(false);
const showTerminal = ref(false);
const termContainer = ref("");
const termCommand = ref("");
const termOutput = ref("");
const termLoading = ref(false);
const showPull = ref(false);
const pullImageName = ref("");
const pullLoading = ref(false);
const showBuild = ref(false);
const buildTag = ref("");
const buildPath = ref("");
const buildLoading = ref(false);
const showPush = ref(false);
const pushTarget = ref("");
const pushLoading = ref(false);
const showTag = ref(false);
const tagValue = ref("");
const tagImageId = ref("");
const tagLoading = ref(false);
const showCreateVolume = ref(false);
const newVolumeName = ref("");
const showCreateNetwork = ref(false);
const newNetworkName = ref("");
const actionLoading = ref("");
const search = ref("");

const tabItems = computed(() => [
  { key: "containers", title: t("docker.tab_containers") },
  { key: "images", title: t("docker.tab_images") },
  { key: "volumes", title: t("docker.tab_volumes") },
  { key: "networks", title: t("docker.tab_networks") },
  { key: "compose", title: t("docker.tab_compose") },
]);

onMounted(() => {
  checkDocker();
});

async function checkDocker() {
  checking.value = true;
  try {
    dockerStatus.value = await invoke("check_docker");
    if (dockerStatus.value.installed && dockerStatus.value.running) {
      loadTabData("containers");
    }
  } catch {
    dockerStatus.value = { installed: false, version: "", running: false };
  } finally {
    checking.value = false;
  }
}

async function loadTabData(tab) {
  switch (tab) {
    case "containers":
      return loadContainers();
    case "images":
      return loadImages();
    case "volumes":
      return loadVolumes();
    case "networks":
      return loadNetworks();
  }
}

function onTabChange(tab) {
  activeTab.value = tab;
  search.value = "";
  if (dockerStatus.value.installed && dockerStatus.value.running) {
    loadTabData(tab);
  }
}

async function loadContainers() {
  containersLoading.value = true;
  containerError.value = null;
  try {
    containers.value = await invoke("list_containers", { all: showAll.value });
  } catch (err) {
    containerError.value = friendlyError(err);
  } finally {
    containersLoading.value = false;
  }
}

async function containerAction(name, action) {
  if (action === "rm" || action === "stop") {
    const label = action === "rm" ? t("docker.delete_confirm") : t("docker.stop_confirm");
    if (!(await showConfirm(label.replace("{name}", name)))) return;
  }
  actionLoading.value = name;
  try {
    const result = await invoke("container_action", { name, action });
    showToast(result || t("docker.action_done"), "success");
    await loadContainers();
  } catch (err) {
    showToast(`${t("docker.action_failed")}: ${friendlyError(err)}`, "error");
  } finally {
    actionLoading.value = "";
  }
}

async function openLogs(name) {
  logContainer.value = name;
  logContent.value = "";
  showLogs.value = true;
  logLoading.value = true;
  try {
    logContent.value = await invoke("get_container_logs", { name, tail: 200 });
  } catch (err) {
    logContent.value = `Error: ${friendlyError(err)}`;
  } finally {
    logLoading.value = false;
  }
}

async function runTerminalCommand() {
  if (!termCommand.value.trim()) return;
  termLoading.value = true;
  try {
    termOutput.value = await invoke("exec_in_container", {
      name: termContainer.value,
      command: termCommand.value,
    });
  } catch (err) {
    termOutput.value = `Error: ${friendlyError(err)}`;
  } finally {
    termLoading.value = false;
  }
}

const filteredContainers = computed(() => {
  const q = search.value.trim().toLowerCase();
  if (!q) return containers.value;
  return containers.value.filter(
    (c) =>
      c.name.toLowerCase().includes(q) ||
      c.image.toLowerCase().includes(q) ||
      c.id.toLowerCase().includes(q)
  );
});

async function loadImages() {
  imagesLoading.value = true;
  imageError.value = null;
  try {
    images.value = await invoke("list_images");
  } catch (err) {
    imageError.value = friendlyError(err);
  } finally {
    imagesLoading.value = false;
  }
}

async function pullImageAction() {
  if (!pullImageName.value.trim()) return;
  pullLoading.value = true;
  try {
    const result = await invoke("pull_image", { image: pullImageName.value.trim() });
    showToast(result, "success");
    showPull.value = false;
    pullImageName.value = "";
    await loadImages();
  } catch (err) {
    showToast(`${t("docker.pull_failed")}: ${friendlyError(err)}`, "error");
  } finally {
    pullLoading.value = false;
  }
}

async function removeImageAction(id, name) {
  if (!(await showConfirm(t("docker.image_delete_confirm").replace("{name}", name)))) return;
  actionLoading.value = id;
  try {
    const result = await invoke("remove_image", { imageId: id, force: false });
    showToast(result || t("docker.action_done"), "success");
    await loadImages();
  } catch (err) {
    showToast(`${t("docker.action_failed")}: ${friendlyError(err)}`, "error");
  } finally {
    actionLoading.value = "";
  }
}

async function buildImageAction() {
  if (!buildTag.value.trim() || !buildPath.value.trim()) return;
  buildLoading.value = true;
  try {
    const result = await invoke("build_image", {
      tag: buildTag.value.trim(),
      path: buildPath.value.trim(),
    });
    showToast(result || t("docker.build_done"), "success");
    showBuild.value = false;
    buildTag.value = "";
    buildPath.value = "";
    await loadImages();
  } catch (err) {
    showToast(`${t("docker.build_failed")}: ${friendlyError(err)}`, "error");
  } finally {
    buildLoading.value = false;
  }
}

function openPush(img) {
  pushTarget.value = `${img.repository}:${img.tag}`;
  showPush.value = true;
}

async function pushImageAction() {
  if (!pushTarget.value.trim()) return;
  pushLoading.value = true;
  try {
    const result = await invoke("push_image", { tag: pushTarget.value.trim() });
    showToast(result || t("docker.push_done"), "success");
    showPush.value = false;
    pushTarget.value = "";
  } catch (err) {
    showToast(`${t("docker.push_failed")}: ${friendlyError(err)}`, "error");
  } finally {
    pushLoading.value = false;
  }
}

function openTag(img) {
  tagImageId.value = img.id;
  tagValue.value = "";
  showTag.value = true;
}

async function tagImageAction() {
  if (!tagValue.value.trim()) return;
  tagLoading.value = true;
  try {
    const result = await invoke("tag_image", {
      imageId: tagImageId.value,
      tag: tagValue.value.trim(),
    });
    showToast(result || t("docker.tag_done"), "success");
    showTag.value = false;
    tagValue.value = "";
    tagImageId.value = "";
    await loadImages();
  } catch (err) {
    showToast(`${t("docker.tag_failed")}: ${friendlyError(err)}`, "error");
  } finally {
    tagLoading.value = false;
  }
}

const filteredImages = computed(() => {
  const q = search.value.trim().toLowerCase();
  if (!q) return images.value;
  return images.value.filter(
    (i) =>
      i.repository.toLowerCase().includes(q) ||
      i.tag.toLowerCase().includes(q) ||
      i.id.toLowerCase().includes(q)
  );
});

async function loadVolumes() {
  volumesLoading.value = true;
  volumeError.value = null;
  try {
    volumes.value = await invoke("list_volumes");
  } catch (err) {
    volumeError.value = friendlyError(err);
  } finally {
    volumesLoading.value = false;
  }
}

async function createVolume() {
  if (!newVolumeName.value.trim()) return;
  try {
    await invoke("volume_action", { name: newVolumeName.value.trim(), action: "create" });
    showToast(t("docker.volume_created"), "success");
    showCreateVolume.value = false;
    newVolumeName.value = "";
    await loadVolumes();
  } catch (err) {
    showToast(`${t("docker.action_failed")}: ${friendlyError(err)}`, "error");
  }
}

async function removeVolume(name) {
  if (!(await showConfirm(t("docker.volume_delete_confirm").replace("{name}", name)))) return;
  actionLoading.value = name;
  try {
    await invoke("volume_action", { name, action: "rm" });
    showToast(t("docker.volume_deleted"), "success");
    await loadVolumes();
  } catch (err) {
    showToast(`${t("docker.action_failed")}: ${friendlyError(err)}`, "error");
  } finally {
    actionLoading.value = "";
  }
}

async function loadNetworks() {
  networksLoading.value = true;
  networkError.value = null;
  try {
    networks.value = await invoke("list_networks");
  } catch (err) {
    networkError.value = friendlyError(err);
  } finally {
    networksLoading.value = false;
  }
}

async function createNetwork() {
  if (!newNetworkName.value.trim()) return;
  try {
    await invoke("network_action", { name: newNetworkName.value.trim(), action: "create" });
    showToast(t("docker.network_created"), "success");
    showCreateNetwork.value = false;
    newNetworkName.value = "";
    await loadNetworks();
  } catch (err) {
    showToast(`${t("docker.action_failed")}: ${friendlyError(err)}`, "error");
  }
}

async function removeNetwork(name) {
  if (!(await showConfirm(t("docker.network_delete_confirm").replace("{name}", name)))) return;
  actionLoading.value = name;
  try {
    await invoke("network_action", { name, action: "rm" });
    showToast(t("docker.network_deleted"), "success");
    await loadNetworks();
  } catch (err) {
    showToast(`${t("docker.action_failed")}: ${friendlyError(err)}`, "error");
  } finally {
    actionLoading.value = "";
  }
}

async function composeUp() {
  composeLoading.value = true;
  composeError.value = null;
  composeLogs.value = "";
  try {
    const result = await invoke("compose_up", {
      file: composeFile.value.trim() || null,
      projectName: composeProject.value.trim() || null,
    });
    showToast(result || t("docker.compose_up_done"), "success");
  } catch (err) {
    composeError.value = friendlyError(err);
  } finally {
    composeLoading.value = false;
  }
}

async function composeDown() {
  if (!(await showConfirm(t("docker.compose_down_confirm")))) return;
  composeLoading.value = true;
  composeError.value = null;
  try {
    const result = await invoke("compose_down", {
      file: composeFile.value.trim() || null,
      projectName: composeProject.value.trim() || null,
    });
    showToast(result || t("docker.compose_down_done"), "success");
  } catch (err) {
    composeError.value = friendlyError(err);
  } finally {
    composeLoading.value = false;
  }
}

async function composePs() {
  composeLoading.value = true;
  composeError.value = null;
  try {
    composeContainers.value = await invoke("compose_ps", {
      file: composeFile.value.trim() || null,
      projectName: composeProject.value.trim() || null,
    });
  } catch (err) {
    composeError.value = friendlyError(err);
  } finally {
    composeLoading.value = false;
  }
}

async function composeViewLogs() {
  composeLoading.value = true;
  composeError.value = null;
  try {
    composeLogs.value = await invoke("compose_logs", {
      file: composeFile.value.trim() || null,
      projectName: composeProject.value.trim() || null,
      tail: 100,
    });
  } catch (err) {
    composeError.value = friendlyError(err);
  } finally {
    composeLoading.value = false;
  }
}

// ── Dialog configs ──
const pullConfig = computed(() => ({
  title: t("docker.pull_image"),
  icon: "download",
  width: 320,
  fields: [
    {
      id: "pull-image",
      placeholder: "nginx:latest",
      value: pullImageName.value,
      onInput: (v) => (pullImageName.value = v),
    },
  ],
  loading: pullLoading.value,
  submitLabel: t("docker.pull"),
  loadingLabel: t("docker.pulling"),
  canSubmit: pullImageName.value.trim() !== "",
  onSubmit: pullImageAction,
  onClose: () => {
    showPull.value = false;
    pullImageName.value = "";
  },
}));

const buildConfig = computed(() => ({
  title: t("docker.build_image"),
  icon: "construction",
  width: 340,
  fields: [
    {
      id: "build-tag",
      placeholder: `${t("docker.build_tag")} (myapp:latest)`,
      value: buildTag.value,
      onInput: (v) => (buildTag.value = v),
      enterSubmit: false,
    },
    {
      id: "build-path",
      placeholder: `${t("docker.build_path")} (.)`,
      value: buildPath.value,
      onInput: (v) => (buildPath.value = v),
      enterSubmit: false,
    },
  ],
  loading: buildLoading.value,
  submitLabel: t("docker.build"),
  loadingLabel: t("docker.building"),
  canSubmit: buildTag.value.trim() !== "" && buildPath.value.trim() !== "",
  onSubmit: buildImageAction,
  onClose: () => {
    showBuild.value = false;
    buildTag.value = "";
    buildPath.value = "";
  },
}));

const pushConfig = computed(() => ({
  title: t("docker.push_image"),
  icon: "upload",
  width: 320,
  fields: [
    {
      id: "push-target",
      placeholder: "registry/user/repo:tag",
      value: pushTarget.value,
      onInput: (v) => (pushTarget.value = v),
    },
  ],
  loading: pushLoading.value,
  submitLabel: t("docker.push"),
  loadingLabel: t("docker.pushing"),
  canSubmit: pushTarget.value.trim() !== "",
  onSubmit: pushImageAction,
  onClose: () => {
    showPush.value = false;
    pushTarget.value = "";
  },
}));

const tagConfig = computed(() => ({
  title: t("docker.tag_image"),
  icon: "sell",
  width: 320,
  fields: [
    {
      id: "tag-value",
      placeholder: "registry/user/repo:tag",
      value: tagValue.value,
      onInput: (v) => (tagValue.value = v),
    },
  ],
  loading: tagLoading.value,
  submitLabel: t("docker.tag"),
  loadingLabel: t("docker.tagging"),
  canSubmit: tagValue.value.trim() !== "",
  onSubmit: tagImageAction,
  onClose: () => {
    showTag.value = false;
    tagValue.value = "";
  },
}));

const createVolumeConfig = computed(() => ({
  title: t("docker.create_volume"),
  icon: "add",
  width: 320,
  fields: [
    {
      id: "volume-name",
      placeholder: "my_volume",
      value: newVolumeName.value,
      onInput: (v) => (newVolumeName.value = v),
    },
  ],
  loading: false,
  submitLabel: t("docker.create"),
  loadingLabel: t("docker.create"),
  canSubmit: newVolumeName.value.trim() !== "",
  onSubmit: createVolume,
  onClose: () => {
    showCreateVolume.value = false;
    newVolumeName.value = "";
  },
}));

const createNetworkConfig = computed(() => ({
  title: t("docker.create_network"),
  icon: "add",
  width: 320,
  fields: [
    {
      id: "network-name",
      placeholder: "my_network",
      value: newNetworkName.value,
      onInput: (v) => (newNetworkName.value = v),
    },
  ],
  loading: false,
  submitLabel: t("docker.create"),
  loadingLabel: t("docker.create"),
  canSubmit: newNetworkName.value.trim() !== "",
  onSubmit: createNetwork,
  onClose: () => {
    showCreateNetwork.value = false;
    newNetworkName.value = "";
  },
}));

const showSearch = computed(() => ["containers", "images"].includes(activeTab.value));
</script>

<template>
  <div class="page container-page">
    <!-- Header with back button -->
    <div class="page-header">
      <div class="header-left">
        <Button variant="ghost" size="sm" @click="router.push('/dashboard')">
          <AppIcon name="left" class="size-4" />
          {{ t("nav.dashboard") }}
        </Button>
        <Separator orientation="vertical" class="mx-1 h-4" />
        <ContainerIcons name="docker-logo" :size="18" class="docker-logo" />
        <h1 class="page-title-inline">{{ t("nav.containers") }}</h1>
      </div>
    </div>

    <!-- Checking -->
    <Card v-if="checking" class="status-card shadow-sm">
      <CardContent class="pt-6">
        <div class="space-y-3 py-4 text-center">
          <Skeleton class="mx-auto h-10 w-10 rounded-full" />
          <Skeleton class="mx-auto h-4 w-48" />
          <Skeleton class="mx-auto h-3 w-64" />
        </div>
      </CardContent>
    </Card>

    <!-- Not installed -->
    <Card v-else-if="!dockerStatus.installed" class="status-card shadow-sm">
      <CardContent class="pt-6">
        <div class="status-center">
          <ContainerIcons name="docker-logo" :size="48" class="status-icon" />
          <h2 class="status-title">{{ t("docker.not_installed_title") }}</h2>
          <p class="status-desc">{{ t("docker.not_installed_desc") }}</p>
          <div class="install-guide">
            <p class="guide-title">{{ t("docker.install_guide") }}</p>
            <ul class="guide-list">
              <li>
                <span class="guide-label">macOS:</span>
                <a
                  href="https://docs.docker.com/desktop/install/mac-install/"
                  target="_blank"
                  class="guide-link"
                >Docker Desktop</a>
              </li>
              <li>
                <span class="guide-label">Linux:</span>
                <code class="guide-code">curl -fsSL https://get.docker.com | sh</code>
              </li>
              <li>
                <span class="guide-label">Windows:</span>
                <a
                  href="https://docs.docker.com/desktop/install/windows-install/"
                  target="_blank"
                  class="guide-link"
                >Docker Desktop</a>
              </li>
            </ul>
          </div>
        </div>
      </CardContent>
    </Card>

    <!-- Not running -->
    <Card v-else-if="!dockerStatus.running" class="status-card shadow-sm">
      <CardContent class="pt-6">
        <div class="status-center">
          <ContainerIcons name="container-exited" :size="48" class="status-icon" />
          <h2 class="status-title">{{ t("docker.not_running_title") }}</h2>
          <p class="status-desc">{{ t("docker.not_running_desc") }}</p>
          <div class="status-version">{{ dockerStatus.version }}</div>
        </div>
      </CardContent>
    </Card>

    <template v-else>
      <!-- Search -->
      <div v-if="showSearch" class="search-row">
        <div class="relative max-w-[480px]">
          <AppIcon
            name="search"
            class="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
          />
          <Input
            v-model="search"
            :placeholder="activeTab === 'containers' ? t('docker.search_containers') : t('docker.search_images')"
            class="pl-9"
          />
        </div>
      </div>

      <!-- Tabs -->
      <Tabs v-model="activeTab" @update:model-value="onTabChange" class="docker-tabs">
        <TabsList class="mb-4">
          <TabsTrigger
            v-for="tab in tabItems"
            :key="tab.key"
            :value="tab.key"
          >
            {{ tab.title }}
          </TabsTrigger>
        </TabsList>

        <TabsContent v-for="tab in tabItems" :key="tab.key" :value="tab.key">
          <ContainersTab
            v-if="tab.key === 'containers'"
            :items="filteredContainers"
            :loading="containersLoading"
            :error="containerError"
            :search="search"
            :show-all="showAll"
            :action-loading="actionLoading"
            @show-all-change="(c) => { showAll = c; loadContainers(); }"
            @refresh="loadContainers"
            @action="containerAction"
            @logs="openLogs"
            @terminal="(name) => { termContainer = name; termCommand = ''; termOutput = ''; showTerminal = true; }"
          />
          <ImagesTab
            v-else-if="tab.key === 'images'"
            :items="filteredImages"
            :loading="imagesLoading"
            :error="imageError"
            :search="search"
            :action-loading="actionLoading"
            @pull="showPull = true"
            @build="showBuild = true"
            @refresh="loadImages"
            @push="openPush"
            @tag="openTag"
            @remove="removeImageAction"
          />
          <VolumesTab
            v-else-if="tab.key === 'volumes'"
            :items="volumes"
            :loading="volumesLoading"
            :error="volumeError"
            :action-loading="actionLoading"
            @create="showCreateVolume = true"
            @refresh="loadVolumes"
            @remove="removeVolume"
          />
          <NetworksTab
            v-else-if="tab.key === 'networks'"
            :items="networks"
            :loading="networksLoading"
            :error="networkError"
            :action-loading="actionLoading"
            @create="showCreateNetwork = true"
            @refresh="loadNetworks"
            @remove="removeNetwork"
          />
          <ComposeTab
            v-else
            :file="composeFile"
            :project="composeProject"
            :loading="composeLoading"
            :error="composeError"
            :containers="composeContainers"
            :logs="composeLogs"
            @file-input="(v) => (composeFile = v)"
            @project-input="(v) => (composeProject = v)"
            @up="composeUp"
            @down="composeDown"
            @ps="composePs"
            @logs="composeViewLogs"
            @clear-logs="composeLogs = ''"
          />
        </TabsContent>
      </Tabs>
    </template>

    <!-- Logs dialog -->
    <Dialog :open="showLogs" @update:open="(o) => (showLogs = o)">
      <DialogContent class="max-w-[760px]">
        <DialogHeader>
          <DialogTitle>{{ t("docker.logs") }}: {{ logContainer }}</DialogTitle>
        </DialogHeader>
        <Skeleton v-if="logLoading" class="h-40 w-full" />
        <pre v-else class="log-pre">{{ logContent || t("docker.no_logs") }}</pre>
      </DialogContent>
    </Dialog>

    <!-- Terminal dialog -->
    <Dialog :open="showTerminal" @update:open="(o) => (showTerminal = o)">
      <DialogContent class="max-w-[640px]">
        <DialogHeader>
          <DialogTitle>{{ t("docker.terminal") }}: {{ termContainer }}</DialogTitle>
        </DialogHeader>
        <div class="terminal-row">
          <Input
            v-model="termCommand"
            :placeholder="t('docker.terminal_ph')"
            class="terminal-input"
            @keydown.enter="runTerminalCommand"
          />
          <Button :disabled="termLoading" @click="runTerminalCommand">
            <Spinner v-if="termLoading" class="size-4" />
            {{ t("docker.run") }}
          </Button>
        </div>
        <pre v-if="termOutput" class="term-output">{{ termOutput }}</pre>
      </DialogContent>
    </Dialog>

    <ContainerDialog v-if="showPull" :config="pullConfig" />
    <ContainerDialog v-if="showBuild" :config="buildConfig" />
    <ContainerDialog v-if="showPush" :config="pushConfig" />
    <ContainerDialog v-if="showTag" :config="tagConfig" />
    <ContainerDialog v-if="showCreateVolume" :config="createVolumeConfig" />
    <ContainerDialog v-if="showCreateNetwork" :config="createNetworkConfig" />
  </div>
</template>

<style scoped>
.container-page {
  display: flex;
  flex-direction: column;
}
.header-left {
  display: flex;
  align-items: center;
  gap: 6px;
}
.page-title-inline {
  font-size: 16px;
  font-weight: 600;
  color: var(--color-foreground);
  margin: 0;
}
.docker-logo {
  color: var(--color-muted-foreground);
}
.status-card {
  border-radius: 10px;
}
.status-center {
  text-align: center;
  padding: 24px 0;
}
.status-icon {
  color: var(--color-muted-foreground);
}
.status-title {
  margin-top: 12px;
  font-size: 16px;
  font-weight: 600;
  color: var(--color-foreground);
}
.status-desc {
  margin-top: 6px;
  font-size: 13px;
  color: var(--color-muted-foreground);
  max-width: 460px;
  margin-left: auto;
  margin-right: auto;
}
.install-guide {
  display: inline-block;
  text-align: left;
  margin-top: 20px;
  font-size: 13px;
  color: var(--color-muted-foreground);
}
.guide-title {
  font-weight: 500;
  margin-bottom: 8px;
  color: var(--color-foreground);
}
.guide-list {
  list-style: disc;
  padding-left: 20px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.guide-label {
  font-weight: 500;
}
.guide-link {
  color: var(--color-primary);
  text-decoration: underline;
}
.guide-code {
  color: var(--color-primary);
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
}
.status-version {
  margin-top: 12px;
  font-size: 12px;
  color: var(--color-muted-foreground);
}
.search-row {
  margin-bottom: 16px;
}
.docker-tabs {
  width: 100%;
}
.log-pre {
  margin: 0;
  max-height: 500px;
  overflow: auto;
  padding: 12px;
  background-color: var(--color-muted);
  border-radius: 8px;
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
  color: var(--color-muted-foreground);
  white-space: pre-wrap;
}
.terminal-row {
  display: flex;
  gap: 8px;
  margin-bottom: 12px;
}
.terminal-input {
  font-family: "JetBrains Mono", monospace;
  flex: 1;
}
.term-output {
  margin: 0;
  max-height: 300px;
  overflow: auto;
  padding: 12px;
  background-color: var(--color-muted);
  border-radius: 8px;
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
  color: var(--color-muted-foreground);
  white-space: pre-wrap;
}
</style>
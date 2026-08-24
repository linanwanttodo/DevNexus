<script setup>
import { ref, computed, onMounted, watch } from "vue";
import { useRoute } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { showToast } from "../lib/toast.js";
import { showConfirm } from "../lib/confirm.js";
import { t, tFormat } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";
import AppIcon from "../components/AppIcon.vue";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { Badge } from "@/components/ui/badge";
import { Checkbox } from "@/components/ui/checkbox";
import { Switch } from "@/components/ui/switch";
import { Progress } from "@/components/ui/progress";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";

const route = useRoute();

// 当前标签页由路由决定 /tuning/linux|macos|windows
const activeTab = computed(() => {
  if (route.path.includes("macos")) return "macos";
  if (route.path.includes("windows")) return "windows";
  return "linux";
});

const overview = ref(null);

// ── 磁盘清理（Linux） ──
const candidates = ref([]);
const scanning = ref(false);
const cleaning = ref(false);
const selected = ref({});
const diskUsage = ref([]);
const exclusions = ref([]);
const exclusionDialog = ref(false);
const exclusionInput = ref("");

async function scan() {
  scanning.value = true;
  selected.value = {};
  try {
    candidates.value = await invoke("scan_caches");
    diskUsage.value = await invoke("get_disk_usage");
  } catch (err) {
    showToast(friendlyError(err), "error");
  } finally {
    scanning.value = false;
  }
}

async function loadExclusions() {
  try {
    exclusions.value = await invoke("list_exclusions");
  } catch { /* 忽略 */ }
}

async function addExclusion() {
  const p = exclusionInput.value.trim();
  if (!p) return;
  try {
    await invoke("add_exclusion", { path: p });
    exclusionInput.value = "";
    await loadExclusions();
  } catch (err) {
    showToast(friendlyError(err), "error");
  }
}

async function cleanSelected() {
  const paths = Object.entries(selected.value).filter(([, v]) => v).map(([k]) => k);
  if (!paths.length) return;
  cleaning.value = true;
  try {
    const freed = await invoke("clean_paths", { paths });
    showToast(tFormat("systemTune.freed", { size: humanSize(freed) }), "success");
    selected.value = {};
    await scan();
  } catch (err) {
    showToast(friendlyError(err), "error");
  } finally {
    cleaning.value = false;
  }
}

const totalFound = computed(() => candidates.value.reduce((a, c) => a + BigInt(c.bytes), 0n));
const totalSelected = computed(() => {
  let t = 0n;
  for (const [p, v] of Object.entries(selected.value)) {
    if (v) {
      const c = candidates.value.find((x) => x.path === p);
      if (c) t += BigInt(c.bytes);
    }
  }
  return t;
});

// ── Linux 工具箱状态 ──
const swapInfo = ref(null);
const dns = ref(null);
const tzInfo = ref(null);
const firewall = ref(null);
const limits = ref(null);
const cleanupTargets = ref([]);
const cudSelected = ref({});
const cudBusy = ref(false);
const swapSize = ref(1024);
const tzInput = ref("");
const dnsBusy = ref(false);

const loadingSwap = ref(false);
const loadingDns = ref(false);
const loadingTz = ref(false);
const loadingFirewall = ref(false);
const loadingLimits = ref(false);
const scanningCleanup = ref(false);

async function loadToolbox() {
  try {
    overview.value = await invoke("get_tuning_overview");
  } catch { /* 忽略 */ }
  if (!overview.value || overview.value.supported?.length === 0) return;
  await Promise.allSettled([loadSwap(), loadDns(), loadTz(), loadFirewall(), loadLimits()]);
}

async function loadSwap() {
  loadingSwap.value = true;
  try { swapInfo.value = await invoke("get_swap_info"); } catch { swapInfo.value = null; }
  finally { loadingSwap.value = false; }
}
async function loadDns() {
  loadingDns.value = true;
  try { dns.value = await invoke("get_dns_config"); } catch { dns.value = null; }
  finally { loadingDns.value = false; }
}
async function loadTz() {
  loadingTz.value = true;
  try { tzInfo.value = await invoke("get_timezone_info"); } catch { tzInfo.value = null; }
  finally { loadingTz.value = false; }
}
async function loadFirewall() {
  loadingFirewall.value = true;
  try { firewall.value = await invoke("get_firewall_status"); } catch { firewall.value = null; }
  finally { loadingFirewall.value = false; }
}
async function loadLimits() {
  loadingLimits.value = true;
  try { limits.value = await invoke("get_system_limits"); } catch { limits.value = null; }
  finally { loadingLimits.value = false; }
}

async function createSwap() {
  if (!(await showConfirm(tFormat("tuningLinux.swap_create_hint", { size: swapSize.value })))) return;
  try {
    const msg = await invoke("set_swap", { sizeMb: swapSize.value });
    showToast(msg, "success");
    await loadSwap();
  } catch (err) { showToast(friendlyError(err), "error"); }
}
async function disableSwap() {
  if (!swapInfo.value?.devices?.length) return;
  if (!(await showConfirm(t("tuningLinux.swap_disable")))) return;
  try {
    for (const d of swapInfo.value.devices) {
      await invoke("disable_swap", { path: d.filename });
    }
    showToast("Swap disabled", "success");
    await loadSwap();
  } catch (err) { showToast(friendlyError(err), "error"); }
}

async function applyDns(preset) {
  dnsBusy.value = true;
  try {
    const msg = await invoke("set_dns", { preset });
    if (msg.replace(/^DNS_/, "").length > 4) {
      showToast(t("tuningLinux.dns_apply") + " ✓", "success");
    }
    await loadDns();
  } catch (err) { showToast(friendlyError(err), "error"); }
  finally { dnsBusy.value = false; }
}

async function setTz() {
  if (!tzInput.value.trim()) return;
  if (!(await showConfirm(`${t("tuningLinux.tz_set")}: ${tzInput.value}`))) return;
  try {
    await invoke("set_timezone", { tz: tzInput.value });
    showToast("Timezone set", "success");
    tzInput.value = "";
    await loadTz();
  } catch (err) { showToast(friendlyError(err), "error"); }
}

async function toggleFirewall() {
  const enable = !(firewall.value?.ufw_active);
  if (!(await showConfirm(enable ? t("tuningLinux.firewall_enable") : t("tuningLinux.firewall_disable")))) return;
  try {
    await invoke("set_firewall", { enable });
    showToast("OK", "success");
    await loadFirewall();
  } catch (err) { showToast(friendlyError(err), "error"); }
}

async function scanCleanup() {
  scanningCleanup.value = true;
  cudSelected.value = {};
  try {
    cleanupTargets.value = await invoke("scan_cleanup_targets");
  } catch (err) { showToast(friendlyError(err), "error"); }
  finally { scanningCleanup.value = false; }
}

const dangerSelected = computed(() => {
  const ids = Object.entries(cudSelected.value).filter(([, v]) => v).map(([k]) => k);
  return cleanupTargets.value.some((t) => ids.includes(t.id) && t.risk === "dangerous");
});

async function doClean(dryRun) {
  const ids = Object.entries(cudSelected.value).filter(([, v]) => v).map(([k]) => k);
  if (!ids.length) return;
  // 危险项二次确认
  let confirmed = !dangerSelected.value;
  if (dangerSelected.value) {
    const typed = promptInfo.value;
    if (typed !== "DANGER") {
      showToast(t("tuningLinux.confirm_danger") + " ❌", "error");
      return;
    }
    confirmed = true;
  }
  // 二次确认弹窗
  if (!(await showConfirm(dryRun ? t("tuningLinux.dry_run") : t("tuningLinux.clean_targets")))) return;
  cudBusy.value = true;
  try {
    const res = await invoke("clean_targets", { targetIds: ids, dryRun, confirmed });
    if (!dryRun) {
      const freedMb = res.freed_mb || 0;
      showToast(tFormat("tuningLinux.freed_clean", { size: `${freedMb} MB` }), "success");
    } else {
      const lines = (res.executed || []).map((s) => s.replace("[dry-run] ", "")).join("\n");
      if (lines) showToast(t("tuningLinux.dry_run") + ":\n" + lines, "info");
    }
    if (!dryRun) await scanCleanup();
  } catch (err) { showToast(friendlyError(err), "error"); }
  finally { cudBusy.value = false; promptInfo.value = ""; }
}

const promptInfo = ref("");

// ── 一键优化 ──
const optimizing = ref(false);
async function doOptimize() {
  optimizing.value = true;
  try {
    const msg = await invoke("optimize_disk");
    showToast(tFormat("systemTune.optimize_done", { msg }), "success");
  } catch (err) { showToast(friendlyError(err), "error"); }
  finally { optimizing.value = false; }
}

// ── Windows 优化 ──
const winCleanItems = ref([]);
const winCleanSelected = ref({});
const winCleanScanning = ref(false);
const winCleaning = ref(false);
const winJsxsBusy = ref(false);
const winHibernation = ref(null);
const winStartup = ref([]);
const winStorage = ref([]);
const winStorageBusy = ref(false);

async function winScanCleanup() {
  winCleanScanning.value = true;
  winCleanSelected.value = {};
  try {
    winCleanItems.value = await invoke("win_scan_cleanup");
  } catch (err) { showToast(friendlyError(err), "error"); }
  finally { winCleanScanning.value = false; }
}

async function winClean() {
  const ids = Object.entries(winCleanSelected.value).filter(([, v]) => v).map(([k]) => k);
  if (!ids.length) return;
  if (!(await showConfirm(t("winOpt.clean_confirm")))) return;
  winCleaning.value = true;
  try {
    const freed = await invoke("win_clean_paths", { ids });
    showToast(tFormat("systemTune.freed", { size: humanSize(freed) }), "success");
    winCleanSelected.value = {};
    await winScanCleanup();
  } catch (err) { showToast(friendlyError(err), "error"); }
  finally { winCleaning.value = false; }
}

async function winWinsxs(resetBase) {
  if (!(await showConfirm(resetBase ? t("winOpt.winsxs_reset_confirm") : t("winOpt.winsxs_confirm")))) return;
  winJsxsBusy.value = true;
  try {
    const msg = await invoke("win_winsxs_cleanup", { resetBase });
    showToast(msg, "success");
  } catch (err) { showToast(friendlyError(err), "error"); }
  finally { winJsxsBusy.value = false; }
}

async function winToggleHibernation() {
  const enable = !(winHibernation.value?.enabled);
  if (!(await showConfirm(enable ? t("winOpt.turn_on") : t("winOpt.turn_off")))) return;
  try {
    await invoke("win_set_hibernation", { enable });
    winHibernation.value = await invoke("win_get_hibernation");
    showToast(enable ? t("winOpt.on") : t("winOpt.off"), "success");
  } catch (err) { showToast(friendlyError(err), "error"); }
}

async function winLoadStartup() {
  try {
    winStartup.value = await invoke("win_list_startup");
  } catch (err) { showToast(friendlyError(err), "error"); }
}

async function winSetStartup(entry, v) {
  try {
    await invoke("win_set_startup", { name: entry.name, hive: entry.hive, enable: v });
    entry.enabled = v;
    showToast(entry.name + (v ? " ✓" : " ✗"), "success");
  } catch (err) { showToast(friendlyError(err), "error"); }
}

async function winLoadStorage() {
  winStorageBusy.value = true;
  try {
    winStorage.value = await invoke("win_storage_usage");
  } catch (err) { showToast(friendlyError(err), "error"); }
  finally { winStorageBusy.value = false; }
}

function humanSize(n) {
  if (typeof n === "bigint") n = Number(n);
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

function riskBadge(risk) {
  return { safe: "secondary", warn: "warning", dangerous: "destructive" }[risk] || "secondary";
}

watch(activeTab, (v) => {
  if (v === "linux") loadToolbox();
  if (v === "windows") { winLoadStartup(); winLoadStorage(); }
});

onMounted(() => {
  loadExclusions();
});
</script>

<template>
  <div class="page">
    <div class="page-header">
      <div>
        <h1 class="page-title">{{ t("nav.system_tune") }}</h1>
        <p class="page-desc">
          {{ activeTab === "linux" ? t("tuningLinux.desc") : t("systemTune." + activeTab + "_desc") }}
        </p>
      </div>
    </div>

    <Tabs :default-value="activeTab" :model-value="activeTab" class="w-full">
      <TabsList class="mb-4">
        <TabsTrigger value="linux" @click="$router.push('/tuning/linux')">
          <AppIcon name="terminal" class="size-4" />
          {{ t("tuningLinux.nav") }}
        </TabsTrigger>
        <TabsTrigger value="macos" @click="$router.push('/tuning/macos')">
          <AppIcon name="apple" class="size-4" />
          {{ t("systemTune.mac") }}
        </TabsTrigger>
        <TabsTrigger value="windows" @click="$router.push('/tuning/windows')">
          <AppIcon name="monitor" class="size-4" />
          {{ t("systemTune.win") }}
        </TabsTrigger>
      </TabsList>

      <!-- ── macOS 占位 ── -->
      <TabsContent value="macos" class="space-y-4">
        <Card>
          <CardContent class="py-16 flex flex-col items-center gap-4 text-center">
            <AppIcon name="apple" class="size-14 opacity-30" />
            <h2 class="text-lg font-medium">{{ t("tuningLinux.unsupported_title") }}</h2>
            <p class="text-sm text-muted-foreground max-w-md">{{ t("tuningLinux.unsupported_desc") }}</p>
            <Button variant="outline" :disabled="optimizing" @click="doOptimize">
              <Spinner v-if="optimizing" class="size-4" />
              <AppIcon v-else name="sparkles" class="size-4" />
              {{ optimizing ? t("systemTune.optimizing") : t("systemTune.optimize") }}
            </Button>
          </CardContent>
        </Card>
      </TabsContent>

      <!-- ── Windows 优化 ── -->
      <TabsContent value="windows" class="space-y-4">
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
          <!-- 磁盘清理卡片 -->
          <Card class="shadow-sm p-4 space-y-4">
            <div class="flex items-center justify-between">
              <h2 class="text-sm font-semibold flex items-center gap-2">
                <AppIcon name="database" class="size-4" /> {{ t("winOpt.cleanup") }}
              </h2>
              <div class="flex gap-2">
                <Button size="sm" variant="outline" :disabled="winCleanScanning" @click="winScanCleanup">
                  <Spinner v-if="winCleanScanning" class="size-3.5" />
                  <AppIcon v-else name="search" class="size-3.5" />
                  {{ t("winOpt.scan") }}
                </Button>
                <Button size="sm" variant="destructive" :disabled="!Object.values(winCleanSelected).some(Boolean) || winCleaning" @click="winClean">
                  {{ winCleaning ? t("systemTune.cleaning") : t("systemTune.clean_selected") }}
                </Button>
              </div>
            </div>

            <div v-if="!winCleanItems.length && !winCleanScanning" class="text-xs text-muted-foreground">
              {{ t("winOpt.scan_hint") }}
            </div>

            <div v-else class="space-y-1.5">
              <div v-for="it in winCleanItems" :key="it.id" class="flex items-center gap-2 rounded border px-2 py-1">
                <Checkbox :checked="!!winCleanSelected[it.id]" @update:checked="(v) => (winCleanSelected[it.id] = v)" />
                <div class="flex-1 min-w-0">
                  <div class="text-xs font-medium">{{ it.name }}</div>
                  <div class="text-[11px] text-muted-foreground font-mono truncate">{{ it.path }}</div>
                </div>
                <Badge :variant="it.risk === 'warn' ? 'warning' : 'secondary'" class="text-[10px]">
                  {{ it.risk === 'warn' ? t("tuningLinux.risk_warn") : t("tuningLinux.risk_safe") }}
                </Badge>
                <span class="text-xs text-muted-foreground font-mono shrink-0">{{ humanSize(it.bytes) }}</span>
              </div>
            </div>

            <div class="pt-3 border-t space-y-2">
              <h3 class="text-xs font-semibold">{{ t("winOpt.extra") }}</h3>
              <div class="flex items-center gap-2">
                <Button size="sm" variant="outline" :disabled="winJsxsBusy" @click="winWinsxs(false)">
                  <Spinner v-if="winJsxsBusy" class="size-3.5" />
                  <AppIcon v-else name="archive" class="size-3.5" />
                  {{ t("winOpt.winsxs") }}
                </Button>
                <Button size="sm" variant="outline" :disabled="winJsxsBusy" @click="winWinsxs(true)">
                  {{ t("winOpt.winsxs_reset") }}
                </Button>
              </div>
              <div class="flex items-center justify-between rounded border px-2 py-1.5">
                <div class="text-xs">
                  <div>{{ t("winOpt.hibernation") }}</div>
                  <div class="text-muted-foreground font-mono">
                    {{ winHibernation?.enabled ? t("winOpt.on") : t("winOpt.off") }}
                    <span v-if="winHibernation?.hiberfil_mb"> · {{ humanSize(winHibernation.hiberfil_mb * 1024 * 1024) }}</span>
                  </div>
                </div>
                <Button size="sm" variant="outline" @click="winToggleHibernation">
                  {{ winHibernation?.enabled ? t("winOpt.turn_off") : t("winOpt.turn_on") }}
                </Button>
              </div>
            </div>
          </Card>

          <!-- 启动项 + 存储卡片 -->
          <Card class="shadow-sm p-4 space-y-4">
            <div class="flex items-center justify-between">
              <h2 class="text-sm font-semibold flex items-center gap-2">
                <AppIcon name="settings" class="size-4" /> {{ t("winOpt.startup") }}
              </h2>
              <Button size="sm" variant="outline" @click="winLoadStartup">
                <AppIcon name="refresh" class="size-3.5" />
              </Button>
            </div>

            <div v-if="!winStartup.length" class="text-xs text-muted-foreground">{{ t("winOpt.startup_hint") }}</div>
            <div v-else class="space-y-1.5 max-h-52 overflow-y-auto">
              <div v-for="(s, si) in winStartup" :key="si" class="flex items-center gap-2 rounded border px-2 py-1">
                <span class="text-xs font-medium truncate flex-1">{{ s.name }}</span>
                <Badge :variant="s.hive === 'HKLM' ? 'outline' : 'secondary'" class="text-[10px]">{{ s.hive }}</Badge>
                <Switch :checked="s.enabled" @update:checked="(v) => winSetStartup(s, v)" />
              </div>
            </div>

            <div class="pt-3 border-t space-y-2">
              <h3 class="text-xs font-semibold">{{ t("systemTune.disk_usage") }}</h3>
              <div v-if="!winStorage.length" class="text-xs text-muted-foreground">{{ t("winOpt.storage_hint") }}</div>
              <div v-else class="space-y-2">
                <div v-for="d in winStorage" :key="d.mount" class="space-y-1">
                  <div class="text-xs font-medium">{{ d.mount }} <span class="text-muted-foreground font-normal">({{ d.format }})</span></div>
                  <Progress :model-value="Math.round(d.used_bytes / d.total_bytes * 100)" class="h-2" />
                  <div class="flex justify-between text-[11px] text-muted-foreground">
                    <span>{{ humanSize(d.used_bytes) }} / {{ humanSize(d.total_bytes) }}</span>
                    <span>{{ t("systemTune.free") }}: {{ humanSize(d.free_bytes) }}</span>
                  </div>
                </div>
              </div>
              <Button size="sm" variant="outline" :disabled="winStorageBusy" @click="winLoadStorage">
                <Spinner v-if="winStorageBusy" class="size-3.5" />
                <AppIcon v-else name="bar-chart" class="size-3.5" />
                {{ t("winOpt.refresh_storage") }}
              </Button>
            </div>
          </Card>
        </div>
      </TabsContent>

      <!-- ── Linux 工具箱 ── -->
      <TabsContent value="linux" class="space-y-4">
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
          <!-- 工具箱卡片 -->
          <Card class="shadow-sm p-4 space-y-5">
            <h2 class="text-sm font-semibold flex items-center gap-2">
              <AppIcon name="tool" class="size-4" /> {{ t("tuningLinux.toolbox") }}
            </h2>

            <!-- Swap -->
            <div class="space-y-2">
              <div class="flex items-center justify-between">
                <span class="text-xs font-medium">{{ t("tuningLinux.swap_title") }}</span>
                <Badge :variant="swapInfo?.enabled ? 'secondary' : 'outline'">
                  {{ swapInfo?.enabled ? t("tuningLinux.swap_enabled") : t("tuningLinux.swap_disabled") }}
                </Badge>
              </div>
              <div v-if="loadingSwap" class="text-xs text-muted-foreground">
                <Spinner class="size-3" />
              </div>
              <div v-else class="text-xs text-muted-foreground space-y-1">
                <div v-if="swapInfo?.devices?.length">
                  <div v-for="d in swapInfo.devices" :key="d.filename" class="flex justify-between font-mono">
                    <span>{{ d.filename }}</span>
                    <span>{{ humanSize(d.size_mb * 1024 * 1024) }}（{{ t("systemTune.used") }} {{ humanSize(d.used_mb * 1024 * 1024) }}）</span>
                  </div>
                </div>
                <div v-else>{{ t("tuningLinux.swap_disabled") }}</div>
                <div class="flex items-center gap-2 pt-1">
                  <Input v-model="swapSize" type="number" min="256" class="w-20 h-7 text-xs" />
                  <Button size="sm" variant="outline" class="h-7" @click="createSwap">
                    {{ t("tuningLinux.swap_create") }}
                  </Button>
                  <Button v-if="swapInfo?.enabled" size="sm" variant="ghost" class="h-7 text-destructive" @click="disableSwap">
                    {{ t("tuningLinux.swap_disable") }}
                  </Button>
                </div>
              </div>
            </div>

            <!-- DNS -->
            <div class="space-y-2">
              <span class="text-xs font-medium">{{ t("tuningLinux.dns_title") }}</span>
              <div class="text-xs text-muted-foreground font-mono" v-if="dns">
                <div v-for="ns in dns.nameservers" :key="ns">{{ ns }}</div>
              </div>
              <div class="flex flex-wrap gap-1.5 pt-1">
                <Button size="sm" variant="outline" class="h-7" :disabled="dnsBusy" @click="applyDns('114')">
                  {{ t("tuningLinux.dns_preset_114") }}
                </Button>
                <Button size="sm" variant="outline" class="h-7" :disabled="dnsBusy" @click="applyDns('google')">
                  {{ t("tuningLinux.dns_preset_google") }}
                </Button>
                <Button size="sm" variant="outline" class="h-7" :disabled="dnsBusy" @click="applyDns('cloudflare')">
                  {{ t("tuningLinux.dns_preset_cloudflare") }}
                </Button>
                <Button size="sm" variant="outline" class="h-7" :disabled="dnsBusy" @click="applyDns('ali')">
                  {{ t("tuningLinux.dns_preset_ali") }}
                </Button>
              </div>
            </div>

            <!-- 时区 -->
            <div class="space-y-2">
              <span class="text-xs font-medium">{{ t("tuningLinux.tz_title") }}</span>
              <div class="text-xs text-muted-foreground space-y-1">
                <div>{{ t("tuningLinux.tz_current") }}: <span class="font-mono">{{ tzInfo?.timezone || "-" }}</span></div>
                <div>{{ t("tuningLinux.tz_ntp") }}: {{ tzInfo?.ntp_enabled ? t("tuningLinux.swap_enabled") : t("tuningLinux.swap_disabled") }}</div>
              </div>
              <div class="flex items-center gap-2 pt-1">
                <Input v-model="tzInput" class="h-7 text-xs" placeholder="Asia/Shanghai" @keydown.enter="setTz" />
                <Button size="sm" class="h-7" @click="setTz">{{ t("tuningLinux.tz_set") }}</Button>
              </div>
            </div>

            <!-- 防火墙 -->
            <div class="space-y-2">
              <div class="flex items-center justify-between">
                <span class="text-xs font-medium">{{ t("tuningLinux.firewall_title") }}</span>
                <Badge :variant="firewall?.ufw_active ? 'secondary' : 'outline'">
                  {{ firewall?.ufw_active ? t("tuningLinux.firewall_ufw_active") : t("tuningLinux.firewall_ufw_inactive") }}
                </Badge>
              </div>
              <div class="text-xs text-muted-foreground">{{ t("tuningLinux.firewall_ufw_active") }}</div>
              <Button size="sm" variant="outline" class="h-7" @click="toggleFirewall">
                {{ firewall?.ufw_active ? t("tuningLinux.firewall_disable") : t("tuningLinux.firewall_enable") }}
              </Button>
            </div>

            <!-- 系统限制 -->
            <div class="space-y-2">
              <span class="text-xs font-medium">{{ t("tuningLinux.limits_title") }}</span>
              <div v-if="limits" class="text-xs text-muted-foreground space-y-0.5 font-mono">
                <div>{{ t("tuningLinux.limits_nofile") }}: {{ limits.nofile_soft }} / {{ limits.nofile_hard }}</div>
                <div>{{ t("tuningLinux.limits_core") }}: {{ limits.core_dump }}</div>
                <div>{{ t("tuningLinux.limits_procs") }}: {{ limits.max_user_processes }}</div>
              </div>
            </div>
          </Card>

          <!-- 日志清理卡片 -->
          <Card class="shadow-sm p-4 space-y-4">
            <div class="flex items-center justify-between">
              <h2 class="text-sm font-semibold flex items-center gap-2">
                <AppIcon name="database" class="size-4" /> {{ t("tuningLinux.cleanup") }}
              </h2>
              <Button size="sm" variant="outline" :disabled="scanningCleanup" @click="scanCleanup">
                <Spinner v-if="scanningCleanup" class="size-3.5" />
                <AppIcon v-else name="search" class="size-3.5" />
                {{ t("tuningLinux.scan_clean") }}
              </Button>
            </div>

            <div v-if="!cleanupTargets.length && !scanningCleanup" class="text-xs text-muted-foreground">
              {{ t("systemTune.scan_hint") }}
            </div>

            <div v-else class="space-y-2">
              <div v-for="t in cleanupTargets" :key="t.id" class="flex items-start gap-2 rounded border p-2">
                <Checkbox :checked="!!cudSelected[t.id]" @update:checked="(v) => (cudSelected[t.id] = v)" />
                <div class="flex-1 min-w-0">
                  <div class="flex items-center gap-2">
                    <span class="text-xs font-medium truncate">{{ t.name }}</span>
                    <Badge :variant="riskBadge(t.risk)">{{ t("tuningLinux.risk_" + t.risk) }}</Badge>
                  </div>
                  <div class="text-xs text-muted-foreground truncate">{{ t.description }}</div>
                  <div class="text-[11px] text-muted-foreground/70 font-mono truncate">{{ t.action }}</div>
                </div>
              </div>

              <div class="flex items-center gap-2 pt-1" v-if="dangerSelected">
                <Input v-model="promptInfo" :placeholder="t('tuningLinux.confirm_placeholder')" class="h-7 text-xs flex-1" />
              </div>

              <div class="flex items-center gap-2">
                <Button size="sm" :disabled="cudBusy" @click="doClean(true)">
                  {{ t("tuningLinux.dry_run") }}
                </Button>
                <Button size="sm" variant="destructive" :disabled="cudBusy" @click="doClean(false)">
                  {{ t("tuningLinux.clean_targets") }}
                </Button>
              </div>
            </div>

            <!-- 磁盘清理概览 -->
            <div class="pt-3 border-t space-y-3">
              <div class="flex items-center gap-2">
                <Button size="sm" variant="outline" :disabled="scanning" @click="scan">
                  {{ scanning ? t("systemTune.scanning") : (candidates.length ? t("systemTune.rescan") : t("systemTune.scan")) }}
                </Button>
                <Button size="sm" variant="destructive" :disabled="!Object.values(selected).some(Boolean) || cleaning" @click="cleanSelected">
                  {{ cleaning ? t("systemTune.cleaning") : t("systemTune.clean_selected") }}
                </Button>
                <Button size="sm" variant="ghost" @click="exclusionDialog = true">
                  <AppIcon name="shield" class="size-3.5" /> {{ t("systemTune.exclusions") }}
                </Button>
              </div>

              <div v-if="candidates.length" class="space-y-1.5">
                <div v-for="c in candidates" :key="c.id" class="flex items-center gap-2 rounded border px-2 py-1">
                  <Checkbox :checked="!!selected[c.path]" @update:checked="(v) => (selected[c.path] = v)" />
                  <span class="text-xs truncate flex-1">{{ c.name }}</span>
                  <span class="text-xs text-muted-foreground font-mono">{{ humanSize(c.bytes) }}</span>
                </div>
              </div>
            </div>
          </Card>
        </div>
      </TabsContent>
    </Tabs>

    <!-- 排除项对话框 -->
    <div v-if="exclusionDialog" class="fixed inset-0 z-50 flex items-center justify-center bg-black/40" @click.self="exclusionDialog = false">
      <div class="w-[380px] rounded-lg border bg-card p-4">
        <h2 class="text-sm font-semibold mb-3">{{ t("systemTune.exclusions") }}</h2>
        <div class="space-y-2">
          <div class="flex gap-2">
            <Input v-model="exclusionInput" :placeholder="t('systemTune.add_exclusion')" class="h-7 text-xs flex-1" @keydown.enter="addExclusion" />
            <Button size="sm" class="h-7" @click="addExclusion">{{ t("systemTune.add_exclusion") }}</Button>
          </div>
          <div v-for="ep in exclusions" :key="ep" class="flex items-center justify-between rounded border px-2 py-1 text-xs">
            <span class="truncate font-mono">{{ ep }}</span>
            <Button size="sm" variant="ghost" class="text-destructive h-6" @click="invoke('remove_exclusion', { path: ep }).then(loadExclusions)">✕</Button>
          </div>
        </div>
        <div class="flex justify-end mt-3">
          <Button size="sm" variant="outline" @click="exclusionDialog = false">{{ t("common.close") }}</Button>
        </div>
      </div>
    </div>
  </div>
</template>
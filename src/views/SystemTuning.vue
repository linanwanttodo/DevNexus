<script setup>
import { ref, computed, onMounted, watch } from "vue";
import { useRoute } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { showToast } from "../lib/toast.js";
import { t, tFormat } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";
import AppIcon from "../components/AppIcon.vue";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { Progress } from "@/components/ui/progress";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyMedia,
} from "@/components/ui/empty";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

const route = useRoute();

// 当前激活的标签页，由路由决定
const activeTab = computed(() => {
  if (route.path.startsWith("/system-tune/mac")) return "mac";
  if (route.path.startsWith("/system-tune/win")) return "win";
  return "disk";
});

// ── 磁盘清理 ──
const candidates = ref([]);
const scanning = ref(false);
const cleaning = ref(false);
const selected = ref({});
const diskUsage = ref([]);
const exclusions = ref([]);
const exclusionInput = ref("");
const exclusionDialog = ref(false);

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

async function removeExclusion(path) {
  try {
    await invoke("remove_exclusion", { path });
    await loadExclusions();
  } catch (err) {
    showToast(friendlyError(err), "error");
  }
}

async function cleanSelected() {
  const paths = Object.entries(selected.value)
    .filter(([, v]) => v)
    .map(([k]) => k);
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

const totalSelected = computed(() => {
  let total = 0n;
  for (const [p, v] of Object.entries(selected.value)) {
    if (v) {
      const c = candidates.value.find((x) => x.path === p);
      if (c) total += BigInt(c.bytes);
    }
  }
  return total;
});

const totalFound = computed(() => {
  return candidates.value.reduce((a, c) => a + BigInt(c.bytes), 0n);
});

// ── 一键优化 ──
const optimizing = ref(false);
async function doOptimize() {
  optimizing.value = true;
  try {
    const msg = await invoke("optimize_disk");
    showToast(tFormat("systemTune.optimize_done", { msg }), "success");
  } catch (err) {
    showToast(friendlyError(err), "error");
  } finally {
    optimizing.value = false;
  }
}

function humanSize(n) {
  if (typeof n === "bigint") n = Number(n);
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

onMounted(() => {
  loadExclusions();
});
</script>

<template>
  <div class="page">
    <div class="page-header">
      <div>
        <h1 class="page-title">{{ t("nav.system_tune") }}</h1>
        <p class="page-desc">{{ t("systemTune." + activeTab + "_desc") }}</p>
      </div>
    </div>

    <Tabs :default-value="activeTab" :model-value="activeTab" class="w-full">
      <TabsList class="mb-4">
        <TabsTrigger value="disk" @click="$router.push('/system-tune')">
          <AppIcon name="database" class="size-4" />
          {{ t("systemTune.disk") }}
        </TabsTrigger>
        <TabsTrigger value="mac" @click="$router.push('/system-tune/mac')">
          <AppIcon name="apple" class="size-4" />
          {{ t("systemTune.mac") }}
        </TabsTrigger>
        <TabsTrigger value="win" @click="$router.push('/system-tune/win')">
          <AppIcon name="monitor" class="size-4" />
          {{ t("systemTune.win") }}
        </TabsTrigger>
      </TabsList>

      <!-- ── 磁盘清理 ── -->
      <TabsContent value="disk" class="space-y-4">
        <div class="flex items-center gap-2">
          <Button :disabled="scanning" @click="scan">
            <Spinner v-if="scanning" class="size-4" />
            <AppIcon v-else name="search" class="size-4" />
            {{ scanning ? t("systemTune.scanning") : (candidates.length ? t("systemTune.rescan") : t("systemTune.scan")) }}
          </Button>
          <Button
            variant="outline"
            :disabled="!Object.values(selected).some(Boolean) || cleaning"
            @click="cleanSelected"
          >
            <Spinner v-if="cleaning" class="size-4" />
            <AppIcon v-else name="delete" class="size-4" />
            {{ cleaning ? t("systemTune.cleaning") : t("systemTune.clean_selected") }}
          </Button>
          <Button variant="ghost" @click="exclusionDialog = true">
            <AppIcon name="shield" class="size-4" />
            {{ t("systemTune.exclusions") }}
          </Button>
          <div class="flex-1" />
          <span class="text-xs text-muted-foreground" v-if="candidates.length">
            {{ tFormat("systemTune.total_found", { size: humanSize(totalFound) }) }}
            · {{ tFormat("systemTune.selected_total", { size: humanSize(totalSelected) }) }}
          </span>
        </div>

        <div v-if="!candidates.length && !scanning" class="flex flex-col items-center gap-2 py-14 text-muted-foreground">
          <AppIcon name="database" class="size-10 opacity-40" />
          <p class="text-sm">{{ t("systemTune.scan_hint") }}</p>
        </div>

        <Card v-if="diskUsage.length" class="shadow-sm">
          <CardHeader class="py-3">
            <CardTitle class="text-sm font-medium">{{ t("systemTune.disk_usage") }}</CardTitle>
          </CardHeader>
          <CardContent class="pb-3">
            <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
              <div v-for="d in diskUsage" :key="d.mount" class="rounded border p-3 space-y-1">
                <div class="text-xs font-medium">{{ d.mount }}</div>
                <Progress :model-value="Math.round(d.used_bytes / d.total_bytes * 100)" class="h-2" />
                <div class="flex justify-between text-xs text-muted-foreground">
                  <span>{{ t("systemTune.used") }}: {{ humanSize(d.used_bytes) }}</span>
                  <span>{{ t("systemTune.free") }}: {{ humanSize(d.free_bytes) }}</span>
                  <span>{{ t("systemTune.total") }}: {{ humanSize(d.total_bytes) }}</span>
                </div>
              </div>
            </div>
          </CardContent>
        </Card>

        <Card v-if="candidates.length" class="shadow-sm">
          <CardContent class="p-0">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead class="w-10">
                    <Checkbox
                      :checked="Object.values(selected).filter(Boolean).length === candidates.length && candidates.length > 0"
                      @update:checked="(v) => { candidates.forEach(c => selected[c.path] = v); }"
                    />
                  </TableHead>
                  <TableHead>{{ t("systemTune.name") }}</TableHead>
                  <TableHead class="w-[120px]">{{ t("systemTune.size") }}</TableHead>
                  <TableHead class="w-[80px]">{{ t("systemTune.files") }}</TableHead>
                  <TableHead class="w-[200px] hidden sm:table-cell">{{ t("systemTune.path") }}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                <TableRow v-for="c in candidates" :key="c.id">
                  <TableCell>
                    <Checkbox :checked="!!selected[c.path]" @update:checked="(v) => selected[c.path] = v" />
                  </TableCell>
                  <TableCell>{{ c.name }}</TableCell>
                  <TableCell class="font-mono text-xs">{{ humanSize(c.bytes) }}</TableCell>
                  <TableCell class="text-muted-foreground text-xs">{{ c.file_count }}</TableCell>
                  <TableCell class="text-muted-foreground font-mono text-xs truncate hidden sm:table-cell max-w-[200px]">
                    {{ c.path }}
                  </TableCell>
                </TableRow>
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      </TabsContent>

      <!-- ── macOS 优化 ── -->
      <TabsContent value="mac" class="space-y-4">
        <Card>
          <CardContent class="py-8 flex flex-col items-center gap-4 text-center">
            <AppIcon name="apple" class="size-12 opacity-40" />
            <p class="text-sm text-muted-foreground max-w-md">
              {{ t("systemTune.mac_desc") }}
            </p>
            <Button :disabled="optimizing" @click="doOptimize">
              <Spinner v-if="optimizing" class="size-4" />
              <AppIcon v-else name="sparkles" class="size-4" />
              {{ optimizing ? t("systemTune.optimizing") : t("systemTune.optimize") }}
            </Button>
            <p class="text-xs text-muted-foreground">{{ t("systemTune.optimize_hint") }}</p>
          </CardContent>
        </Card>
      </TabsContent>

      <!-- ── Windows 优化 ── -->
      <TabsContent value="win" class="space-y-4">
        <Card>
          <CardContent class="py-8 flex flex-col items-center gap-4 text-center">
            <AppIcon name="monitor" class="size-12 opacity-40" />
            <p class="text-sm text-muted-foreground max-w-md">
              {{ t("systemTune.win_desc") }}
            </p>
            <Button :disabled="optimizing" @click="doOptimize">
              <Spinner v-if="optimizing" class="size-4" />
              <AppIcon v-else name="sparkles" class="size-4" />
              {{ optimizing ? t("systemTune.optimizing") : t("systemTune.optimize") }}
            </Button>
            <p class="text-xs text-muted-foreground">{{ t("systemTune.optimize_hint") }}</p>
          </CardContent>
        </Card>
      </TabsContent>
    </Tabs>

    <!-- 排除项对话框 -->
    <Dialog :open="exclusionDialog" @update:open="(v) => !v && (exclusionDialog = false)">
      <DialogContent class="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>{{ t("systemTune.exclusions") }}</DialogTitle>
        </DialogHeader>
        <div class="space-y-3">
          <div class="flex items-center gap-2">
            <Input v-model="exclusionInput" :placeholder="t('systemTune.add_exclusion')" @keydown.enter="addExclusion" />
            <Button size="sm" @click="addExclusion">{{ t("systemTune.add_exclusion") }}</Button>
          </div>
          <div v-if="exclusions.length === 0" class="text-xs text-muted-foreground py-2">
            {{ t("systemTune.empty") }}
          </div>
          <div v-for="ep in exclusions" :key="ep" class="flex items-center justify-between gap-2 rounded border px-3 py-2 text-sm">
            <span class="truncate font-mono text-xs">{{ ep }}</span>
            <Button size="sm" variant="ghost" class="text-destructive" @click="removeExclusion(ep)">
              {{ t("systemTune.remove_exclusion") }}
            </Button>
          </div>
        </div>
        <DialogFooter>
          <Button @click="exclusionDialog = false">{{ t("common.close") }}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
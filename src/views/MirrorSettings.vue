<script setup>
import { ref, computed, onMounted } from "vue";
import { useRoute } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { showToast } from "../lib/toast.js";
import { t } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";
import AppIcon from "../components/AppIcon.vue";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardFooter, CardHeader } from "@/components/ui/card";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Badge } from "@/components/ui/badge";
import {
  Alert,
  AlertDescription,
  AlertTitle,
} from "@/components/ui/alert";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";

const route = useRoute();

// 当前子导航分组（/mirrors/npm → "npm"；/mirrors → null = 全部）
const routeGroup = computed(() => {
  const seg = route.path.split("/")[2];
  return seg || null;
});

const groups = ref([]);
const loading = ref(true);
const error = ref(null);
const testingUrls = ref({});
const testingGroup = ref(null);
const selectedCountry = ref("all");

const countries = computed(() => [
  { id: "all", label: t("mirrors.country_all") },
  { id: "CN", label: t("mirrors.country_cn") },
  { id: "RU", label: t("mirrors.country_ru") },
  { id: "US", label: t("mirrors.country_us") },
  { id: "EU", label: t("mirrors.country_eu") },
  { id: "JP", label: t("mirrors.country_jp") },
  { id: "AU", label: t("mirrors.country_au") },
]);

async function loadMirrors() {
  try {
    loading.value = true;
    error.value = null;
    groups.value = await invoke("list_mirrors");
    // 标记当前激活的镜像
    for (const g of groups.value) {
      for (const m of g.mirrors) {
        m.is_active = g.current_url && m.url === g.current_url;
      }
    }
  } catch (err) {
    error.value = friendlyError(err);
  } finally {
    loading.value = false;
  }
}

async function testMirror(groupId, mirrorUrl) {
  testingUrls.value = { ...testingUrls.value, [mirrorUrl]: true };
  try {
    const latency = await invoke("test_mirror_latency", { url: mirrorUrl });
    for (const g of groups.value) {
      if (g.id === groupId) {
        for (const m of g.mirrors) {
          if (m.url === mirrorUrl) {
            m.latency_ms = latency;
          }
        }
      }
    }
  } catch (err) {
    console.error("Latency test failed:", err);
  } finally {
    const next = { ...testingUrls.value };
    delete next[mirrorUrl];
    testingUrls.value = next;
  }
}

async function switchMirror(groupId, mirrorUrl) {
  try {
    const msg = await invoke("switch_mirror", { mirrorId: groupId, url: mirrorUrl });
    showToast(msg, "success");
    await loadMirrors();
  } catch (err) {
    showToast(t("common.error_msg").replace("{error}", friendlyError(err)), "error");
  }
}

async function testAllMirrors(group) {
  testingGroup.value = group.id;
  // 清除之前的推荐标记
  for (const m of group.mirrors) {
    m.recommended = false;
  }
  try {
    const results = await Promise.all(
      group.mirrors.map(async (m) => {
        try {
          const latency = await invoke("test_mirror_latency", { url: m.url });
          m.latency_ms = latency;
          return { mirror: m, latency };
        } catch {
          m.latency_ms = 0;
          return { mirror: m, latency: Infinity };
        }
      })
    );
    // 找到最快的（排除超时）
    const fastest = results.reduce(
      (best, cur) => (cur.latency > 0 && cur.latency < best.latency ? cur : best),
      { latency: Infinity }
    );
    if (fastest.mirror) {
      fastest.mirror.recommended = true;
    }
  } catch (err) {
    console.error("Batch test failed:", err);
  } finally {
    testingGroup.value = null;
  }
}

function getCountryFlag(code) {
  const flags = { CN: "CN", RU: "RU", US: "US", EU: "EU", JP: "JP", AU: "AU" };
  return flags[code] || code;
}

const filteredGroups = computed(() =>
  groups.value
    // 侧边栏子导航：/mirrors/npm 之类只显示对应包管理器分组
    .filter((g) => !routeGroup.value || g.id === routeGroup.value)
    .map((g) => ({
      ...g,
      mirrors: g.mirrors.filter(
        (m) => selectedCountry.value === "all" || m.country === selectedCountry.value
      ),
    }))
);

function latencyLabel(mirror) {
  if (mirror.latency_ms > 0) return `${mirror.latency_ms}ms`;
  if (mirror.latency_ms === 0) return t("mirrors.timeout");
  return t("mirrors.test");
}

onMounted(() => {
  loadMirrors();
});
</script>

<template>
  <div class="page mirrors-page">
    <div class="page-header">
      <div>
        <h1 class="page-title">{{ t("mirrors.title") }}</h1>
        <p class="page-desc">{{ t("mirrors.description") }}</p>
      </div>
      <div class="flex gap-2 items-center">
        <Select v-model="selectedCountry">
          <SelectTrigger class="w-[130px]">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem v-for="c in countries" :key="c.id" :value="c.id">
              {{ c.label }}
            </SelectItem>
          </SelectContent>
        </Select>
        <Button variant="outline" @click="loadMirrors">{{ t("common.refresh") }}</Button>
      </div>
    </div>

    <!-- 加载态 -->
    <div v-if="loading" class="space-y-3">
      <Skeleton v-for="i in 2" :key="i" class="h-36 w-full" />
    </div>

    <!-- 错误 -->
    <Alert v-else-if="error" variant="destructive" class="py-4">
      <AppIcon name="close-circle-fill" class="size-4" />
      <AlertTitle>{{ t("error.title") }}</AlertTitle>
      <AlertDescription>{{ error }}</AlertDescription>
      <Button variant="outline" size="sm" class="mt-2" @click="loadMirrors">
        {{ t("common.retry") }}
      </Button>
    </Alert>

    <div v-else class="group-list">
      <Card
        v-for="group in filteredGroups"
        :key="group.id"
        class="section-card shadow-sm"
      >
        <CardHeader class="pb-3">
          <div class="group-head">
            <span class="group-label">{{ group.label }}</span>
            <span v-if="group.current_url" class="text-xs text-muted-foreground">
              {{ t("mirrors.active_prefix") }}: {{ group.current_url }}
            </span>
          </div>
        </CardHeader>

        <CardContent class="pt-0">
          <div class="mirror-list">
            <div
              v-for="mirror in group.mirrors"
              :key="mirror.url"
              class="mirror-row"
              :class="mirror.is_active ? 'border-primary/60 bg-primary/10' : ''"
            >
              <div class="mirror-left">
                <span class="country-flag">{{ getCountryFlag(mirror.country) }}</span>
                <div class="mirror-info">
                  <div class="mirror-name">{{ mirror.name }}</div>
                  <code class="mirror-url">{{ mirror.url }}</code>
                </div>
              </div>
              <div class="mirror-right">
                <Button
                  variant="outline"
                  size="sm"
                  :disabled="!!testingUrls[mirror.url] || testingGroup !== null"
                  @click="testMirror(group.id, mirror.url)"
                >
                  <Spinner v-if="testingUrls[mirror.url]" class="size-3.5" />
                  {{ testingUrls[mirror.url] ? "..." : latencyLabel(mirror) }}
                </Button>
                <Badge
                  v-if="mirror.is_active"
                  class="border-transparent bg-success/15 text-success dark:text-success"
                >
                  {{ t("mirrors.active") }}
                </Badge>
                <Badge v-else-if="mirror.recommended" variant="secondary">
                  {{ t("mirrors.recommended") }}
                </Badge>
                <Button
                  v-if="mirror.is_active || mirror.recommended"
                  size="sm"
                  @click="switchMirror(group.id, mirror.url)"
                >
                  {{ t("mirrors.use") }}
                </Button>
                <Button
                  v-else
                  variant="outline"
                  size="sm"
                  @click="switchMirror(group.id, mirror.url)"
                >
                  {{ t("mirrors.use") }}
                </Button>
              </div>
            </div>
          </div>
        </CardContent>

        <CardFooter class="border-t pt-3">
          <Button
            variant="outline"
            size="sm"
            :disabled="testingGroup !== null"
            @click="testAllMirrors(group)"
          >
            <Spinner v-if="testingGroup === group.id" class="size-3.5" />
            {{ testingGroup === group.id ? "..." : t("mirrors.test_all") }}
          </Button>
        </CardFooter>
      </Card>
    </div>
  </div>
</template>

<style scoped>
.group-head {
  display: flex;
  align-items: center;
  gap: 12px;
}
.group-label {
  font-weight: 600;
  color: var(--color-primary);
  font-size: 14px;
}
.mirror-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.mirror-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 12px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  transition: border-color 0.15s ease, background-color 0.15s ease;
}
.mirror-row:hover {
  border-color: var(--color-ring);
}
.mirror-left {
  display: flex;
  align-items: center;
  gap: 12px;
  flex: 1;
  min-width: 0;
}
.country-flag {
  width: 36px;
  flex-shrink: 0;
  font-size: 12px;
  color: var(--color-muted-foreground);
  font-family: "JetBrains Mono", monospace;
}
.mirror-info {
  min-width: 0;
}
.mirror-name {
  font-size: 14px;
  color: var(--color-foreground);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.mirror-url {
  display: block;
  max-width: 460px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
  color: var(--color-muted-foreground);
}
.mirror-right {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}
</style>
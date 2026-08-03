<script setup>
import { ref, computed, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { showToast } from "../lib/toast.js";
import { t } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";

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
  groups.value.map((g) => ({
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
        <a-select v-model="selectedCountry" style="width: 130px">
          <a-option v-for="c in countries" :key="c.id" :value="c.id">
            {{ c.label }}
          </a-option>
        </a-select>
        <a-button @click="loadMirrors">{{ t("common.refresh") }}</a-button>
      </div>
    </div>

    <a-spin :loading="loading" style="width: 100%">
      <a-result
        v-if="error"
        status="error"
        :title="error"
        style="padding: 48px 0"
      >
        <template #extra>
          <a-button type="primary" @click="loadMirrors">{{ t("common.retry") }}</a-button>
        </template>
      </a-result>

      <div v-else class="group-list">
        <a-card
          v-for="group in filteredGroups"
          :key="group.id"
          :bordered="true"
          class="section-card"
        >
          <template #title>
            <div class="group-head">
              <span class="group-label">{{ group.label }}</span>
              <a-typography-text
                v-if="group.current_url"
                type="secondary"
                style="font-size: 12px"
              >
                {{ t("mirrors.active_prefix") }}: {{ group.current_url }}
              </a-typography-text>
            </div>
          </template>

          <div class="mirror-list">
            <div
              v-for="mirror in group.mirrors"
              :key="mirror.url"
              class="mirror-row"
              :class="{ active: mirror.is_active }"
            >
              <div class="mirror-left">
                <span class="country-flag">{{ getCountryFlag(mirror.country) }}</span>
                <div class="mirror-info">
                  <div class="mirror-name">{{ mirror.name }}</div>
                  <a-typography-text code type="secondary" class="mirror-url">
                    {{ mirror.url }}
                  </a-typography-text>
                </div>
              </div>
              <div class="mirror-right">
                <a-button
                  size="mini"
                  @click="testMirror(group.id, mirror.url)"
                  :disabled="!!testingUrls[mirror.url] || testingGroup !== null"
                >
                  {{ testingUrls[mirror.url] ? "..." : latencyLabel(mirror) }}
                </a-button>
                <a-tag v-if="mirror.is_active" color="green">{{ t("mirrors.active") }}</a-tag>
                <a-tag v-else-if="mirror.recommended" color="arcoblue">{{ t("mirrors.recommended") }}</a-tag>
                <a-button
                  v-if="mirror.is_active || mirror.recommended"
                  size="mini"
                  type="primary"
                  @click="switchMirror(group.id, mirror.url)"
                >
                  {{ t("mirrors.use") }}
                </a-button>
                <a-button
                  v-else
                  size="mini"
                  @click="switchMirror(group.id, mirror.url)"
                >
                  {{ t("mirrors.use") }}
                </a-button>
              </div>
            </div>
          </div>

          <template #footer>
            <a-button
              size="mini"
              :loading="testingGroup === group.id"
              :disabled="testingGroup !== null"
              @click="testAllMirrors(group)"
            >
              {{ testingGroup === group.id ? "..." : t("mirrors.test_all") }}
            </a-button>
          </template>
        </a-card>
      </div>
    </a-spin>
  </div>
</template>

<style scoped>
.mirrors-page {
  padding: 20px 24px;
  max-width: 1000px;
  margin: 0 auto;
}
.group-head {
  display: flex;
  align-items: center;
  gap: 12px;
}
.group-label {
  font-weight: 600;
  color: var(--color-primary-6);
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
  border-radius: 8px;
  transition: border-color 0.15s ease, background-color 0.15s ease;
}
.mirror-row:hover {
  border-color: var(--color-border-2);
}
.mirror-row.active {
  border-color: var(--color-primary-6);
  background-color: var(--color-primary-1);
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
  color: var(--color-text-3);
  font-family: "JetBrains Mono", monospace;
}
.mirror-info {
  min-width: 0;
}
.mirror-name {
  font-size: 14px;
  color: var(--color-text-1);
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
}
.mirror-right {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}
</style>

<script setup>
import { ref, computed, watch, onMounted } from "vue";
import { useRouter } from "vue-router";
import { t } from "../lib/i18n.js";
import {
  getIslandEnabled,
  setIslandEnabled,
  getDeepSeekKey,
  setDeepSeekKey,
} from "../lib/stores.js";
import {
  applyIslandState,
  isIslandVisible,
} from "../lib/island.js";
import AppIcon from "../components/AppIcon.vue";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Input } from "@/components/ui/input";
import deepseekIcon from "../assets/deepseek.png";

const router = useRouter();

const islandEnabled = getIslandEnabled();
const deepSeekKey = getDeepSeekKey();
const isTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

// 输入即保存：key 一旦变化立即持久化（localStorage + Rust 内存，供岛窗口读取），
// 避免"查询能显示、下次打开就没了"——之前只 v-model 绑定 ref 却从不调用
// setDeepSeekKey，导致 key 从未真正落盘。
const keySaved = ref(false);

// 显示/隐藏 key（默认隐藏，点眼睛切换）
const showKey = ref(false);

watch(
  deepSeekKey,
  (val) => {
    setDeepSeekKey(val);
    keySaved.value = true;
    // 保存后自动刷新一次余额展示（若有旧结果则清除，等待用户查询）
    if (val.trim()) {
      balanceResult.value = null;
      balanceError.value = "";
    }
  },
  { immediate: false }
);

// 当前窗口是否显示（实时查询，与启用开关解耦：预览操作不改变持久化偏好）
const visible = ref(false);

async function refreshVisible() {
  visible.value = await isIslandVisible();
}

const statusText = computed(() =>
  islandEnabled.value ? t("island.enabled_state_on") : t("island.enabled_state_off")
);
const statusVariant = computed(() =>
  islandEnabled.value ? "default" : "secondary"
);

const visibleText = computed(() =>
  visible.value ? t("island.visible_on") : t("island.visible_off")
);

async function onToggle(value) {
  setIslandEnabled(value);
  await applyIslandState();
  await refreshVisible();
}

onMounted(async () => {
  await refreshVisible();
  // 状态一致性：窗口实际显示时，开关必须同步为开启——
  // 避免"灵动岛已经显示，但启动开关显示关闭"的不一致（历史操作可能脱节）。
  // 反向（开关开但窗口隐藏）保留用户偏好，不强制关闭。
  if (visible.value && !islandEnabled.value) {
    setIslandEnabled(true);
  }
  // 从 Rust（磁盘持久化）恢复 key：localStorage 可能被清/隔离，
  // 以磁盘文件为唯一权威来源，重启后输入框与余额展示都正常
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    const diskKey = await invoke("deepseek_get_key");
    if (diskKey && !deepSeekKey.value) {
      deepSeekKey.value = diskKey;
      keySaved.value = true;
    }
  } catch {
    // 非 Tauri 环境忽略
  }
  // 已有 key 时，打开页面即显示"已保存"
  if (deepSeekKey.value && deepSeekKey.value.trim()) {
    keySaved.value = true;
  }
});

const behaviors = [
  { key: "click", icon: "check-circle" },
  { key: "drag", icon: "arrow-up" },
];

// ---- DeepSeek 余额查询 ----
const balanceLoading = ref(false);
const balanceResult = ref(null); // { isAvailable, balanceInfos[] }
const balanceError = ref("");

async function checkBalance() {
  if (!deepSeekKey.value.trim()) {
    balanceError.value = t("island.ds_key_empty");
    balanceResult.value = null;
    return;
  }
  // 先确保 key 已写入 Rust store（watch 输入即保存，这里兜底同步一次）
  setDeepSeekKey(deepSeekKey.value);
  balanceLoading.value = true;
  balanceError.value = "";
  balanceResult.value = null;
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    // key 由 Rust 侧从 store 读取，前端不传参
    balanceResult.value = await invoke("deepseek_get_balance");
  } catch (e) {
    balanceError.value = String(e);
  } finally {
    balanceLoading.value = false;
  }
}
</script>

<template>
  <div class="page island-page">
    <!-- Header -->
    <div class="breadcrumb">
      <Button variant="ghost" size="sm" class="back-btn" @click="router.push('/dashboard')">
        <AppIcon name="left" class="size-4" />
        {{ t("nav.dashboard") }}
      </Button>
      <span class="crumb-sep">/</span>
      <span class="crumb-title">{{ t("island.title") }}</span>
    </div>

    <div class="island-content space-y-3">
      <!-- 启用开关 -->
      <Card class="section-card shadow-sm">
        <CardHeader class="flex-row items-center justify-between !py-4">
          <CardTitle class="text-base font-medium">{{ t("island.title") }}</CardTitle>
          <Badge :variant="statusVariant">{{ statusText }}</Badge>
        </CardHeader>
        <CardContent class="space-y-3">
          <div class="setting-row">
            <div>
              <div class="setting-label">{{ t("island.enable") }}</div>
              <div class="setting-desc">{{ t("island.enable_desc") }}</div>
            </div>
            <Switch :checked="islandEnabled" @update:model-value="onToggle" />
          </div>

          <!-- 状态反馈：由启用开关统一控制显示/隐藏（不再提供单独按钮） -->
          <div class="flex items-center gap-2">
            <Badge :variant="visible ? 'default' : 'secondary'">
              {{ visibleText }}
            </Badge>
            <span class="text-xs text-muted-foreground">{{ t("island.state_desc") }}</span>
          </div>
        </CardContent>
      </Card>

      <!-- DeepSeek 余额：API Key 输入 + 查询 -->
      <Card class="section-card shadow-sm overflow-hidden">
        <CardHeader class="flex-row items-center justify-between !py-4">
          <div class="flex items-center gap-3">
            <div class="ds-brand">
              <img :src="deepseekIcon" alt="DeepSeek" class="size-9 rounded-full object-cover ring-2 ring-primary/20" />
            </div>
            <div>
              <CardTitle class="text-base font-medium">{{ t("island.ds_title") }}</CardTitle>
              <p class="text-xs text-muted-foreground mt-0.5">DeepSeek API · 实时余额</p>
            </div>
          </div>
          <Badge v-if="balanceResult" :variant="balanceResult.isAvailable ? 'default' : 'secondary'">
            {{ balanceResult.isAvailable ? t("island.ds_available") : t("island.ds_unavailable") }}
          </Badge>
        </CardHeader>
        <CardContent class="space-y-3">
          <div class="setting-row">
            <div>
              <div class="setting-label">{{ t("island.ds_key_label") }}</div>
              <div class="setting-desc">{{ t("island.ds_key_desc") }}</div>
            </div>
          </div>
          <div class="flex flex-wrap items-center gap-2">
            <div class="relative flex-1 min-w-[220px]">
              <Input
                v-model="deepSeekKey"
                :type="showKey ? 'text' : 'password'"
                :placeholder="t('island.ds_key_placeholder')"
                class="w-full pr-10"
                @keyup.enter="checkBalance"
              />
              <button
                type="button"
                class="ds-eye"
                :aria-label="showKey ? t('island.ds_hide_key') : t('island.ds_show_key')"
                :title="showKey ? t('island.ds_hide_key') : t('island.ds_show_key')"
                @click="showKey = !showKey"
              >
                <AppIcon :name="showKey ? 'eye-off' : 'eye'" class="size-4" />
              </button>
            </div>
            <Button :loading="balanceLoading" :disabled="balanceLoading" @click="checkBalance">
              {{ t("island.ds_check") }}
            </Button>
          </div>
          <div v-if="keySaved && deepSeekKey" class="ds-saved">
            <AppIcon name="check" class="behavior-icon" />
            {{ t("island.ds_saved") }}
          </div>

          <!-- 查询结果 -->
          <div v-if="balanceResult" class="ds-result">
            <div v-for="info in balanceResult.balanceInfos" :key="info.currency" class="ds-line">
              <span class="ds-currency-chip">{{ info.currency }}</span>
              <span class="ds-total">{{ info.totalBalance }}</span>
              <span class="ds-sub">
                {{ t("island.ds_granted") }} {{ info.grantedBalance }} ·
                {{ t("island.ds_topped") }} {{ info.toppedUpBalance }}
              </span>
            </div>
            <div v-if="balanceResult.balanceInfos.length === 0" class="ds-empty">
              {{ t("island.ds_key_empty") }}
            </div>
          </div>
          <div v-else-if="balanceError" class="ds-error">{{ balanceError }}</div>
          <div v-else class="ds-hint">
            <AppIcon name="key" class="behavior-icon" />
            {{ t("island.ds_hint") }}
          </div>
        </CardContent>
      </Card>

      <!-- 胶囊预览 -->
      <Card class="section-card shadow-sm">
        <CardHeader>
          <CardTitle class="text-base font-medium">{{ t("island.preview") }}</CardTitle>
        </CardHeader>
        <CardContent>
          <div class="preview-stage">
            <div class="preview-capsule">
              <span v-if="islandEnabled" class="preview-time">10:24:36</span>
              <span v-else class="preview-off">{{ t("island.preview_off") }}</span>
            </div>
          </div>
        </CardContent>
      </Card>

      <!-- 行为说明 -->
      <Card class="section-card shadow-sm">
        <CardHeader>
          <CardTitle class="text-base font-medium">{{ t("island.behaviors") }}</CardTitle>
        </CardHeader>
        <CardContent class="space-y-3">
          <div v-for="b in behaviors" :key="b.key" class="setting-row">
            <div class="flex items-start gap-3">
              <AppIcon :name="b.icon" class="behavior-icon" />
              <div>
                <div class="setting-label">{{ t(`island.behavior_${b.key}`) }}</div>
                <div class="setting-desc">{{ t(`island.behavior_${b.key}_desc`) }}</div>
              </div>
            </div>
          </div>
        </CardContent>
      </Card>

      <!-- 非桌面环境提示 -->
      <Alert v-if="!isTauri" variant="default">
        <AlertTitle>{{ t("island.not_tauri") }}</AlertTitle>
        <AlertDescription>{{ t("island.not_tauri_desc") }}</AlertDescription>
      </Alert>
    </div>
  </div>
</template>

<style scoped>
.breadcrumb {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
}

.crumb-sep {
  color: var(--color-muted-foreground);
}

.crumb-title {
  font-size: 13px;
  color: var(--color-muted-foreground);
}

.island-content {
  max-width: 720px;
}

.preview-stage {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 96px;
  border-radius: var(--nx-radius-6, 12px);
  background: var(--color-muted, rgba(128, 128, 128, 0.1));
  overflow: hidden;
}

.preview-capsule {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 196px;
  height: 34px;
  border-radius: 17px;
  background: #000;
  box-shadow:
    0 8px 24px rgba(0, 0, 0, 0.4),
    inset 0 0 0 1px rgba(255, 255, 255, 0.08);
  transition:
    width 0.38s ease,
    height 0.38s ease,
    border-radius 0.38s ease;
}

/* 悬停预览：模拟点击展开——胶囊变长变高，仍是药丸形 */
.preview-capsule:hover {
  width: 320px;
  height: 60px;
  border-radius: 30px;
}

.preview-time {
  color: #fff;
  font-size: 13px;
  font-weight: 600;
  letter-spacing: 0.08em;
  font-variant-numeric: tabular-nums;
  font-family: "JetBrains Mono", ui-monospace, monospace;
}

.preview-off {
  color: rgba(255, 255, 255, 0.45);
  font-size: 12px;
}

.behavior-icon {
  width: 16px;
  height: 16px;
  margin-top: 2px;
  color: var(--color-primary);
}

/* ── DeepSeek 余额结果 ── */
.ds-brand {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 44px;
  height: 44px;
  border-radius: 12px;
  background: linear-gradient(135deg, rgba(78, 138, 254, 0.16), rgba(56, 189, 248, 0.08));
}

.ds-result {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px 14px;
  border-radius: var(--nx-radius-6, 12px);
  background: linear-gradient(135deg, var(--color-muted, rgba(128, 128, 128, 0.1)), transparent);
  border: 1px solid var(--color-border, rgba(128, 128, 128, 0.15));
}

.ds-available {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  font-weight: 600;
}

.ds-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.ds-dot.ok {
  background: #22c55e;
  box-shadow: 0 0 6px rgba(34, 197, 94, 0.6);
}

.ds-dot.bad {
  background: #ef4444;
  box-shadow: 0 0 6px rgba(239, 68, 68, 0.6);
}

.ds-line {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 13px;
}

.ds-currency-chip {
  font-size: 11px;
  font-weight: 700;
  padding: 2px 8px;
  border-radius: var(--nx-radius-round, 9999px);
  background: var(--color-primary, #4e8afe);
  color: #fff;
  min-width: 44px;
  text-align: center;
}

.ds-total {
  font-size: 16px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}

.ds-sub {
  font-size: 12px;
  color: var(--color-muted-foreground);
  margin-left: auto;
  white-space: nowrap;
}

.ds-hint {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--color-muted-foreground);
  padding: 8px 2px;
}

.ds-saved {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: #22c55e;
}

/* key 显示/隐藏切换按钮（输入框右侧眼睛） */
.ds-eye {
  position: absolute;
  right: 8px;
  top: 50%;
  transform: translateY(-50%);
  display: flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--color-muted-foreground);
  cursor: pointer;
  transition:
    background-color 0.15s ease,
    color 0.15s ease;
}

.ds-eye:hover {
  background: var(--color-muted);
  color: var(--color-foreground);
}

.ds-empty {
  font-size: 12px;
  color: var(--color-muted-foreground);
}

.ds-error {
  font-size: 12px;
  color: #ef4444;
  word-break: break-all;
}
</style>
<script setup>
import { ref, computed, onMounted, onBeforeUnmount, nextTick } from "vue";
import { open as openDialog, save } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import {
  listConnections,
  openSftp,
  closeSftp,
  listSftpDir,
  readSftpFile,
  writeSftpFile,
  mkdirSftp,
  mkdirLocal,
  listLocalDir,
  readLocalFileChunk,
  renameSftp,
  deleteSftp,
  statSftp,
  chmodSftp,
  copyRecursiveSftp,
  rmRecursiveSftp,
  searchSftp,
  touchConnection,
  onHostkeyPrompt,
  acceptHostkey,
  rejectHostkey,
  aiSftp,
  aiSftpModels,
} from "../lib/api-ssh.js";
import { showToast } from "../lib/toast.js";
import { showConfirm } from "../lib/confirm.js";
import { t, tFormat } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";
import AppIcon from "../components/AppIcon.vue";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Spinner } from "@/components/ui/spinner";
import { Progress } from "@/components/ui/progress";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyMedia,
} from "@/components/ui/empty";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

const conns = ref([]);
const connId = ref("");
const sftpId = ref(null);
const connecting = ref(false);
const loadingDir = ref(false);
const cwd = ref("/");
const entries = ref([]);

// ── 传输队列 ──
// 所有上传/下载顺序执行；面板按任务独立展示进度，完成的任务几秒后自动消失。
const transfers = ref([]); // { id, kind: 'up'|'down', name, done, total, status: 'active'|'done'|'error' }
const CHUNK = 256 * 1024;
let transferSeq = 0;
let opChain = Promise.resolve(); // 串行化：避免并发 SFTP 传输争抢带宽/乱序写
const busy = computed(() => transfers.value.some((t) => t.status === "active"));

/** 加入传输队列（顺序执行）。返回响应式任务对象，op(item) 内更新 item.done */
function enqueueTransfer(kind, name, total, op) {
  const item = { id: ++transferSeq, kind, name, done: 0, total, status: "active" };
  transfers.value.push(item);
  const run = async () => {
    try {
      await op(item);
      item.status = "done";
      setTimeout(() => {
        transfers.value = transfers.value.filter((t) => t.id !== item.id);
      }, 4000);
    } catch (err) {
      item.status = "error";
      showToast(friendlyError(err), "error");
    }
  };
  opChain = opChain.then(run).catch(() => {});
  return item;
}

// ── 本地侧（双栏模式）──
const dualPane = ref(localStorage.getItem("ssh-sftp-dual") !== "0");
const localCwd = ref("");
const localEntries = ref([]);
const localLoading = ref(false);

// 输入对话框（新建文件夹 / 重命名）：{ title, value, onOk }
const prompt = ref(null);
const promptValue = ref("");

// host key 首连确认
const hostkeyPrompt = ref(null);
let unlistenHostkey = null;

// ── SFTP AI 助手 ──
const aiOpen = ref(true);
const aiModels = ref([]);
const aiModel = ref("");
const aiBusy = ref(false);
const aiMessages = ref([]); // { role, content, actions? }
const aiInput = ref("");

async function loadAiModels() {
  try {
    const models = await aiSftpModels();
    aiModels.value = models || [];
    if (!aiModel.value && aiModels.value.length) aiModel.value = aiModels.value[0].model;
  } catch {
    // 无 Provider 时静默，AI 面板发送时会提示
  }
}

async function sendAi() {
  const text = aiInput.value.trim();
  if (!text || aiBusy.value || !sftpId.value) return;
  if (!aiModels.value.length) {
    showToast(t("ssh.ai.noProvider"), "error");
    return;
  }
  aiBusy.value = true;
  aiInput.value = "";
  aiMessages.value.push({ role: "user", content: text });
  await nextTick();
  scrollAi();

  const history = aiMessages.value
    .filter((m) => m.role === "user" || m.role === "assistant")
    .map((m) => ({ role: m.role, content: m.content }));

  try {
    const res = await aiSftp({
      sftpId: sftpId.value,
      cwd: cwd.value,
      listing: entries.value,
      history,
      message: text,
      model: aiModel.value || null,
    });
    aiMessages.value.push({
      role: "assistant",
      content: res.reply,
      actions: res.actions || [],
    });
  } catch (err) {
    aiMessages.value.push({ role: "assistant", content: `⚠️ ${friendlyError(err)}`, actions: [] });
  } finally {
    aiBusy.value = false;
    await nextTick();
    scrollAi();
  }
}

function scrollAi() {
  const box = document.querySelector(".sftp-ai-messages");
  if (box) box.scrollTop = box.scrollHeight;
}

function isValidRemotePath(p) {
  return typeof p === 'string' && p.startsWith('/') && !p.includes('..') && p.length <= 4096 && !/[\0\n\r]/.test(p);
}

async function runAiAction(action) {
  if (!sftpId.value) return;
  // 前端二次校验：后端已过滤非法动作，这里再做一次防御
  const validActions = new Set(['navigate', 'rename', 'delete', 'open']);
  if (!validActions.has(action.action)) {
    showToast(`Unsupported action: ${action.action}`, 'error');
    return;
  }
  try {
    if (action.action === "navigate" && action.path) {
      if (!isValidRemotePath(action.path)) {
        showToast(t("ssh.invalid_path"), "error");
        return;
      }
      await cd(action.path);
    } else if (action.action === "rename" && action.from && action.to) {
      if (!isValidRemotePath(action.from) || !isValidRemotePath(action.to)) {
        showToast(t("ssh.invalid_path"), "error");
        return;
      }
      if (await showConfirm(tFormat("ssh.rename_confirm", { from: action.from, to: action.to, name: action.from }))) {
        await renameSftp(sftpId.value, action.from, action.to);
        await refresh();
      }
    } else if (action.action === "delete" && action.path) {
      if (!isValidRemotePath(action.path)) {
        showToast(t("ssh.invalid_path"), "error");
        return;
      }
      // 完整路径展示，防止 basename 误导（如删除 /）
      if (await showConfirm(tFormat("ssh.delete_confirm", { name: action.path }))) {
        await deleteSftp(sftpId.value, action.path, !!action.is_dir);
        await refresh();
      }
    } else if (action.action === "open" && action.path) {
      // 尝试下载该文件（复用下载队列）
      const name = action.path.split("/").pop() || action.path;
      enqueueTransfer("down", name, 0, async (item) => {
        let local;
        try {
          local = await save({ defaultPath: name });
        } catch {
          return;
        }
        if (!local) return;
        let offset = 0;
        while (true) {
          const b64 = await readSftpFile(sftpId.value, action.path, offset, CHUNK);
          const bytes = b64ToBytes(b64);
          if (bytes.length === 0) break;
          await invoke("sftp_write_local_chunk", {
            path: local,
            dataB64: bytesToBase64(bytes),
            append: offset > 0,
          });
          offset += bytes.length;
          item.done = offset;
          item.total = Math.max(item.total, offset);
        }
        showToast(t("ssh.download") + " ✓ " + name, "success");
      });
    }
  } catch (err) {
    showToast(friendlyError(err), "error");
  }
}

const sortedEntries = computed(() =>
  [...entries.value].sort((a, b) => {
    if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
    return a.name.localeCompare(b.name);
  })
);
const crumbs = computed(() => {
  const parts = cwd.value.split("/").filter(Boolean);
  return [{ name: "/", path: "/" }].concat(
    parts.map((p, i) => ({ name: p, path: "/" + parts.slice(0, i + 1).join("/") }))
  );
});

// 二进制安全的 base64（勿用 TextEncoder 编码字节流）
function bytesToBase64(bytes) {
  let bin = "";
  for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
  return btoa(bin);
}

function b64ToBytes(b64) {
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return bytes;
}

function join(base, name) {
  return (base === "/" ? "" : base) + "/" + name;
}

function parentOf(p) {
  if (p === "/") return "/";
  const idx = p.lastIndexOf("/");
  return idx <= 0 ? "/" : p.slice(0, idx);
}

function humanSize(n) {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

function fmtTime(mtime) {
  if (!mtime) return "-";
  return new Date(mtime * 1000).toLocaleString();
}

async function connect() {
  if (!connId.value || connecting.value) return;
  connecting.value = true;
  try {
    sftpId.value = await openSftp(connId.value);
    await cd("/");
    touchConnection(connId.value).catch(() => {}); // 记录最近使用时间
    if (dualPane.value && !localCwd.value) openLocalHome();
  } catch (err) {
    sftpId.value = null;
    showToast(friendlyError(err), "error");
  } finally {
    connecting.value = false;
  }
}

async function disconnect() {
  // 主动关闭后端 SFTP 通道（仅 SFTP，不影响同连接的终端会话）
  if (sftpId.value) {
    closeSftp(sftpId.value).catch(() => {});
  }
  sftpId.value = null;
  entries.value = [];
  cwd.value = "/";
}

async function cd(p) {
  loadingDir.value = true;
  try {
    entries.value = await listSftpDir(sftpId.value, p);
    cwd.value = p;
  } catch (err) {
    showToast(friendlyError(err), "error");
  } finally {
    loadingDir.value = false;
  }
}

function enter(entry) {
  if (entry.is_dir && !busy.value) cd(join(cwd.value, entry.name));
}

function goUp() {
  if (!busy.value) cd(parentOf(cwd.value));
}

async function refresh() {
  if (sftpId.value && !busy.value) await cd(cwd.value);
}

function download(entry) {
  if (busy.value || entry.is_dir || !sftpId.value) return;
  const remote = join(cwd.value, entry.name);
  enqueueTransfer("down", entry.name, entry.size, async (item) => {
    let local;
    try {
      local = await save({ defaultPath: entry.name });
    } catch {
      return; // 对话框失败/取消
    }
    if (!local) return;

    let offset = 0;
    while (offset < item.total) {
      const b64 = await readSftpFile(sftpId.value, remote, offset, CHUNK);
      const bytes = b64ToBytes(b64);
      if (bytes.length === 0) break; // 提前 EOF（文件被截断）
      await invoke("sftp_write_local_chunk", {
        path: local,
        dataB64: bytesToBase64(bytes),
        append: offset > 0,
      });
      offset += bytes.length;
      item.done = offset;
    }
    showToast(t("ssh.download") + " ✓ " + entry.name, "success");
  });
}

// ── 目录递归下载（远端 → 本地选目录）──
// 先遍历远端目录树收集文件清单，再逐个分块下载到本地并还原目录结构。
// 进度按累计字节数展示（total = 清单内所有文件大小之和）。
async function walkRemote(dir, acc) {
  const list = await listSftpDir(sftpId.value, dir);
  for (const e of list) {
    if (e.name === "." || e.name === "..") continue;
    const remote = join(dir, e.name);
    if (e.is_dir) {
      await walkRemote(remote, acc);
    } else {
      acc.push({ remote, size: e.size });
    }
  }
}

function downloadDir(entry) {
  if (busy.value || !sftpId.value) return;
  enqueueTransfer("down", entry.name + "/", 0, async (item) => {
    let localRoot;
    try {
      localRoot = await openDialog({ directory: true, title: t("ssh.download_folder") });
    } catch {
      return; // 对话框失败/取消
    }
    if (!localRoot) return;

    const root = join(cwd.value, entry.name);
    const base = localRoot.endsWith("/") ? localRoot.slice(0, -1) : localRoot;
    const localDir = `${base}/${entry.name}`;
    const files = [];
    await walkRemote(root, files);
    item.total = files.reduce((s, f) => s + f.size, 0);
    await mkdirLocal(localDir); // 远端根目录本身也建一份，保持所选名字一致
    for (const f of files) {
      // 相对远端根的路径 → 本地目标路径（POSIX 风格两端一致）
      const rel = f.remote.slice(root.length + 1);
      const localPath = `${localDir}/${rel}`;
      const parent = localPath.slice(0, localPath.lastIndexOf("/"));
      await mkdirLocal(parent);
      let offset = 0;
      while (offset < f.size) {
        const b64 = await readSftpFile(sftpId.value, f.remote, offset, CHUNK);
        const bytes = b64ToBytes(b64);
        if (bytes.length === 0) break; // 提前 EOF
        await invoke("sftp_write_local_chunk", {
          path: localPath,
          dataB64: bytesToBase64(bytes),
          append: offset > 0,
        });
        offset += bytes.length;
        item.done += bytes.length;
      }
    }
    showToast(t("ssh.download") + " ✓ " + entry.name, "success");
  });
}

// ── 本地侧导航（双栏模式）──
async function cdLocal(p) {
  localLoading.value = true;
  try {
    localEntries.value = await listLocalDir(p);
    localCwd.value = p;
  } catch (err) {
    showToast(friendlyError(err), "error");
  } finally {
    localLoading.value = false;
  }
}
function goUpLocal() {
  if (localCwd.value && localCwd.value !== "/") cdLocal(parentOf(localCwd.value));
}
function refreshLocal() {
  if (localCwd.value) cdLocal(localCwd.value);
}
function enterLocal(e) {
  if (e.is_dir && localCwd.value) cdLocal(join(localCwd.value, e.name));
}
function openLocalHome() {
  // 初始打开本机主目录（不可用时回退根目录）
  import("@tauri-apps/api/path")
    .then((m) => m.homeDir())
    .then((h) => cdLocal(h || "/"))
    .catch(() => cdLocal("/"));
}
function toggleDualPane() {
  dualPane.value = !dualPane.value;
  localStorage.setItem("ssh-sftp-dual", dualPane.value ? "1" : "0");
  if (dualPane.value && !localCwd.value) openLocalHome();
}
const localCrumbs = computed(() => {
  if (!localCwd.value) return [];
  const parts = localCwd.value.split("/").filter(Boolean);
  const out = [{ name: "/", path: "/" }];
  let acc = "";
  for (const p of parts) {
    acc += "/" + p;
    out.push({ name: p, path: acc });
  }
  return out;
});
const localSorted = computed(() =>
  [...localEntries.value].sort((a, b) => {
    if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
    return a.name.localeCompare(b.name);
  })
);

// ── 跨栏拖拽（远端行 ⇄ 本地栏）──
function onRemoteDragStart(ev, e) {
  ev.dataTransfer.setData(
    "text/ssh-remote",
    JSON.stringify({ path: join(cwd.value, e.name), is_dir: e.is_dir, name: e.name, size: e.size })
  );
  ev.dataTransfer.effectAllowed = "copy";
}
function onLocalDragStart(ev, e) {
  ev.dataTransfer.setData(
    "text/ssh-local",
    JSON.stringify({ path: join(localCwd.value, e.name), is_dir: e.is_dir, name: e.name, size: e.size })
  );
  ev.dataTransfer.effectAllowed = "copy";
}
function onDropToLocal(e) {
  const data = e.dataTransfer?.getData("text/ssh-remote");
  if (!data || !localCwd.value) return;
  try {
    downloadTo(JSON.parse(data), localCwd.value);
  } catch {
    // 非法数据忽略
  }
}

/** 下载远端条目到指定本地目录（双栏拖拽用，无对话框）。目录递归还原结构。 */
function downloadTo(info, localDir) {
  if (busy.value || !sftpId.value) return;
  if (info.is_dir) {
    enqueueTransfer("down", info.name + "/", 0, async (item) => {
      const root = info.path;
      const base = localDir.endsWith("/") ? localDir.slice(0, -1) : localDir;
      const localD = `${base}/${info.name}`;
      const files = [];
      const walk = async (dir) => {
        const list = await listSftpDir(sftpId.value, dir);
        for (const e2 of list) {
          if (e2.name === "." || e2.name === "..") continue;
          const rp = join(dir, e2.name);
          if (e2.is_dir) await walk(rp);
          else files.push({ remote: rp, size: e2.size });
        }
      };
      await walk(root);
      item.total = files.reduce((s, f) => s + f.size, 0);
      await mkdirLocal(localD);
      for (const f of files) {
        const rel = f.remote.slice(root.length + 1);
        const localPath = `${localD}/${rel}`;
        const parent = localPath.slice(0, localPath.lastIndexOf("/"));
        await mkdirLocal(parent);
        let offset = 0;
        while (offset < f.size) {
          const b64 = await readSftpFile(sftpId.value, f.remote, offset, CHUNK);
          const bytes = b64ToBytes(b64);
          if (bytes.length === 0) break;
          await invoke("sftp_write_local_chunk", {
            path: localPath,
            dataB64: bytesToBase64(bytes),
            append: offset > 0,
          });
          offset += bytes.length;
          item.done += bytes.length;
        }
      }
      showToast(t("ssh.download") + " ✓ " + info.name, "success");
    });
    return;
  }
  enqueueTransfer("down", info.name, info.size || 0, async (item) => {
    const base = localDir.endsWith("/") ? localDir.slice(0, -1) : localDir;
    const localPath = `${base}/${info.name}`;
    let offset = 0;
    while (true) {
      const b64 = await readSftpFile(sftpId.value, info.path, offset, CHUNK);
      const bytes = b64ToBytes(b64);
      if (bytes.length === 0) break;
      await invoke("sftp_write_local_chunk", {
        path: localPath,
        dataB64: bytesToBase64(bytes),
        append: offset > 0,
      });
      offset += bytes.length;
      item.done = offset;
      item.total = Math.max(item.total, offset);
    }
    refreshLocal();
    showToast(t("ssh.download") + " ✓ " + info.name, "success");
  });
}

function upload(file) {
  if (busy.value) return;
  const total = file.size;
  enqueueTransfer("up", file.name, total, async (item) => {
    const data = new Uint8Array(await file.arrayBuffer());
    const remote = join(cwd.value, file.name);
    let offset = 0;
    while (offset < total) {
      const chunk = data.subarray(offset, offset + CHUNK);
      await writeSftpFile(sftpId.value, remote, bytesToBase64(chunk), offset);
      offset += chunk.length;
      item.done = offset;
    }
    showToast(t("ssh.upload") + " ✓ " + file.name, "success");
    await refresh();
  });
}

/** 本地路径 → 远端当前目录（双栏模式：本地行拖到远程栏 / 双击上传按钮）。
 *  文件分块读取上传；目录先递归收集清单，远端 mkdir + 逐文件写入。 */
function uploadFromLocal(localPath, isDir, name) {
  if (busy.value || !sftpId.value) return;
  const remoteRoot = join(cwd.value, name);
  enqueueTransfer("up", name, 0, async (item) => {
    const files = [];
    if (isDir) {
      const walk = async (dir) => {
        const list = await listLocalDir(dir);
        for (const e of list) {
          const p = `${dir.endsWith("/") ? dir.slice(0, -1) : dir}/${e.name}`;
          if (e.is_dir) await walk(p);
          else files.push({ path: p, size: e.size });
        }
      };
      await walk(localPath);
      item.total = files.reduce((s, f) => s + f.size, 0);
      await mkdirSftp(sftpId.value, remoteRoot);
      for (const f of files) {
        const rel = f.path.slice(localPath.length + 1);
        const remotePath = `${remoteRoot}/${rel}`;
        const parent = remotePath.slice(0, remotePath.lastIndexOf("/"));
        await mkdirSftp(sftpId.value, parent);
        let offset = 0;
        while (offset < f.size) {
          const b64 = await readLocalFileChunk(f.path, offset, CHUNK);
          const bytes = b64ToBytes(b64);
          if (bytes.length === 0) break;
          await writeSftpFile(sftpId.value, remotePath, bytesToBase64(bytes), offset);
          offset += bytes.length;
          item.done += bytes.length;
        }
      }
    } else {
      const st = await statSftp(sftpId.value, remoteRoot).catch(() => null);
      item.total = st ? st.size : 0;
      let offset = 0;
      while (true) {
        const b64 = await readLocalFileChunk(localPath, offset, CHUNK);
        const bytes = b64ToBytes(b64);
        if (bytes.length === 0) break;
        await writeSftpFile(sftpId.value, remoteRoot, bytesToBase64(bytes), offset);
        offset += bytes.length;
        item.done = offset;
        item.total = Math.max(item.total, offset);
      }
    }
    showToast(t("ssh.upload") + " ✓ " + name, "success");
    await refresh();
  });
}

function onDrop(e) {
  if (!sftpId.value) return;
  // 1) 本地侧（双栏）拖入的条目 → 上传到远端当前目录
  const localData = e.dataTransfer?.getData("text/ssh-local");
  if (localData) {
    try {
      const info = JSON.parse(localData);
      uploadFromLocal(info.path, !!info.is_dir, info.name);
    } catch {
      // 非法数据忽略
    }
    return;
  }
  // 2) 操作系统拖入的文件 → 逐个入队上传
  const files = [...(e.dataTransfer?.files || [])];
  for (const f of files) upload(f);
}

function onPickFiles(e) {
  const files = [...(e.target.files || [])];
  for (const f of files) upload(f);
  e.target.value = "";
}

function openMkdir() {
  promptValue.value = "";
  prompt.value = {
    title: t("ssh.new_folder"),
    onOk: async () => {
      const name = promptValue.value.trim();
      if (!name) return false;
      await mkdirSftp(sftpId.value, join(cwd.value, name));
      await refresh();
      return true;
    },
  };
}

function openRename(entry) {
  promptValue.value = entry.name;
  prompt.value = {
    title: t("ssh.rename"),
    onOk: async () => {
      const name = promptValue.value.trim();
      if (!name || name === entry.name) return true;
      await renameSftp(sftpId.value, join(cwd.value, entry.name), join(cwd.value, name));
      await refresh();
      return true;
    },
  };
}

async function onDelete(entry) {
  if (!(await showConfirm(tFormat("ssh.delete_confirm", { name: entry.name })))) return;
  try {
    await deleteSftp(sftpId.value, join(cwd.value, entry.name), entry.is_dir);
    await refresh();
  } catch (err) {
    showToast(friendlyError(err), "error");
  }
}

// ── 权限编辑（chmod）──
const chmod = ref(null); // { path, modeStr } 对话框
function openChmod(entry) {
  chmod.value = {
    path: join(cwd.value, entry.name),
    name: entry.name,
    modeStr: (entry.mode & 0o7777).toString(8).padStart(3, "0"),
  };
}
async function onChmodOk() {
  if (!chmod.value) return;
  const mode = parseInt(chmod.value.modeStr, 8);
  if (Number.isNaN(mode)) {
    showToast(t("ssh.chmod_invalid"), "error");
    return;
  }
  try {
    await chmodSftp(sftpId.value, chmod.value.path, mode);
    chmod.value = null;
    showToast(t("ssh.chmod_done"), "success");
    await refresh();
  } catch (err) {
    showToast(friendlyError(err), "error");
  }
}

// ── 文件搜索（远端 find）──
const searchDialog = ref(null); // { pattern, maxDepth }
const searchResults = ref([]);
const searchBusy = ref(false);
function openSearch() {
  searchDialog.value = { pattern: "", maxDepth: null };
  searchResults.value = [];
}
async function onSearchOk() {
  if (!searchDialog.value) return;
  const pattern = searchDialog.value.pattern.trim();
  if (!pattern) {
    showToast(t("ssh.search_empty"), "error");
    return;
  }
  searchBusy.value = true;
  searchResults.value = [];
  try {
    const res = await searchSftp(sftpId.value, cwd.value, pattern, searchDialog.value.maxDepth || null);
    searchResults.value = res || [];
    if (searchResults.value.length === 0) showToast(t("ssh.search_no_result"), "info");
  } catch (err) {
    showToast(friendlyError(err), "error");
  } finally {
    searchBusy.value = false;
  }
}
function gotoResult(p) {
  const parent = parentOf(p);
  searchDialog.value = null;
  cd(parent).then(() => {
    const name = p.split("/").pop();
    showToast(t("ssh.search_in") + " " + p, "success");
    void name;
  });
}

// ── 复制/移动 → 目标目录（dialog）──
const moveDialog = ref(null); // { entry, action: 'copy'|'move', dest }
function openCopy(entry, action) {
  moveDialog.value = {
    entry,
    action,
    src: join(cwd.value, entry.name),
    dest: cwd.value,
  };
}
async function onMoveOk() {
  const d = moveDialog.value;
  if (!d) return;
  try {
    const destDir = d.dest.trim() || "/";
    const target = join(destDir, d.entry.name);
    if (d.action === "copy") {
      await copyRecursiveSftp(sftpId.value, d.src, target, false);
      showToast(t("ssh.copy_done") + " ✓ " + d.entry.name, "success");
    } else {
      // 移动 = 复制 + 删除源
      await copyRecursiveSftp(sftpId.value, d.src, target, false);
      if (d.entry.is_dir) await rmRecursiveSftp(sftpId.value, d.src);
      else await deleteSftp(sftpId.value, d.src, false);
      showToast(t("ssh.move_done") + " ✓ " + d.entry.name, "success");
    }
    moveDialog.value = null;
    await refresh();
  } catch (err) {
    showToast(friendlyError(err), "error");
  }
}

async function onPromptOk() {
  const p = prompt.value;
  if (!p) return;
  try {
    const ok = await p.onOk();
    if (ok !== false) prompt.value = null;
  } catch (err) {
    showToast(friendlyError(err), "error");
  }
}

async function onHostkeyAccept() {
  const p = hostkeyPrompt.value;
  hostkeyPrompt.value = null;
  try {
    await acceptHostkey(p.sessionId, p.host, p.fingerprint);
  } catch (err) {
    showToast(friendlyError(err), "error");
  }
}

async function onHostkeyReject() {
  const p = hostkeyPrompt.value;
  hostkeyPrompt.value = null;
  try {
    await rejectHostkey(p.sessionId);
  } catch (err) {
    showToast(friendlyError(err), "error");
  }
}

function transferPercent(item) {
  return item && item.total > 0 ? Math.round((item.done / item.total) * 100) : 0;
}

onMounted(async () => {
  unlistenHostkey = await onHostkeyPrompt((p) => {
    hostkeyPrompt.value = p;
  });
  loadAiModels();
  try {
    conns.value = await listConnections();
  } catch (err) {
    showToast(friendlyError(err), "error");
  }
});

onBeforeUnmount(() => {
  if (unlistenHostkey) unlistenHostkey();
});
</script>

<template>
  <div class="page sftp-page">
    <!-- Header -->
    <div class="page-header">
      <div>
        <h1 class="page-title">{{ t("ssh.sftp") }}</h1>
        <p class="page-desc">{{ t("nav.ssh") }}</p>
      </div>
      <div class="flex items-center gap-2">
        <Select v-model="connId" :disabled="!!sftpId">
          <SelectTrigger class="w-[200px]">
            <SelectValue :placeholder="t('ssh.connections')" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem v-for="c in conns" :key="c.id" :value="c.id">
              {{ c.name }}
            </SelectItem>
          </SelectContent>
        </Select>
        <Button v-if="!sftpId" :disabled="!connId || connecting" @click="connect">
          <Spinner v-if="connecting" class="size-4" />
          <AppIcon v-else name="play-arrow" class="size-4" />
          {{ t("ssh.connect") }}
        </Button>
        <Button v-else variant="outline" @click="disconnect">
          <AppIcon name="close-circle-fill" class="size-4" />
          {{ t("ssh.disconnect") }}
        </Button>
        <Button
          variant="outline"
          :title="t('ssh.dual_pane')"
          :class="{ 'bg-accent': dualPane }"
          :disabled="!sftpId"
          @click="toggleDualPane"
        >
          <AppIcon name="monitor" class="size-4" />
          {{ t("ssh.dual_pane") }}
        </Button>
      </div>
    </div>

    <!-- 未连接 -->
    <Empty v-if="!sftpId" class="py-14">
      <EmptyMedia>
        <AppIcon name="folder" class="size-10 text-muted-foreground/60" />
      </EmptyMedia>
      <EmptyContent>
        <EmptyDescription>
          <div>{{ t("ssh.not_connected") }}</div>
          <div class="empty-hint">{{ t("ssh.sftp_hint") }}</div>
        </EmptyDescription>
      </EmptyContent>
    </Empty>

    <!-- 文件浏览器 -->
    <div v-else class="sftp-ai-layout">
      <!-- 本地侧（双栏模式） -->
      <Card
        v-if="dualPane"
        class="shadow-sm local-pane flex-1 min-w-0"
        @dragover.prevent
        @drop.prevent="onDropToLocal"
      >
        <CardContent class="p-0">
          <div class="sftp-toolbar">
            <span class="local-tag">{{ t("ssh.local_pane") }}</span>
            <div class="crumbs">
              <template v-for="(crumb, i) in localCrumbs" :key="crumb.path">
                <button type="button" class="crumb" @click="cdLocal(crumb.path)">
                  {{ crumb.name === "/" ? " / " : crumb.name }}
                </button>
                <span v-if="i < localCrumbs.length - 1" class="crumb-sep">/</span>
              </template>
            </div>
            <div class="flex items-center gap-1.5">
              <Button size="sm" variant="ghost" :title="t('ssh.up')" @click="goUpLocal">
                <AppIcon name="arrow-up" class="size-4" />
              </Button>
              <Button size="sm" variant="ghost" :title="t('ssh.home_dir')" @click="openLocalHome">
                <AppIcon name="monitor" class="size-4" />
              </Button>
              <Button size="sm" variant="ghost" :title="t('ssh.refresh')" @click="refreshLocal">
                <AppIcon name="refresh" class="size-4" />
              </Button>
            </div>
          </div>

          <div v-if="localLoading" class="flex justify-center py-10">
            <Spinner />
          </div>
          <Empty v-else-if="localEntries.length === 0" class="py-10">
            <EmptyContent>
              <EmptyDescription>{{ t("ssh.empty_dir") }}</EmptyDescription>
            </EmptyContent>
          </Empty>
          <Table v-else>
            <TableHeader>
              <TableRow>
                <TableHead>{{ t("ssh.name") }}</TableHead>
                <TableHead class="w-[110px]">{{ t("ssh.size") }}</TableHead>
                <TableHead class="w-[180px]">{{ t("ssh.modified") }}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow
                v-for="e in localSorted"
                :key="e.name"
                class="entry-row"
                :class="{ dir: e.is_dir }"
                draggable="true"
                @dblclick="enterLocal(e)"
                @dragstart="onLocalDragStart($event, e)"
              >
                <TableCell>
                  <div class="flex items-center gap-2">
                    <AppIcon :name="e.is_dir ? 'folder' : 'file'" class="size-4 opacity-60" />
                    <span>{{ e.name }}</span>
                  </div>
                </TableCell>
                <TableCell class="text-muted-foreground text-xs">
                  {{ e.is_dir ? "-" : humanSize(e.size) }}
                </TableCell>
                <TableCell class="text-muted-foreground text-xs">{{ fmtTime(e.mtime) }}</TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      <!-- 远程侧 -->
      <Card class="shadow-sm drop-zone flex-1 min-w-0" @dragover.prevent @drop.prevent="onDrop">
      <CardContent class="p-0">
        <!-- 工具栏 -->
        <div class="sftp-toolbar">
          <div class="crumbs">
            <template v-for="(crumb, i) in crumbs" :key="crumb.path">
              <button type="button" class="crumb" @click="cd(crumb.path)">
                {{ crumb.name === "/" ? " / " : crumb.name }}
              </button>
              <span v-if="i < crumbs.length - 1" class="crumb-sep">/</span>
            </template>
          </div>
          <div class="flex items-center gap-1.5">
            <Button size="sm" variant="ghost" :title="t('ssh.up')" :disabled="busy" @click="goUp">
              <AppIcon name="arrow-up" class="size-4" />
            </Button>
            <Button size="sm" variant="ghost" :title="t('ssh.refresh')" :disabled="busy" @click="refresh">
              <AppIcon name="refresh" class="size-4" />
            </Button>
            <Button size="sm" variant="ghost" :title="t('ssh.search_files')" :disabled="busy" @click="openSearch">
              <AppIcon name="search" class="size-4" />
            </Button>
            <Button size="sm" variant="ghost" :title="t('ssh.new_folder')" :disabled="busy" @click="openMkdir">
              <AppIcon name="plus" class="size-4" />
            </Button>
            <label class="cursor-pointer">
              <span
                class="inline-flex h-8 items-center gap-1.5 rounded-md px-2.5 text-sm font-medium transition-colors hover:bg-accent"
                :class="{ 'pointer-events-none opacity-50': busy }"
              >
                <AppIcon name="upload" class="size-4" />
              </span>
              <input type="file" multiple class="sr-only" :disabled="busy" @change="onPickFiles" />
            </label>
          </div>
        </div>

        <div v-if="loadingDir" class="flex justify-center py-10">
          <Spinner />
        </div>

        <Empty v-else-if="entries.length === 0" class="py-10">
          <EmptyContent>
            <EmptyDescription>{{ t("ssh.empty_dir") }}</EmptyDescription>
          </EmptyContent>
        </Empty>

        <Table v-else>
          <TableHeader>
            <TableRow>
              <TableHead>{{ t("ssh.name") }}</TableHead>
              <TableHead class="w-[110px]">{{ t("ssh.size") }}</TableHead>
              <TableHead class="w-[180px]">{{ t("ssh.modified") }}</TableHead>
              <TableHead class="w-[130px] text-right">{{ t("common.actions") }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow
              v-for="e in sortedEntries"
              :key="e.name"
              class="entry-row"
              :class="{ dir: e.is_dir }"
              draggable="true"
              @dblclick="enter(e)"
              @dragstart="onRemoteDragStart($event, e)"
            >
              <TableCell>
                <span class="entry-name" @click="enter(e)">
                  <AppIcon :name="e.is_dir ? 'folder' : 'file'" class="size-4 shrink-0" />
                  <span class="truncate">{{ e.name }}</span>
                </span>
              </TableCell>
              <TableCell class="text-muted-foreground">
                {{ e.is_dir ? "-" : humanSize(e.size) }}
              </TableCell>
              <TableCell class="text-muted-foreground">{{ fmtTime(e.mtime) }}</TableCell>
              <TableCell class="text-right">
                <div class="flex items-center justify-end gap-0.5">
                <Button
                  size="icon-sm"
                  variant="ghost"
                  :title="t('ssh.chmod')"
                  :disabled="busy"
                  @click="openChmod(e)"
                >
                  <AppIcon name="shield" class="size-3.5" />
                </Button>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  :title="t('ssh.copy_to')"
                  :disabled="busy"
                  @click="openCopy(e, 'copy')"
                >
                  <AppIcon name="copy" class="size-3.5" />
                </Button>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  :title="t('ssh.move_to')"
                  :disabled="busy"
                  @click="openCopy(e, 'move')"
                >
                  <AppIcon name="move" class="size-3.5" />
                </Button>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  :title="e.is_dir ? t('ssh.download_folder') : t('ssh.download')"
                  :disabled="busy"
                  @click="e.is_dir ? downloadDir(e) : download(e)"
                >
                  <AppIcon name="download" class="size-4" />
                </Button>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  :title="t('ssh.rename')"
                  :disabled="busy"
                  @click="openRename(e)"
                >
                  <AppIcon name="edit" class="size-4" />
                </Button>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  class="text-destructive"
                  :title="t('ssh.delete')"
                  :disabled="busy"
                  @click="onDelete(e)"
                >
                  <AppIcon name="delete" class="size-4" />
                </Button>
                </div>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </CardContent>
    </Card>

    <!-- SFTP AI 助手面板 -->
    <aside class="sftp-ai-panel" :class="{ collapsed: !aiOpen }">
      <div class="sftp-ai-head">
        <div class="sftp-ai-title">
          <AppIcon name="sparkles" class="size-4" />
          <span>{{ t("ssh.ai.title") }}</span>
        </div>
        <button class="sftp-ai-toggle" @click="aiOpen = !aiOpen">
          <AppIcon :name="aiOpen ? 'panel-right-close' : 'panel-right-open'" class="size-4" />
        </button>
      </div>
      <div v-if="aiOpen" class="sftp-ai-body">
        <div class="sftp-ai-models">
          <Select v-model="aiModel">
            <SelectTrigger class="w-full">
              <SelectValue :placeholder="t('ssh.ai.pickModel')" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem v-for="m in aiModels" :key="m.model + m.provider" :value="m.model">
                {{ m.model }} · {{ m.provider }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div class="sftp-ai-messages">
          <div v-if="!aiMessages.length" class="sftp-ai-empty">
            {{ t("ssh.ai.sftpHint") }}
          </div>
          <div v-for="(m, i) in aiMessages" :key="i" class="sftp-ai-msg" :class="m.role">
            <div class="sftp-ai-msg-role">{{ m.role === 'user' ? t('ssh.ai.you') : t('ssh.ai.assistant') }}</div>
            <div class="sftp-ai-msg-text">{{ m.content }}</div>
            <div v-if="m.actions && m.actions.length" class="sftp-ai-actions">
              <div v-for="(act, ai) in m.actions" :key="ai" class="sftp-ai-action">
                <code class="sftp-ai-action-code">{{ act.action === 'rename' ? `${act.action} ${act.from} → ${act.to}` : `${act.action} ${act.path || ''}${act.is_dir ? ' (dir)' : ''}` }}</code>
                <Button size="sm" variant="outline" @click="runAiAction(act)">
                  <AppIcon name="play" class="size-3.5" />
                  {{ t("ssh.ai.run") }}
                </Button>
              </div>
            </div>
          </div>
        </div>
        <div class="sftp-ai-input">
          <Input
            v-model="aiInput"
            :placeholder="t('ssh.ai.inputPlaceholder')"
            @keydown.enter.prevent="sendAi"
          />
          <Button :disabled="aiBusy || !aiInput.trim()" @click="sendAi">
            <Spinner v-if="aiBusy" class="size-3.5" />
            <AppIcon v-else name="send" class="size-4" />
            {{ t("ssh.ai.send") }}
          </Button>
        </div>
      </div>
    </aside>
  </div>

    <!-- 传输队列 -->
    <div v-if="transfers.length" class="transfer-bar">
      <div v-for="item in transfers" :key="item.id" class="transfer-item">
        <AppIcon :name="item.kind === 'up' ? 'upload' : 'download'" class="size-4 shrink-0" />
        <span class="transfer-name">{{ item.name }}</span>
        <span class="transfer-kind">
          {{ item.status === "error"
            ? t("ssh.transfer_failed")
            : item.status === "done"
              ? "✓"
              : item.kind === "up"
                ? t("ssh.uploading")
                : t("ssh.downloading") }}
        </span>
        <Progress :model-value="transferPercent(item)" class="flex-1" />
        <span class="transfer-pct">
          {{ item.status === "error" ? "✗" : transferPercent(item) + "%" }}
        </span>
      </div>
    </div>

    <!-- 新建文件夹 / 重命名 -->
    <Dialog :open="prompt !== null" @update:open="(v) => !v && (prompt = null)">
      <DialogContent class="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>{{ prompt?.title }}</DialogTitle>
        </DialogHeader>
        <div>
          <Label for="sftp-prompt" class="mb-1.5 block">{{ t("ssh.name") }}</Label>
          <Input
            id="sftp-prompt"
            v-model="promptValue"
            @keydown.enter="onPromptOk"
          />
        </div>
        <DialogFooter>
          <Button variant="outline" @click="prompt = null">{{ t("common.cancel") }}</Button>
          <Button @click="onPromptOk">{{ t("common.confirm") }}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <!-- host key 首连确认 -->
    <Dialog :open="hostkeyPrompt !== null" @update:open="(v) => !v && onHostkeyReject()">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{{ t("ssh.hostkey_title") }}</DialogTitle>
        </DialogHeader>
        <p class="text-sm text-muted-foreground break-all">
          {{
            tFormat("ssh.hostkey_body", {
              host: hostkeyPrompt?.host || "",
              fingerprint: hostkeyPrompt?.fingerprint || "",
            })
          }}
        </p>
        <DialogFooter>
          <Button variant="outline" @click="onHostkeyReject">{{ t("ssh.reject") }}</Button>
          <Button @click="onHostkeyAccept">{{ t("ssh.accept") }}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <!-- 权限编辑（chmod） -->
    <Dialog :open="chmod !== null" @update:open="(v) => !v && (chmod = null)">
      <DialogContent class="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>{{ t("ssh.chmod") }} — {{ chmod?.name }}</DialogTitle>
        </DialogHeader>
        <div class="space-y-3">
          <div>
            <Label for="ssh-chmod-mode" class="mb-1.5 block">{{ t("ssh.chmod_mode") }}</Label>
            <div class="flex items-center gap-2">
              <Input id="ssh-chmod-mode" v-model="chmod.modeStr" class="w-24 font-mono" placeholder="755" />
              <span class="text-xs text-muted-foreground">{{ t("ssh.chmod_octal") }}</span>
            </div>
          </div>
          <p class="text-xs text-muted-foreground">
            {{ t("ssh.chmod_hint") }}
          </p>
        </div>
        <DialogFooter>
          <Button variant="outline" @click="chmod = null">{{ t("common.cancel") }}</Button>
          <Button @click="onChmodOk">{{ t("common.confirm") }}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <!-- 复制/移动 到目录 -->
    <Dialog :open="moveDialog !== null" @update:open="(v) => !v && (moveDialog = null)">
      <DialogContent class="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>{{ moveDialog?.action === 'copy' ? t('ssh.copy_to') : t('ssh.move_to') }}</DialogTitle>
        </DialogHeader>
        <div class="space-y-3">
          <p class="text-sm">{{ moveDialog?.entry?.name }}</p>
          <div>
            <Label for="sftp-move-dest" class="mb-1.5 block">{{ t("ssh.dest_path") }}</Label>
            <Input id="sftp-move-dest" v-model="moveDialog.dest" placeholder="/" />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" @click="moveDialog = null">{{ t("common.cancel") }}</Button>
          <Button @click="onMoveOk">{{ t("common.confirm") }}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <!-- 文件搜索 -->
    <Dialog :open="searchDialog !== null" @update:open="(v) => !v && (searchDialog = null)">
      <DialogContent class="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{{ t("ssh.search_files") }}</DialogTitle>
        </DialogHeader>
        <div class="space-y-3">
          <div class="flex items-center gap-2">
            <Input v-model="searchDialog.pattern" :placeholder="t('ssh.search_placeholder')" class="flex-1" @keydown.enter="onSearchOk" />
            <Button :disabled="searchBusy" @click="onSearchOk">
              <Spinner v-if="searchBusy" class="size-3.5" />
              <AppIcon v-else name="search" class="size-3.5" />
              {{ t("ssh.search") }}
            </Button>
          </div>
          <div class="flex items-center gap-2 text-xs text-muted-foreground">
            <Label for="sftp-search-depth" class="shrink-0">{{ t("ssh.search_maxdepth") }}</Label>
            <Input id="sftp-search-depth" v-model="searchDialog.maxDepth" type="number" min="1" max="20" class="w-16" placeholder="5" />
          </div>
          <div v-if="searchResults.length" class="sftp-search-results">
            <div v-for="(r, ri) in searchResults" :key="ri" class="sftp-search-result" @click="gotoResult(r)">
              <AppIcon name="file" class="size-3.5 shrink-0" />
              <span class="truncate font-mono text-xs">{{ r }}</span>
            </div>
          </div>
          <p v-if="!searchResults.length && !searchBusy" class="text-xs text-muted-foreground">
            {{ t("ssh.search_no_result") }}
          </p>
        </div>
        <DialogFooter>
          <Button variant="outline" @click="searchDialog = null">{{ t("common.close") }}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>

<style scoped>
.sftp-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 14px;
  border-bottom: 1px solid var(--color-border);
  flex-wrap: wrap;
}

.crumbs {
  display: flex;
  align-items: center;
  gap: 2px;
  font-size: 13px;
  min-width: 0;
  overflow-x: auto;
  white-space: nowrap;
}

.crumb {
  border: none;
  background: transparent;
  color: var(--color-muted-foreground);
  padding: 2px 4px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
}

.crumb:hover {
  background-color: var(--color-accent);
  color: var(--color-foreground);
}

.crumb:last-child {
  color: var(--color-foreground);
  font-weight: 500;
}

.crumb-sep {
  color: var(--color-muted-foreground);
  opacity: 0.5;
}

.entry-name {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  cursor: pointer;
}

.entry-row.dir .entry-name:hover {
  color: var(--color-primary);
}

.sftp-search-results {
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-height: 260px;
  overflow-y: auto;
}
.sftp-search-result {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 8px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  cursor: pointer;
  color: var(--color-foreground);
  transition: background-color 0.12s ease;
}
.sftp-search-result:hover {
  background-color: var(--color-muted);
}

.drop-zone {
  transition: outline 0.12s ease;
}

.transfer-bar {
  position: sticky;
  bottom: 8px;
  margin-top: 12px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 8px 14px;
  border-radius: 8px;
  border: 1px solid var(--color-border);
  background-color: var(--color-card);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
}

/* 队列中的单个传输任务 */
.transfer-item {
  display: flex;
  align-items: center;
  gap: 10px;
}

/* 双栏模式：本地侧标识 */
.local-tag {
  flex-shrink: 0;
  padding: 2px 8px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 600;
  color: var(--color-muted-foreground, #888);
  background: var(--color-accent, rgba(127, 127, 127, 0.15));
}

.local-pane :deep(.crumbs) {
  flex: 1;
  min-width: 0;
  overflow-x: auto;
}

.transfer-name {
  font-size: 13px;
  max-width: 260px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.transfer-kind {
  font-size: 12px;
  color: var(--color-muted-foreground);
  flex-shrink: 0;
}

.transfer-pct {
  font-size: 12px;
  font-family: "JetBrains Mono", monospace;
  color: var(--color-muted-foreground);
  width: 42px;
  text-align: right;
  flex-shrink: 0;
}

.empty-hint {
  margin-top: 4px;
  font-size: 12px;
  opacity: 0.7;
}

/* ── SFTP AI 助手布局 ─────────────────────────────────────── */
.sftp-ai-layout {
  display: flex;
  gap: 12px;
  min-height: 0;
  flex: 1;
}

.sftp-ai-panel {
  width: 330px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  background-color: var(--color-card);
  overflow: hidden;
}
.sftp-ai-panel.collapsed {
  width: 44px;
}
.sftp-ai-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 12px;
  border-bottom: 1px solid var(--color-border);
  background-color: var(--color-muted);
}
.sftp-ai-title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  font-weight: 600;
  color: var(--color-foreground);
}
.sftp-ai-toggle {
  border: none;
  background: transparent;
  color: var(--color-muted-foreground);
  cursor: pointer;
  padding: 4px;
  border-radius: 6px;
  display: inline-flex;
}
.sftp-ai-toggle:hover {
  background-color: var(--color-border);
  color: var(--color-foreground);
}
.sftp-ai-body {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
}
.sftp-ai-models {
  padding: 10px 12px;
  border-bottom: 1px solid var(--color-border);
}
.sftp-ai-messages {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  scrollbar-width: thin;
}
.sftp-ai-empty {
  margin: auto;
  text-align: center;
  font-size: 12px;
  color: var(--color-muted-foreground);
  padding: 20px;
}
.sftp-ai-msg {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.sftp-ai-msg.user .sftp-ai-msg-role {
  color: var(--color-primary);
}
.sftp-ai-msg.assistant .sftp-ai-msg-role {
  color: var(--color-success);
}
.sftp-ai-msg-role {
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}
.sftp-ai-msg-text {
  font-size: 12px;
  line-height: 1.55;
  color: var(--color-foreground);
  white-space: pre-wrap;
  word-break: break-word;
}
.sftp-ai-actions {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-top: 4px;
}
.sftp-ai-action {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background-color: var(--color-muted);
}
.sftp-ai-action-code {
  flex: 1;
  font-family: "JetBrains Mono", monospace;
  font-size: 11px;
  color: var(--color-foreground);
  white-space: pre-wrap;
  word-break: break-all;
}
.sftp-ai-input {
  display: flex;
  gap: 8px;
  padding: 10px 12px;
  border-top: 1px solid var(--color-border);
}
</style>

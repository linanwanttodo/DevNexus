<script>
import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { showToast } from "../lib/toast.js";
  import { showConfirm } from "../lib/confirm.js";
  import { t, getVersion, onLangChange } from "../lib/i18n.js";

  let _v = $state(getVersion());
  $effect(() => onLangChange(v => _v = v));

  let locked = $state(true);
  let hasMasterPassword = $state(false);
  let masterPassword = $state("");
  let setupPassword = $state("");
  let setupPasswordConfirm = $state("");
  let passwords = $state([]);
  let loading = $state(true);
  let showAddModal = $state(false);
  let showPassword = $state(null);
  
  // 表单数据
  let entryName = $state("");
  let username = $state("");
  let password = $state("");
  let url = $state("");
  let notes = $state("");

  async function checkState() {
    try {
      const lockedState = await invoke("is_locked");
      const hasPwd = await invoke("has_master_password");
      locked = lockedState;
      hasMasterPassword = hasPwd;
    } catch (err) {
      console.error("Failed to check password manager state:", err);
    }
  }

  async function setupMasterPassword() {
    if (!setupPassword || setupPassword.length < 4) {
      showToast("Master password must be at least 4 characters");
      return;
    }
    if (setupPassword !== setupPasswordConfirm) {
      showToast("Passwords do not match");
      return;
    }

    try {
      await invoke("set_master_password", { masterPassword: setupPassword });
      locked = false;
      hasMasterPassword = true;
      setupPassword = "";
      setupPasswordConfirm = "";
      await loadPasswords();
      showToast("Master password set successfully");
    } catch (err) {
      showToast("Failed to set master password: " + (err.message || err));
    }
  }

  async function unlock() {
    if (!masterPassword) return;

    try {
      const success = await invoke("unlock", { masterPassword });
      if (success) {
        locked = false;
        masterPassword = "";
        await loadPasswords();
        showToast("Password manager unlocked");
      } else {
        showToast("Incorrect master password");
      }
    } catch (err) {
      showToast("Failed to unlock: " + (err.message || err));
    }
  }

  async function lockVault() {
    try {
      await invoke("lock");
      locked = true;
      passwords = [];
      showToast("Password manager locked");
    } catch (err) {
      console.error("Failed to lock:", err);
    }
  }

  async function loadPasswords() {
    try {
      loading = true;
      passwords = await invoke("list_passwords");
    } catch (err) {
      console.error("Failed to load passwords:", err);
      showToast(t('passwords.failed_load'));
    } finally {
      loading = false;
    }
  }

  async function addPassword() {
    if (!entryName || !username || !password) {
      showToast(t('passwords.fill_fields'));
      return;
    }

    try {
      await invoke("add_password", {
        name: entryName,
        username,
        password,
        url: url || null,
        notes: notes || null,
      });
      
      showAddModal = false;
      resetForm();
      await loadPasswords();
      showToast(t('passwords.add_success'));
    } catch (err) {
      showToast(t('passwords.add_failed').replace('{error}', err.message || err));
    }
  }

  async function deletePassword(id) {
    if (!await showConfirm(t('passwords.delete_confirm'))) return;
    
    try {
      await invoke("delete_password", { id });
      await loadPasswords();
    } catch (err) {
      showToast(t('passwords.delete_failed').replace('{error}', err.message || err));
    }
  }

  async function viewPassword(id) {
    try {
      const pwd = await invoke("get_password", { id });
      showPassword = { id, password: pwd };
    } catch (err) {
      showToast(t('passwords.view_failed').replace('{error}', err.message || err));
    }
  }

  async function exportCSV() {
    try {
      const csvContent = await invoke("export_chrome_csv");
      
      // 创建下载
      const blob = new Blob([csvContent], { type: 'text/csv' });
      const downloadUrl = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = downloadUrl;
      a.download = `passwords_export_${new Date().toISOString().split('T')[0]}.csv`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(downloadUrl);
      
      showToast(t('passwords.export_success'));
    } catch (err) {
      showToast(t('passwords.export_failed').replace('{error}', err.message || err));
    }
  }

  async function importCSV(event) {
    const file = event.target.files[0];
    if (!file) return;

    try {
      const text = await file.text();
      const count = await invoke("import_chrome_csv", { csvContent: text });
      await loadPasswords();
      showToast(t('passwords.import_success').replace('{count}', count));
      event.target.value = ''; // 重置文件输入
    } catch (err) {
      showToast(t('passwords.import_failed').replace('{error}', err.message || err));
    }
  }

  async function saveToFile() {
    const masterPassword = prompt(t('passwords.master_pwd_save'));
    if (!masterPassword) return;

    try {
      // 使用 Tauri dialog 选择文件路径（需要添加 tauri-plugin-dialog）
      const filePath = prompt(t('passwords.file_path_save'));
      if (!filePath) return;

      await invoke("save_to_file", {
        filePath,
        masterPassword,
      });
      showToast(t('passwords.save_success'));
    } catch (err) {
      showToast(t('passwords.save_failed').replace('{error}', err.message || err));
    }
  }

  async function loadFromFile() {
    const masterPassword = prompt(t('passwords.master_pwd_load'));
    if (!masterPassword) return;

    try {
      const filePath = prompt(t('passwords.file_path_load'));
      if (!filePath) return;

      const count = await invoke("load_from_file", {
        filePath,
        masterPassword,
      });
      await loadPasswords();
      showToast(t('passwords.load_success').replace('{count}', count));
    } catch (err) {
      showToast(t('passwords.load_failed').replace('{error}', err.message || err));
    }
  }

  function resetForm() {
    entryName = "";
    username = "";
    password = "";
    url = "";
    notes = "";
  }

  function copyToClipboard(text) {
    navigator.clipboard.writeText(text).then(() => {
      showToast(t('passwords.copied'));
    });
  }

  onMount(async () => {
    await checkState();
    if (!locked) {
      await loadPasswords();
    } else {
      loading = false;
    }
  });
</script>

<div class="mx-auto max-w-6xl">
  <!-- Header -->
  <div class="mb-6 flex items-center justify-between">
    <div>
      <h1 class="text-xl font-semibold text-nx-text">{t('passwords.title')}</h1>
      <p class="mt-1 text-xs text-nx-text-muted">{t('passwords.desc')}</p>
    </div>
    <div class="flex gap-2">
      {#if locked}
        <span></span>
      {:else}
        <button 
          class="flex items-center gap-2 border border-nx-border bg-nx-surface px-4 py-2 text-sm font-medium text-nx-text"
          onclick={exportCSV}>
          <span class="material-symbols-outlined text-lg">download</span>
          {t('passwords.export_csv')}
        </button>
        <label class="flex cursor-pointer items-center gap-2 border border-nx-border bg-nx-surface px-4 py-2 text-sm font-medium text-nx-text">
          <span class="material-symbols-outlined text-lg">upload</span>
          {t('passwords.import_csv')}
          <input type="file" accept=".csv" class="hidden" onchange={importCSV} />
        </label>
        <button 
          class="flex items-center gap-2 border border-nx-border bg-nx-surface px-4 py-2 text-sm font-medium text-nx-text"
          onclick={saveToFile}>
          <span class="material-symbols-outlined text-lg">save</span>
          {t('passwords.save_encrypted')}
        </button>
        <button 
          class="flex items-center gap-2 bg-nx-accent px-4 py-2 text-sm font-medium text-white"
          onclick={() => showAddModal = true}>
          <span class="material-symbols-outlined text-lg">add</span>
          {t('passwords.add')}
        </button>
        <button 
          class="flex items-center gap-2 border border-nx-border bg-nx-surface px-4 py-2 text-sm font-medium text-nx-text-secondary"
          onclick={lockVault}>
          <span class="material-symbols-outlined text-lg">lock</span>
          {t('passwords.lock')}
        </button>
      {/if}
    </div>
  </div>

  <!-- Lock/Setup Screen -->
  {#if locked}
    {#if hasMasterPassword}
      <!-- Unlock -->
      <div class="mx-auto mt-16 max-w-md border border-nx-border bg-nx-surface p-8 text-center">
        <div class="mb-4">
          <span class="material-symbols-outlined text-nx-text-secondary text-5xl">lock</span>
        </div>
        <h2 class="mb-2 text-lg font-semibold text-nx-text">{t('passwords.title_locked')}</h2>
        <p class="mb-6 text-sm text-nx-text-muted">{t('passwords.desc_locked')}</p>
        <input
          type="password"
          bind:value={masterPassword}
          placeholder={t('passwords.master_password_placeholder')}
          class="mb-4 w-full border border-nx-border bg-nx-bg px-4 py-3 text-sm text-nx-text outline-none focus:border-nx-text-secondary"
          onkeydown={(e) => e.key === 'Enter' && unlock()}
        />
        <button
          class="w-full bg-nx-text px-4 py-3 text-sm font-medium text-nx-deep disabled:opacity-50"
          onclick={unlock}
          disabled={!masterPassword}>
          {t('passwords.unlock')}
        </button>
      </div>
    {:else}
      <!-- Setup Master Password -->
      <div class="mx-auto mt-16 max-w-md border border-nx-border bg-nx-surface p-8 text-center">
        <div class="mb-4">
          <span class="material-symbols-outlined text-nx-text-secondary text-5xl">lock_reset</span>
        </div>
        <h2 class="mb-2 text-lg font-semibold text-nx-text">{t('passwords.title_setup')}</h2>
        <p class="mb-6 text-sm text-nx-text-muted">{t('passwords.desc_setup')}</p>
        <input
          type="password"
          bind:value={setupPassword}
          placeholder={t('passwords.setup_password_placeholder')}
          class="mb-3 w-full border border-nx-border bg-nx-bg px-4 py-3 text-sm text-nx-text outline-none focus:border-nx-text-secondary"
        />
        <input
          type="password"
          bind:value={setupPasswordConfirm}
          placeholder={t('passwords.setup_password_confirm_placeholder')}
          class="mb-4 w-full border border-nx-border bg-nx-bg px-4 py-3 text-sm text-nx-text outline-none focus:border-nx-text-secondary"
          onkeydown={(e) => e.key === 'Enter' && setupMasterPassword()}
        />
        <button
          class="w-full bg-nx-text px-4 py-3 text-sm font-medium text-nx-deep disabled:opacity-50"
          onclick={setupMasterPassword}
          disabled={!setupPassword || !setupPasswordConfirm}>
          {t('passwords.setup')}
        </button>
      </div>
    {/if}
  {:else if loading}
    <div class="flex items-center justify-center py-12">
      <span class="material-symbols-outlined animate-spin text-nx-text-muted text-3xl">progress_activity</span>
    </div>
  {:else if passwords.length === 0}
    <div class="border border-nx-border bg-nx-surface p-12 text-center">
      <span class="material-symbols-outlined text-nx-text-muted text-4xl">lock</span>
      <div class="mt-4 text-sm text-nx-text-muted">{t('passwords.no_passwords')}</div>
      <div class="mt-1 text-xs text-nx-text-muted">{t('passwords.no_passwords_desc')}</div>
    </div>
  {:else}
    <div class="border border-nx-border bg-nx-surface">
      <table class="w-full">
        <thead>
          <tr class="border-b border-nx-border text-xs text-nx-text-muted">
            <th class="px-4 py-3 text-left font-medium">{t('passwords.name')}</th>
            <th class="px-4 py-3 text-left font-medium">{t('passwords.username')}</th>
            <th class="px-4 py-3 text-left font-medium">URL</th>
            <th class="px-4 py-3 text-left font-medium">{t('passwords.created')}</th>
            <th class="px-4 py-3 text-right font-medium">{t('passwords.actions')}</th>
          </tr>
        </thead>
        <tbody>
          {#each passwords as entry}
            <tr class="group border-b border-nx-border last:border-0">
              <td class="px-4 py-3">
                <div class="flex items-center gap-2">
                  <span class="material-symbols-outlined text-nx-text-secondary">key</span>
                  <span class="text-sm font-medium text-nx-text">{entry.name}</span>
                </div>
              </td>
              <td class="px-4 py-3 text-sm text-nx-text-secondary">{entry.username}</td>
              <td class="px-4 py-3 text-xs text-nx-text-muted">
                {#if entry.url}
                  <a href={entry.url} target="_blank" class="text-nx-text-secondary underline">{entry.url}</a>
                {:else}
                  <span class="text-nx-text-muted">{t('passwords.no_url')}</span>
                {/if}
              </td>
              <td class="px-4 py-3 text-xs text-nx-text-muted">{entry.created_at}</td>
              <td class="px-4 py-3">
                <div class="flex items-center justify-end gap-1">
                  <button 
                    class="p-1.5 text-nx-text-secondary"
                    title={t('passwords.title_view')}
                    onclick={() => viewPassword(entry.id)}>
                    <span class="material-symbols-outlined text-lg">visibility</span>
                  </button>
                  <button 
                    class="p-1.5 text-nx-text-secondary"
                    title={t('passwords.title_copy')}
                    onclick={() => copyToClipboard(entry.username)}>
                    <span class="material-symbols-outlined text-lg">content_copy</span>
                  </button>
                  <button 
                    class="p-1.5 text-nx-text-secondary"
                    title={t('passwords.title_delete')}
                    onclick={() => deletePassword(entry.id)}>
                    <span class="material-symbols-outlined text-lg">delete</span>
                  </button>
                </div>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<!-- Add Password Modal -->
{#if showAddModal}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50" role="button" tabindex="0" onkeydown={(e) => e.key === 'Escape' && (showAddModal = false)} onclick={() => showAddModal = false}>
    <div class="w-full max-w-lg border border-nx-border bg-nx-surface p-6" role="dialog" aria-modal="true" tabindex="-1" onkeydown={(e) => e.stopPropagation()} onclick={(e) => e.stopPropagation()}>
      <h2 class="mb-4 text-lg font-semibold text-nx-text">{t('passwords.add')}</h2>
      
      <div class="space-y-4">
        <div>
          <label class="mb-1 block text-xs text-nx-text-muted" for="pm-name-add">{t('passwords.name')} *</label>
          <input
            id="pm-name-add"
            type="text"
            bind:value={entryName}
            placeholder="GitHub Account"
            class="w-full border border-nx-border bg-nx-bg px-3 py-2 text-sm text-nx-text placeholder:text-nx-text-muted outline-none focus:border-nx-text-secondary"
          />
        </div>

        <div>
          <label class="mb-1 block text-xs text-nx-text-muted" for="pm-username-add">{t('passwords.username')} *</label>
          <input
            id="pm-username-add"
            type="text"
            bind:value={username}
            placeholder="user@example.com"
            class="w-full border border-nx-border bg-nx-bg px-3 py-2 text-sm text-nx-text placeholder:text-nx-text-muted outline-none focus:border-nx-text-secondary"
          />
        </div>

        <div>
          <label class="mb-1 block text-xs text-nx-text-muted" for="pm-password-add">{t('passwords.password')} *</label>
          <input
            id="pm-password-add"
            type="password"
            bind:value={password}
            placeholder="••••••••"
            class="w-full border border-nx-border bg-nx-bg px-3 py-2 text-sm text-nx-text placeholder:text-nx-text-muted outline-none focus:border-nx-text-secondary"
          />
        </div>

        <div>
          <label class="mb-1 block text-xs text-nx-text-muted" for="pm-url-add">URL</label>
          <input
            id="pm-url-add"
            type="url"
            bind:value={url}
            placeholder="https://github.com"
            class="w-full border border-nx-border bg-nx-bg px-3 py-2 text-sm text-nx-text placeholder:text-nx-text-muted outline-none focus:border-nx-text-secondary"
          />
        </div>

        <div>
          <label class="mb-1 block text-xs text-nx-text-muted" for="pm-notes-add">{t('passwords.notes')}</label>
          <textarea
            id="pm-notes-add"
            bind:value={notes}
            placeholder="Additional information..."
            rows="3"
            class="w-full border border-nx-border bg-nx-bg px-3 py-2 text-sm text-nx-text placeholder:text-nx-text-muted outline-none focus:border-nx-text-secondary"></textarea>
        </div>
      </div>

      <div class="mt-6 flex justify-end gap-3">
        <button
          class="border border-nx-border bg-nx-bg px-4 py-2 text-sm font-medium text-nx-text-secondary"
          onclick={() => showAddModal = false}>
          {t('passwords.cancel')}
        </button>
        <button
          class="bg-nx-text px-4 py-2 text-sm font-medium text-nx-deep"
          onclick={addPassword}>
          {t('passwords.save')}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- View Password Modal -->
{#if showPassword}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50" role="button" tabindex="0" onkeydown={(e) => e.key === 'Escape' && (showPassword = null)} onclick={() => showPassword = null}>
    <div class="w-full max-w-md border border-nx-border bg-nx-surface p-6" role="dialog" aria-modal="true" tabindex="-1" onkeydown={(e) => e.stopPropagation()} onclick={(e) => e.stopPropagation()}>
      <h2 class="mb-4 text-lg font-semibold text-nx-text">{t('passwords.details')}</h2>
      
      <div class="border border-nx-border bg-nx-bg p-4">
        <div class="mb-2 text-xs text-nx-text-muted">{t('passwords.password')}</div>
        <div class="flex items-center gap-2">
          <code class="flex-1 break-all text-sm text-nx-text">{showPassword.password}</code>
          <button
            class="p-1.5 text-nx-text-secondary"
            onclick={() => copyToClipboard(showPassword.password)}
            title={t('passwords.title_copy')}>
            <span class="material-symbols-outlined text-lg">content_copy</span>
          </button>
        </div>
      </div>

      <div class="mt-6 flex justify-end">
        <button
          class="bg-nx-text px-4 py-2 text-sm font-medium text-nx-deep"
          onclick={() => showPassword = null}>
          {t('passwords.close')}
        </button>
      </div>
    </div>
  </div>
{/if}

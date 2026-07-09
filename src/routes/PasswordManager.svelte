<script>
import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { showToast } from "../lib/toast.svelte.js";
  import { showConfirm } from "../lib/confirm.svelte.js";
  import { t } from "../lib/i18n.svelte.js";

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
      showToast(t('common.error_msg').replace('{error}', err.message || err), "error");
    }
  }

  async function setupMasterPassword() {
    if (!setupPassword || setupPassword.length < 4) {
      showToast(t('common.master_too_short'));
      return;
    }
    if (setupPassword !== setupPasswordConfirm) {
      showToast(t('common.no_match'));
      return;
    }

    try {
      await invoke("set_master_password", { masterPassword: setupPassword });
      locked = false;
      hasMasterPassword = true;
      setupPassword = "";
      setupPasswordConfirm = "";
      await loadPasswords();
      showToast(t('common.master_set_ok'));
    } catch (err) {
      showToast(t('common.set_master_failed').replace('{error}', err.message || err));
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
        showToast(t('common.unlocked'));
      } else {
        showToast(t('common.incorrect'));
      }
    } catch (err) {
      showToast(t('common.unlock_failed').replace('{error}', err.message || err));
    }
  }

  async function lockVault() {
    try {
      await invoke("lock");
      locked = true;
      passwords = [];
      showToast(t('common.locked'));
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
    const fileMasterPwd = prompt(t('passwords.master_pwd_save'));
    if (!fileMasterPwd) return;

    try {
      const filePath = prompt(t('passwords.file_path_save'));
      if (!filePath) return;

      await invoke("save_to_file", {
        filePath,
        masterPassword: fileMasterPwd,
      });
      showToast(t('passwords.save_success'));
    } catch (err) {
      showToast(t('passwords.save_failed').replace('{error}', err.message || err));
    }
  }

  async function loadFromFile() {
    const fileMasterPwd = prompt(t('passwords.master_pwd_load'));
    if (!fileMasterPwd) return;

    try {
      const filePath = prompt(t('passwords.file_path_load'));
      if (!filePath) return;

      const count = await invoke("load_from_file", {
        filePath,
        masterPassword: fileMasterPwd,
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
    }).catch(() => {
      showToast(t('common.copy_failed'), "error");
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

<div class="mx-auto max-w-6xl p-5">
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
          class="nx-btn nx-btn-ghost flex items-center gap-2"
          onclick={exportCSV}>
          <span class="material-symbols-outlined text-lg">download</span>
          {t('passwords.export_csv')}
        </button>
        <label class="nx-btn nx-btn-ghost flex cursor-pointer items-center gap-2">
          <span class="material-symbols-outlined text-lg">upload</span>
          {t('passwords.import_csv')}
          <input type="file" accept=".csv" class="hidden" onchange={importCSV} />
        </label>
        <button 
          class="nx-btn nx-btn-ghost flex items-center gap-2"
          onclick={saveToFile}>
          <span class="material-symbols-outlined text-lg">save</span>
          {t('passwords.save_encrypted')}
        </button>
        <button 
          class="nx-btn nx-btn-primary flex items-center gap-2"
          onclick={() => showAddModal = true}>
          <span class="material-symbols-outlined text-lg">add</span>
          {t('passwords.add')}
        </button>
        <button 
          class="nx-btn nx-btn-ghost flex items-center gap-2"
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
      <div class="mx-auto mt-16 max-w-md nx-card p-8 text-center">
        <div class="mb-4">
          <span class="material-symbols-outlined text-nx-text-secondary text-5xl">lock</span>
        </div>
        <h2 class="mb-2 text-lg font-semibold text-nx-text">{t('passwords.title_locked')}</h2>
        <p class="mb-6 text-sm text-nx-text-muted">{t('passwords.desc_locked')}</p>
        <input
          type="password"
          bind:value={masterPassword}
          placeholder={t('passwords.master_password_placeholder')}
          class="nx-input mb-4 w-full"
          onkeydown={(e) => e.key === 'Enter' && unlock()}
        />
        <button
          class="nx-btn nx-btn-primary w-full"
          onclick={unlock}
          disabled={!masterPassword}>
          {t('passwords.unlock')}
        </button>
      </div>
    {:else}
      <!-- Setup Master Password -->
      <div class="mx-auto mt-16 max-w-md nx-card p-8 text-center">
        <div class="mb-4">
          <span class="material-symbols-outlined text-nx-text-secondary text-5xl">lock_reset</span>
        </div>
        <h2 class="mb-2 text-lg font-semibold text-nx-text">{t('passwords.title_setup')}</h2>
        <p class="mb-6 text-sm text-nx-text-muted">{t('passwords.desc_setup')}</p>
        <input
          type="password"
          bind:value={setupPassword}
          placeholder={t('passwords.setup_password_placeholder')}
          class="nx-input mb-3 w-full"
        />
        <input
          type="password"
          bind:value={setupPasswordConfirm}
          placeholder={t('passwords.setup_password_confirm_placeholder')}
          class="nx-input mb-4 w-full"
          onkeydown={(e) => e.key === 'Enter' && setupMasterPassword()}
        />
        <button
          class="nx-btn nx-btn-primary w-full"
          onclick={setupMasterPassword}
          disabled={!setupPassword || !setupPasswordConfirm}>
          {t('passwords.setup')}
        </button>
      </div>
    {/if}
  {:else if loading}
    <div class="flex items-center justify-center py-12">
      <span class="material-symbols-outlined nx-animate-spin text-nx-text-muted text-3xl">progress_activity</span>
    </div>
  {:else if passwords.length === 0}
    <div class="nx-empty p-12 text-center">
      <span class="material-symbols-outlined text-nx-text-muted text-4xl">lock</span>
      <div class="mt-4 text-sm text-nx-text-muted">{t('passwords.no_passwords')}</div>
      <div class="mt-1 text-xs text-nx-text-muted">{t('passwords.no_passwords_desc')}</div>
    </div>
  {:else}
    <div class="nx-section">
      <table class="nx-table w-full">
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
  <div class="nx-dialog-overlay" role="button" tabindex="0" onkeydown={(e) => e.key === 'Escape' && (showAddModal = false)} onclick={() => showAddModal = false}>
    <div class="nx-dialog" role="dialog" aria-modal="true" tabindex="-1" onkeydown={(e) => e.stopPropagation()} onclick={(e) => e.stopPropagation()}>
      <div class="nx-dialog-header">
        <h2 class="text-lg font-semibold text-nx-text">{t('passwords.add')}</h2>
      </div>
      
      <div class="nx-dialog-body space-y-4">
        <div>
          <label class="mb-1 block text-xs text-nx-text-muted" for="pm-name-add">{t('passwords.name')} *</label>
          <input
            id="pm-name-add"
            type="text"
            bind:value={entryName}
            placeholder="GitHub Account"
            class="nx-input w-full"
          />
        </div>

        <div>
          <label class="mb-1 block text-xs text-nx-text-muted" for="pm-username-add">{t('passwords.username')} *</label>
          <input
            id="pm-username-add"
            type="text"
            bind:value={username}
            placeholder="user@example.com"
            class="nx-input w-full"
          />
        </div>

        <div>
          <label class="mb-1 block text-xs text-nx-text-muted" for="pm-password-add">{t('passwords.password')} *</label>
          <input
            id="pm-password-add"
            type="password"
            bind:value={password}
            placeholder="••••••••"
            class="nx-input w-full"
          />
        </div>

        <div>
          <label class="mb-1 block text-xs text-nx-text-muted" for="pm-url-add">URL</label>
          <input
            id="pm-url-add"
            type="url"
            bind:value={url}
            placeholder="https://github.com"
            class="nx-input w-full"
          />
        </div>

        <div>
          <label class="mb-1 block text-xs text-nx-text-muted" for="pm-notes-add">{t('passwords.notes')}</label>
          <textarea
            id="pm-notes-add"
            bind:value={notes}
            placeholder="Additional information..."
            rows="3"
            class="nx-input w-full"></textarea>
        </div>
      </div>

      <div class="nx-dialog-footer">
        <button
          class="nx-btn nx-btn-ghost"
          onclick={() => showAddModal = false}>
          {t('passwords.cancel')}
        </button>
        <button
          class="nx-btn nx-btn-primary"
          onclick={addPassword}>
          {t('passwords.save')}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- View Password Modal -->
{#if showPassword}
  <div class="nx-dialog-overlay" role="button" tabindex="0" onkeydown={(e) => e.key === 'Escape' && (showPassword = null)} onclick={() => showPassword = null}>
    <div class="nx-dialog" role="dialog" aria-modal="true" tabindex="-1" onkeydown={(e) => e.stopPropagation()} onclick={(e) => e.stopPropagation()}>
      <div class="nx-dialog-header">
        <h2 class="text-lg font-semibold text-nx-text">{t('passwords.details')}</h2>
      </div>
      
      <div class="nx-dialog-body">
        <div class="nx-card p-4">
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
      </div>

      <div class="nx-dialog-footer">
        <button
          class="nx-btn nx-btn-primary"
          onclick={() => showPassword = null}>
          {t('passwords.close')}
        </button>
      </div>
    </div>
  </div>
{/if}

<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

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

  async function loadPasswords() {
    try {
      loading = true;
      passwords = await invoke("list_passwords");
    } catch (err) {
      console.error("Failed to load passwords:", err);
      alert("Failed to load passwords");
    } finally {
      loading = false;
    }
  }

  async function addPassword() {
    if (!entryName || !username || !password) {
      alert("Please fill in all required fields");
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
      alert("Password added successfully");
    } catch (err) {
      alert(`Failed to add password: ${err.message || err}`);
    }
  }

  async function deletePassword(id) {
    if (!confirm("Delete this password?")) return;
    
    try {
      await invoke("delete_password", { id });
      await loadPasswords();
    } catch (err) {
      alert(`Failed to delete password: ${err.message || err}`);
    }
  }

  async function viewPassword(id) {
    try {
      const pwd = await invoke("get_password", { id });
      showPassword = { id, password: pwd };
    } catch (err) {
      alert(`Failed to decrypt password: ${err.message || err}`);
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
      
      alert("Passwords exported successfully");
    } catch (err) {
      alert(`Export failed: ${err.message || err}`);
    }
  }

  async function importCSV(event) {
    const file = event.target.files[0];
    if (!file) return;

    try {
      const text = await file.text();
      const count = await invoke("import_chrome_csv", { csvContent: text });
      await loadPasswords();
      alert(`Successfully imported ${count} passwords`);
      event.target.value = ''; // 重置文件输入
    } catch (err) {
      alert(`Import failed: ${err.message || err}`);
    }
  }

  async function saveToFile() {
    const masterPassword = prompt("Enter master password for encryption:");
    if (!masterPassword) return;

    try {
      // 使用 Tauri dialog 选择文件路径（需要添加 tauri-plugin-dialog）
      const filePath = prompt("Enter file path to save (e.g., /home/user/passwords.enc):");
      if (!filePath) return;

      await invoke("save_to_file", {
        filePath,
        masterPassword,
      });
      alert("Passwords saved successfully");
    } catch (err) {
      alert(`Save failed: ${err.message || err}`);
    }
  }

  async function loadFromFile() {
    const masterPassword = prompt("Enter master password for decryption:");
    if (!masterPassword) return;

    try {
      const filePath = prompt("Enter file path to load:");
      if (!filePath) return;

      const count = await invoke("load_from_file", {
        filePath,
        masterPassword,
      });
      await loadPasswords();
      alert(`Successfully loaded ${count} passwords`);
    } catch (err) {
      alert(`Load failed: ${err.message || err}`);
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
      alert("Copied to clipboard");
    });
  }

  onMount(() => {
    loadPasswords();
  });
</script>

<div class="mx-auto max-w-6xl">
  <!-- Header -->
  <div class="mb-6 flex items-center justify-between">
    <div>
      <h1 class="text-xl font-semibold text-nx-text">Password Manager</h1>
      <p class="mt-1 text-xs text-nx-text-muted">Securely store and manage your passwords with AES-256 encryption</p>
    </div>
    <div class="flex gap-2">
      <button 
        class="flex items-center gap-2 border border-nx-border bg-nx-surface px-4 py-2 text-sm font-medium text-nx-text"
        onclick={exportCSV}>
        <span class="material-symbols-outlined text-lg">download</span>
        Export CSV
      </button>
      <label class="flex cursor-pointer items-center gap-2 border border-nx-border bg-nx-surface px-4 py-2 text-sm font-medium text-nx-text">
        <span class="material-symbols-outlined text-lg">upload</span>
        Import CSV
        <input type="file" accept=".csv" class="hidden" onchange={importCSV} />
      </label>
      <button 
        class="flex items-center gap-2 border border-nx-border bg-nx-surface px-4 py-2 text-sm font-medium text-nx-text"
        onclick={saveToFile}>
        <span class="material-symbols-outlined text-lg">save</span>
        Save Encrypted
      </button>
      <button 
        class="flex items-center gap-2 bg-nx-accent px-4 py-2 text-sm font-medium text-white"
        onclick={() => showAddModal = true}>
        <span class="material-symbols-outlined text-lg">add</span>
        Add Password
      </button>
    </div>
  </div>

  <!-- Password List -->
  {#if loading}
    <div class="flex items-center justify-center py-12">
      <span class="material-symbols-outlined animate-spin text-nx-text-muted text-3xl">progress_activity</span>
    </div>
  {:else if passwords.length === 0}
    <div class="border border-nx-border bg-nx-surface p-12 text-center">
      <span class="material-symbols-outlined text-nx-text-muted text-4xl">lock</span>
      <div class="mt-4 text-sm text-nx-text-muted">No passwords stored</div>
      <div class="mt-1 text-xs text-nx-text-muted">Add your first password entry</div>
    </div>
  {:else}
    <div class="border border-nx-border bg-nx-surface">
      <table class="w-full">
        <thead>
          <tr class="border-b border-nx-border text-xs text-nx-text-muted">
            <th class="px-4 py-3 text-left font-medium">Name</th>
            <th class="px-4 py-3 text-left font-medium">Username</th>
            <th class="px-4 py-3 text-left font-medium">URL</th>
            <th class="px-4 py-3 text-left font-medium">Created</th>
            <th class="px-4 py-3 text-right font-medium">Actions</th>
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
                  <span class="text-nx-text-muted">—</span>
                {/if}
              </td>
              <td class="px-4 py-3 text-xs text-nx-text-muted">{entry.created_at}</td>
              <td class="px-4 py-3">
                <div class="flex items-center justify-end gap-1">
                  <button 
                    class="p-1.5 text-nx-text-secondary"
                    title="View Password"
                    onclick={() => viewPassword(entry.id)}>
                    <span class="material-symbols-outlined text-lg">visibility</span>
                  </button>
                  <button 
                    class="p-1.5 text-nx-text-secondary"
                    title="Copy Username"
                    onclick={() => copyToClipboard(entry.username)}>
                    <span class="material-symbols-outlined text-lg">content_copy</span>
                  </button>
                  <button 
                    class="p-1.5 text-nx-text-secondary"
                    title="Delete"
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
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onclick={() => showAddModal = false}>
    <div class="w-full max-w-lg border border-nx-border bg-nx-surface p-6" onclick={(e) => e.stopPropagation()}>
      <h2 class="mb-4 text-lg font-semibold text-nx-text">Add Password</h2>
      
      <div class="space-y-4">
        <div>
          <label class="mb-1 block text-xs text-nx-text-muted">Name *</label>
          <input
            type="text"
            bind:value={entryName}
            placeholder="GitHub Account"
            class="w-full border border-nx-border bg-nx-bg px-3 py-2 text-sm text-nx-text placeholder:text-nx-text-muted outline-none focus:border-nx-text-secondary"
          />
        </div>

        <div>
          <label class="mb-1 block text-xs text-nx-text-muted">Username *</label>
          <input
            type="text"
            bind:value={username}
            placeholder="user@example.com"
            class="w-full border border-nx-border bg-nx-bg px-3 py-2 text-sm text-nx-text placeholder:text-nx-text-muted outline-none focus:border-nx-text-secondary"
          />
        </div>

        <div>
          <label class="mb-1 block text-xs text-nx-text-muted">Password *</label>
          <input
            type="password"
            bind:value={password}
            placeholder="••••••••"
            class="w-full border border-nx-border bg-nx-bg px-3 py-2 text-sm text-nx-text placeholder:text-nx-text-muted outline-none focus:border-nx-text-secondary"
          />
        </div>

        <div>
          <label class="mb-1 block text-xs text-nx-text-muted">URL</label>
          <input
            type="url"
            bind:value={url}
            placeholder="https://github.com"
            class="w-full border border-nx-border bg-nx-bg px-3 py-2 text-sm text-nx-text placeholder:text-nx-text-muted outline-none focus:border-nx-text-secondary"
          />
        </div>

        <div>
          <label class="mb-1 block text-xs text-nx-text-muted">Notes</label>
          <textarea
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
          Cancel
        </button>
        <button
          class="bg-nx-text px-4 py-2 text-sm font-medium text-nx-deep"
          onclick={addPassword}>
          Save Password
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- View Password Modal -->
{#if showPassword}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onclick={() => showPassword = null}>
    <div class="w-full max-w-md border border-nx-border bg-nx-surface p-6" onclick={(e) => e.stopPropagation()}>
      <h2 class="mb-4 text-lg font-semibold text-nx-text">Password Details</h2>
      
      <div class="border border-nx-border bg-nx-bg p-4">
        <div class="mb-2 text-xs text-nx-text-muted">Password</div>
        <div class="flex items-center gap-2">
          <code class="flex-1 break-all text-sm text-nx-text">{showPassword.password}</code>
          <button
            class="p-1.5 text-nx-text-secondary"
            onclick={() => copyToClipboard(showPassword.password)}
            title="Copy">
            <span class="material-symbols-outlined text-lg">content_copy</span>
          </button>
        </div>
      </div>

      <div class="mt-6 flex justify-end">
        <button
          class="bg-nx-text px-4 py-2 text-sm font-medium text-nx-deep"
          onclick={() => showPassword = null}>
          Close
        </button>
      </div>
    </div>
  </div>
{/if}

<script>
  import { getCurrentWindow } from "@tauri-apps/api/window";

  const appWindow = getCurrentWindow();
  let isMaximized = $state(false);

  async function minimize() {
    await appWindow.minimize();
  }

  async function toggleMaximize() {
    await appWindow.toggleMaximize();
    isMaximized = await appWindow.isMaximized();
  }

  async function close() {
    await appWindow.close();
  }
</script>

<header class="flex h-8 flex-shrink-0 select-none items-center border-b border-nx-border bg-nx-deep">
  <div class="flex items-center gap-2 pl-3 h-full flex-1">
    <span class="material-symbols-outlined text-nx-text-secondary text-sm">terminal</span>
    <span class="text-xs text-nx-text-secondary">DevNexus</span>
  </div>

  <div class="flex h-full items-stretch">
    <button type="button"
      class="flex w-10 items-center justify-center text-nx-text-muted"
      onclick={minimize}
    >
      <span class="material-symbols-outlined text-sm">minimize</span>
    </button>
    <button type="button"
      class="flex w-10 items-center justify-center text-nx-text-muted"
      onclick={toggleMaximize}
    >
      <span class="material-symbols-outlined text-sm">{isMaximized ? 'filter_none' : 'crop_square'}</span>
    </button>
    <button type="button"
      class="flex w-10 items-center justify-center text-nx-text-muted"
      onclick={close}
    >
      <span class="material-symbols-outlined text-sm">close</span>
    </button>
  </div>
</header>
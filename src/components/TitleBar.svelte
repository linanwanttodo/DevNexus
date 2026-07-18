<script>
  import { getCurrentWindow } from "@tauri-apps/api/window";

  const appWindow = getCurrentWindow();

  function minimize() { appWindow.minimize(); }
  function toggleMaximize() { appWindow.toggleMaximize(); }
  function close() { appWindow.close(); }
</script>

<header
  data-tauri-drag-region
  class="flex h-9 flex-shrink-0 items-center justify-between border-b border-[var(--nx-border)] bg-[var(--nx-deep)] select-none"
>
  <!-- Left: drag region spacer -->
  <div class="flex-1" data-tauri-drag-region></div>

  <!-- Right: window controls -->
  <div class="flex h-full">
    <button
      class="flex h-full w-11 items-center justify-center text-nx-text-muted transition-colors hover:bg-white/[0.08] hover:text-nx-text cursor-pointer"
      aria-label="最小化"
      onclick={minimize}
    >
      <svg width="11" height="1" viewBox="0 0 11 1" fill="currentColor">
        <rect width="11" height="1"/>
      </svg>
    </button>
    <button
      class="flex h-full w-11 items-center justify-center text-nx-text-muted transition-colors hover:bg-white/[0.08] hover:text-nx-text cursor-pointer"
      aria-label="最大化"
      onclick={toggleMaximize}
    >
      <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1">
        <rect x="0.5" y="0.5" width="9" height="9"/>
      </svg>
    </button>
    <button
      class="flex h-full w-11 items-center justify-center text-nx-text-muted transition-colors hover:bg-[#e81123] hover:text-white cursor-pointer"
      aria-label="关闭"
      onclick={close}
    >
      <svg width="10" height="10" viewBox="0 0 10 10" fill="currentColor">
        <path d="M1.007 0L0 1.007 3.993 5 0 8.993 1.007 10 5 6.007 8.993 10 10 8.993 6.007 5 10 1.007 8.993 0 5 3.993z"/>
      </svg>
    </button>
  </div>
</header>

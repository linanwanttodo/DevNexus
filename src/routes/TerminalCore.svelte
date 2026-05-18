<script>
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { Terminal } from "xterm";
  import { FitAddon } from "xterm-addon-fit";
  import { WebLinksAddon } from "xterm-addon-web-links";
  import "xterm/css/xterm.css";

  let terminalContainer = $state(null);
  let tabs = $state([]);
  let nextId = $state(1);
  let terminals = {}; // 存储 xterm 实例
  let sessionIds = {}; // 存储会话 ID
  
  // 初始化终端
  async function initTerminal(tabId) {
    try {
      // 创建后端 PTY 会话
      const sessionId = await invoke("spawn_terminal");
      sessionIds[tabId] = sessionId;
      
      // 创建 xterm 实例
      const term = new Terminal({
        cursorBlink: true,
        fontSize: 14,
        fontFamily: "'JetBrains Mono', monospace",
        theme: {
          background: "#0e141a",
          foreground: "#dde3ec",
          cursor: "#7bdb80",
          selectionBackground: "#2f81f740",
          black: "#0e141a",
          red: "#f85149",
          green: "#7bdb80",
          yellow: "#d29922",
          blue: "#58a6ff",
          magenta: "#bc8cff",
          cyan: "#39c5cf",
          white: "#dde3ec",
        },
      });
      
      // 添加插件
      const fitAddon = new FitAddon();
      const webLinksAddon = new WebLinksAddon();
      term.loadAddon(fitAddon);
      term.loadAddon(webLinksAddon);
      
      // 挂载到 DOM
      term.open(terminalContainer);
      fitAddon.fit();
      
      terminals[tabId] = { term, fitAddon };
      
      // 监听后端输出
      const unlisten = await listen("terminal-output", (event) => {
        const payload = event.payload;
        if (payload.session_id === sessionId) {
          term.write(payload.data);
        }
      });
      
      // 监听终端输入并发送到后端
      term.onData(async (data) => {
        try {
          await invoke("write_to_terminal", {
            sessionId: sessionId,
            data: data,
          });
        } catch (err) {
          console.error("Failed to write to terminal:", err);
        }
      });
      
      // 欢迎信息
      term.writeln("\x1b[32mWelcome to DevNexus Terminal!\x1b[0m");
      term.writeln("\x1b[90mSession ID: " + sessionId + "\x1b[0m\r\n");
      
    } catch (err) {
      console.error("Failed to spawn terminal:", err);
      alert("Failed to start terminal: " + (err.message || err));
    }
  }

  // 添加新标签页
  async function addTab() {
    const newTab = { id: nextId++, name: `Terminal ${tabs.length + 1}`, active: true };
    tabs = tabs.map((t) => ({ ...t, active: false }));
    tabs = [...tabs, newTab];
    
    // 等待 DOM 更新后初始化终端
    setTimeout(() => {
      initTerminal(newTab.id);
    }, 100);
  }

  // 关闭标签页
  async function closeTab(id) {
    if (tabs.length <= 1) return;
    
    // 关闭后端会话
    if (sessionIds[id]) {
      try {
        await invoke("close_terminal", { sessionId: sessionIds[id] });
      } catch (err) {
        console.error("Failed to close terminal:", err);
      }
    }
    
    // 清理 xterm 实例
    if (terminals[id]) {
      terminals[id].term.dispose();
      delete terminals[id];
    }
    
    const idx = tabs.findIndex((t) => t.id === id);
    tabs = tabs.filter((t) => t.id !== id);
    
    if (tabs.length > 0 && !tabs.some((t) => t.active)) {
      const newIdx = Math.min(idx, tabs.length - 1);
      tabs = tabs.map((t, i) => ({ ...t, active: i === newIdx }));
    }
  }

  // 切换标签页
  function switchTab(id) {
    tabs = tabs.map((t) => ({ ...t, active: t.id === id }));
  }

  // 初始化第一个标签页
  onMount(() => {
    addTab();
  });

  // 清理所有终端
  onDestroy(async () => {
    for (const tabId in sessionIds) {
      try {
        await invoke("close_terminal", { sessionId: sessionIds[tabId] });
      } catch (err) {
        console.error("Failed to close terminal:", err);
      }
    }
  });
</script>

<div class="flex h-full flex-col">
  <!-- Tab Bar -->
  <div class="flex items-center border-b border-nx-border bg-nx-surface">
    <div class="flex flex-1 overflow-x-auto">
      {#each tabs as tab}
        <button
          class="group flex items-center gap-2 border-r border-nx-border px-4 py-2 text-xs {tab.active
            ? 'bg-nx-bg text-nx-text'
            : 'text-nx-text-muted'}"
          onclick={() => switchTab(tab.id)}>
          <span class="material-symbols-outlined text-sm">terminal</span>
          {tab.name}
          {#if tabs.length > 1}
            <span
              class="ml-1 p-0.5 text-nx-text-muted"
              onclick={(e) => { e.stopPropagation(); closeTab(tab.id); }}>
              <span class="material-symbols-outlined text-xs">close</span>
            </span>
          {/if}
        </button>
      {/each}
    </div>
    <button
      class="flex items-center justify-center px-3 py-2 text-nx-text-muted"
      onclick={addTab}
      title="New Terminal">
      <span class="material-symbols-outlined text-lg">add</span>
    </button>
  </div>

  <!-- Terminal Area -->
  <div class="flex-1 bg-nx-deep overflow-hidden" bind:this={terminalContainer}>
    {#if tabs.length === 0}
      <div class="flex h-full items-center justify-center text-nx-text-muted">
        <div class="text-center">
          <span class="material-symbols-outlined text-4xl">terminal</span>
          <div class="mt-2 text-sm">No terminal sessions</div>
          <button 
            class="mt-4 bg-nx-text px-4 py-2 text-sm font-medium text-nx-deep"
            onclick={addTab}>
            Open Terminal
          </button>
        </div>
      </div>
    {/if}
  </div>
</div>

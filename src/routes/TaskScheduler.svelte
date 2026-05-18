<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  let tasks = $state([]);
  let loading = $state(true);
  let showAddModal = $state(false);
  
  // 表单数据
  let taskName = $state("");
  let cronExpression = $state("0 */6 * * *");
  let taskType = $state("script");
  let scriptContent = $state("");
  let command = $state("");

  const taskTypes = [
    { value: "script", label: "Script" },
    { value: "command", label: "Command" },
    { value: "shutdown", label: "Shutdown" },
    { value: "email", label: "Email" },
  ];

  const presets = [
    { label: "Every hour", value: "0 * * * *" },
    { label: "Every 6 hours", value: "0 */6 * * *" },
    { label: "Daily at midnight", value: "0 0 * * *" },
    { label: "Weekly on Monday", value: "0 0 * * 1" },
    { label: "Monthly on 1st", value: "0 0 1 * *" },
  ];

  async function loadTasks() {
    try {
      loading = true;
      tasks = await invoke("list_tasks");
    } catch (err) {
      console.error("Failed to load tasks:", err);
      alert("Failed to load tasks");
    } finally {
      loading = false;
    }
  }

  async function addTask() {
    if (!taskName || !cronExpression) {
      alert("Please fill in all required fields");
      return;
    }

    try {
      await invoke("add_task", {
        name: taskName,
        cronExpression,
        taskType,
        scriptContent: taskType === "script" ? scriptContent : null,
        command: taskType === "command" ? command : null,
      });
      
      showAddModal = false;
      resetForm();
      await loadTasks();
      alert("Task added successfully");
    } catch (err) {
      alert(`Failed to add task: ${err.message || err}`);
    }
  }

  async function deleteTask(id) {
    if (!confirm("Delete this task?")) return;
    
    try {
      await invoke("delete_task", { taskId: id });
      await loadTasks();
    } catch (err) {
      alert(`Failed to delete task: ${err.message || err}`);
    }
  }

  async function toggleTask(id) {
    try {
      await invoke("toggle_task", { taskId: id });
      await loadTasks();
    } catch (err) {
      alert(`Failed to toggle task: ${err.message || err}`);
    }
  }

  async function executeTask(id) {
    if (!confirm("Execute this task now?")) return;
    
    try {
      const result = await invoke("execute_task", { taskId: id });
      alert(`Task executed successfully:\n${result}`);
      await loadTasks();
    } catch (err) {
      alert(`Execution failed: ${err.message || err}`);
    }
  }

  function resetForm() {
    taskName = "";
    cronExpression = "0 */6 * * *";
    taskType = "script";
    scriptContent = "";
    command = "";
  }

  onMount(() => {
    loadTasks();
  });
</script>

<div class="mx-auto max-w-6xl">
  <!-- Header -->
  <div class="mb-6 flex items-center justify-between">
    <div>
      <h1 class="text-xl font-semibold text-nx-text">Task Scheduler</h1>
      <p class="mt-1 text-xs text-nx-text-muted">Automate scripts and system tasks with cron expressions</p>
    </div>
    <button 
      class="flex items-center gap-2 bg-nx-accent px-4 py-2 text-sm font-medium text-white"
      onclick={() => showAddModal = true}>
      <span class="material-symbols-outlined text-lg">add</span>
      New Task
    </button>
  </div>

  <!-- Tasks List -->
  {#if loading}
    <div class="flex items-center justify-center py-12">
      <span class="material-symbols-outlined animate-spin text-nx-text-muted text-3xl">progress_activity</span>
    </div>
  {:else if tasks.length === 0}
    <div class="border border-nx-border bg-nx-surface p-12 text-center">
      <span class="material-symbols-outlined text-nx-text-muted text-4xl">schedule</span>
      <div class="mt-4 text-sm text-nx-text-muted">No scheduled tasks</div>
      <div class="mt-1 text-xs text-nx-text-muted">Create your first automated task</div>
    </div>
  {:else}
    <div class="space-y-3">
      {#each tasks as task}
        <div class="border border-nx-border bg-nx-surface p-4">
          <div class="flex items-start justify-between">
            <div class="flex-1">
              <div class="flex items-center gap-3">
                <h3 class="text-sm font-medium text-nx-text">{task.name}</h3>
                <span class="px-2 py-0.5 text-xs font-medium
                  {task.enabled ? 'bg-nx-success/15 text-nx-success' : 'bg-nx-text-muted/15 text-nx-text-muted'}">
                  {task.enabled ? 'Active' : 'Disabled'}
                </span>
                <span class="bg-nx-text-secondary/15 px-2 py-0.5 text-xs font-medium text-nx-text-secondary">
                  {task.task_type}
                </span>
              </div>
              
              <div class="mt-2 flex items-center gap-4 text-xs text-nx-text-secondary">
                <div class="flex items-center gap-1">
                  <span class="material-symbols-outlined text-sm">schedule</span>
                  <code class="font-mono">{task.cron_expression}</code>
                </div>
                {#if task.next_run}
                  <div class="flex items-center gap-1">
                    <span class="material-symbols-outlined text-sm">calendar_today</span>
                    <span>Next: {task.next_run}</span>
                  </div>
                {/if}
                {#if task.last_run}
                  <div class="flex items-center gap-1">
                    <span class="material-symbols-outlined text-sm">history</span>
                    <span>Last: {task.last_run}</span>
                  </div>
                {/if}
                <div class="flex items-center gap-1">
                  <span class="material-symbols-outlined text-sm">repeat</span>
                  <span>{task.run_count} runs</span>
                </div>
              </div>
            </div>
            
            <div class="flex items-center gap-1">
              <button 
                class="p-1.5 text-nx-text-secondary"
                title={task.enabled ? 'Disable' : 'Enable'}
                onclick={() => toggleTask(task.id)}>
                <span class="material-symbols-outlined text-lg">
                  {task.enabled ? 'pause_circle' : 'play_circle'}
                </span>
              </button>
              <button 
                class="p-1.5 text-nx-text"
                title="Execute Now"
                onclick={() => executeTask(task.id)}>
                <span class="material-symbols-outlined text-lg">play_arrow</span>
              </button>
              <button 
                class="p-1.5 text-nx-text-secondary"
                title="Delete"
                onclick={() => deleteTask(task.id)}>
                <span class="material-symbols-outlined text-lg">delete</span>
              </button>
            </div>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<!-- Add Task Modal -->
{#if showAddModal}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onclick={() => showAddModal = false}>
    <div class="w-full max-w-lg border border-nx-border bg-nx-surface p-6" onclick={(e) => e.stopPropagation()}>
      <h2 class="mb-4 text-lg font-semibold text-nx-text">Add New Task</h2>
      
      <div class="space-y-4">
        <!-- Task Name -->
        <div>
          <label class="mb-1 block text-xs text-nx-text-muted">Task Name *</label>
          <input
            type="text"
            bind:value={taskName}
            placeholder="My Automated Task"
            class="w-full border border-nx-border bg-nx-bg px-3 py-2 text-sm text-nx-text placeholder:text-nx-text-muted outline-none focus:border-nx-text-secondary"
          />
        </div>

        <!-- Cron Expression -->
        <div>
          <label class="mb-1 block text-xs text-nx-text-muted">Cron Expression *</label>
          <input
            type="text"
            bind:value={cronExpression}
            placeholder="0 */6 * * *"
            class="w-full border border-nx-border bg-nx-bg px-3 py-2 text-sm text-nx-text placeholder:text-nx-text-muted outline-none focus:border-nx-text-secondary font-mono"
          />
          <div class="mt-2 flex flex-wrap gap-2">
            {#each presets as preset}
              <button
                class="border border-nx-border bg-nx-bg px-2 py-1 text-xs text-nx-text-secondary"
                onclick={() => cronExpression = preset.value}>
                {preset.label}
              </button>
            {/each}
          </div>
        </div>

        <!-- Task Type -->
        <div>
          <label class="mb-1 block text-xs text-nx-text-muted">Task Type</label>
          <select
            bind:value={taskType}
            class="w-full border border-nx-border bg-nx-bg px-3 py-2 text-sm text-nx-text outline-none focus:border-nx-text-secondary">
            {#each taskTypes as type}
              <option value={type.value}>{type.label}</option>
            {/each}
          </select>
        </div>

        <!-- Script Content (for script type) -->
        {#if taskType === 'script'}
          <div>
            <label class="mb-1 block text-xs text-nx-text-muted">Script Content</label>
            <textarea
              bind:value={scriptContent}
              placeholder="#!/bin/bash\necho 'Hello World'"
              rows="6"
              class="w-full border border-nx-border bg-nx-bg px-3 py-2 text-sm text-nx-text placeholder:text-nx-text-muted outline-none focus:border-nx-text-secondary font-mono"></textarea>
          </div>
        {/if}

        <!-- Command (for command type) -->
        {#if taskType === 'command'}
          <div>
            <label class="mb-1 block text-xs text-nx-text-muted">Shell Command</label>
            <input
              type="text"
              bind:value={command}
              placeholder="curl https://example.com/api/backup"
              class="w-full border border-nx-border bg-nx-bg px-3 py-2 text-sm text-nx-text placeholder:text-nx-text-muted outline-none focus:border-nx-text-secondary font-mono"
            />
          </div>
        {/if}
      </div>

      <!-- Actions -->
      <div class="mt-6 flex justify-end gap-3">
        <button
          class="border border-nx-border bg-nx-bg px-4 py-2 text-sm font-medium text-nx-text-secondary"
          onclick={() => showAddModal = false}>
          Cancel
        </button>
        <button
          class="bg-nx-text px-4 py-2 text-sm font-medium text-nx-deep"
          onclick={addTask}>
          Create Task
        </button>
      </div>
    </div>
  </div>
{/if}

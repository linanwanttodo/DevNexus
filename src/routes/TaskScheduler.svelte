<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { showToast } from "../lib/toast.svelte.js";
  import { showConfirm } from "../lib/confirm.svelte.js";
  import { t } from "../lib/i18n.svelte.js";

  let tasks = $state([]);
  let loading = $state(true);
  let showAddModal = $state(false);
  let editingTask = $state(null);

  // 左侧选中
  let selectedTaskId = $state(null);

  // 右侧面板
  let activeTab = $state("logs"); // "logs" | "edit"
  let taskLogs = $state([]);
  let selectedLog = $state(null);

  // 表单
  let taskName = $state("");
  let cronExpression = $state("0 */6 * * *");
  let taskType = $state("shell");
  let content = $state("");
  let timeoutSecs = $state(300);

  const taskTypes = [
    { value: "shell", label: t('scheduler.shell'), icon: "terminal" },
    { value: "python", label: t('scheduler.python'), icon: "code" },
    { value: "shutdown", label: t('scheduler.shutdown'), icon: "power_settings_new" },
  ];

  const cronPresets = [
    { label: t('scheduler.every_minute'), value: "* * * * *" },
    { label: t('scheduler.every_hour'), value: "0 * * * *" },
    { label: t('scheduler.every_6h'), value: "0 */6 * * *" },
    { label: t('scheduler.daily_midnight'), value: "0 0 * * *" },
    { label: t('scheduler.weekly_monday'), value: "0 0 * * 1" },
    { label: t('scheduler.monthly_1st'), value: "0 0 1 * *" },
  ];

  // ============ Task CRUD ============

  async function loadTasks() {
    try {
      loading = true;
      tasks = await invoke("list_tasks");
    } catch (err) {
      console.error("Failed to load tasks:", err);
      showToast(t('scheduler.failed_load'));
    } finally {
      loading = false;
    }
  }

  async function addTask() {
    if (!taskName || !cronExpression) {
      showToast(t('scheduler.fill_fields'));
      return;
    }
    // shell/python 必须有内容
    if ((taskType === "shell" || taskType === "python") && !content) {
      showToast(t('scheduler.fill_fields'));
      return;
    }
    try {
      await invoke("add_task", {
        name: taskName,
        cronExpression,
        taskType,
        content: (taskType === "shell" || taskType === "python") ? content : null,
        timeoutSecs: timeoutSecs || 300,
      });
      showAddModal = false;
      resetForm();
      await loadTasks();
      showToast(t('scheduler.add_success'));
    } catch (err) {
      showToast(t('scheduler.add_failed').replace('{error}', err.message || err));
    }
  }

  async function updateTaskData() {
    if (!editingTask) return;
    try {
      await invoke("update_task", {
        taskId: editingTask.id,
        name: taskName || undefined,
        cronExpression: cronExpression || undefined,
        taskType: taskType || undefined,
        content: (taskType === "shell" || taskType === "python") ? (content || undefined) : undefined,
        timeoutSecs: timeoutSecs || undefined,
      });
      editingTask = null;
      activeTab = "logs";
      await loadTasks();
      showToast(t('scheduler.update_success'));
    } catch (err) {
      showToast(t('scheduler.update_failed').replace('{error}', err.message || err));
    }
  }

  async function deleteTask(id) {
    if (!await showConfirm(t('scheduler.delete_confirm'))) return;
    try {
      await invoke("delete_task", { taskId: id });
      if (selectedTaskId === id) {
        selectedTaskId = null;
        taskLogs = [];
        selectedLog = null;
      }
      await loadTasks();
      showToast(t('scheduler.delete_success'));
    } catch (err) {
      showToast(t('scheduler.delete_failed').replace('{error}', err.message || err));
    }
  }

  async function toggleTask(id) {
    try {
      await invoke("toggle_task", { taskId: id });
      await loadTasks();
    } catch (err) {
      showToast(t('scheduler.toggle_failed').replace('{error}', err.message || err));
    }
  }

  async function executeTask(id) {
    if (!await showConfirm(t('scheduler.execute_confirm'))) return;
    try {
      const result = await invoke("execute_task", { taskId: id });
      showToast(t('scheduler.execute_success').replace('{result}', result));
      await loadTasks();
      if (selectedTaskId === id) {
        await loadLogs(id);
      }
    } catch (err) {
      showToast(t('scheduler.execute_failed').replace('{error}', err.message || err));
      await loadTasks();
    }
  }

  // ============ Logs ============

  async function loadLogs(taskId) {
    try {
      taskLogs = await invoke("get_task_logs", { taskId });
      selectedLog = null;
    } catch (err) {
      console.error("Failed to load logs:", err);
    }
  }

  async function clearLogs(taskId) {
    if (!await showConfirm(t('scheduler.clear_logs_confirm'))) return;
    try {
      await invoke("clear_task_logs", { taskId });
      taskLogs = [];
      selectedLog = null;
      showToast(t('scheduler.clear_logs_success'));
    } catch (err) {
      showToast(t('scheduler.clear_logs_failed').replace('{error}', err.message || err));
    }
  }

  // ============ Selection ============

  function selectTask(task) {
    selectedTaskId = task.id;
    activeTab = "logs";
    loadLogs(task.id);
  }

  function openAddModal() {
    resetForm();
    showAddModal = true;
  }

  function openEditModal(task) {
    editingTask = task;
    taskName = task.name;
    cronExpression = task.cron_expression;
    taskType = task.task_type === "python" ? "python" : (task.task_type === "shutdown" ? "shutdown" : "shell");
    content = task.content || "";
    timeoutSecs = task.timeout_secs;
    activeTab = "edit";
  }

  function switchToEdit(task) {
    activeTab = 'edit';
    taskName = task.name;
    cronExpression = task.cron_expression;
    taskType = task.task_type === 'python' ? 'python' : (task.task_type === 'shutdown' ? 'shutdown' : 'shell');
    content = task.content || '';
    timeoutSecs = task.timeout_secs;
  }

  function resetForm() {
    taskName = "";
    cronExpression = "0 */6 * * *";
    taskType = "shell";
    content = "";
    timeoutSecs = 300;
  }

  // ============ Helpers ============

  function statusLabel(status) {
    switch (status) {
      case "idle": return t('scheduler.status_idle');
      case "disabled": return t('scheduler.status_disabled');
      default: return status;
    }
  }

  function statusColor(status) {
    switch (status) {
      case "idle": return "bg-nx-accent/15 text-nx-accent";
      case "disabled": return "bg-nx-text-muted/15 text-nx-text-muted";
      default: return "bg-nx-text-muted/15 text-nx-text-muted";
    }
  }

  function logStatusColor(status) {
    switch (status) {
      case "success": return "text-nx-success";
      case "failed": return "text-nx-error";
      case "timeout": return "text-nx-warning";
      default: return "text-nx-text-muted";
    }
  }

  function logStatusLabel(status) {
    switch (status) {
      case "success": return t('scheduler.log_success');
      case "failed": return t('scheduler.log_failed');
      case "timeout": return t('scheduler.log_timeout');
      default: return status;
    }
  }

  function taskTypeLabel(type) {
    switch (type) {
      case "python": return "Python";
      case "shutdown": return t('scheduler.shutdown');
      default: return "Shell";
    }
  }

  function taskTypeIcon(type) {
    switch (type) {
      case "python": return "code";
      case "shutdown": return "power_settings_new";
      default: return "terminal";
    }
  }

  onMount(() => {
    loadTasks();
  });
</script>

<div class="mx-auto h-full p-5">
  <!-- Header -->
  <div class="mb-4 flex items-center justify-between">
    <div>
      <h1 class="text-xl font-semibold text-nx-text">{t('scheduler.title')}</h1>
      <p class="mt-1 text-xs text-nx-text-muted">{t('scheduler.desc')}</p>
    </div>
    <button
      class="nx-btn nx-btn-primary flex items-center gap-2"
      onclick={openAddModal}>
      <span class="material-symbols-outlined text-lg">add</span>
      {t('scheduler.add')}
    </button>
  </div>

  <!-- Main Layout: Left Task List + Right Detail -->
  <div class="flex gap-4" style="height: calc(100vh - 160px);">

    <!-- Left: Task List -->
    <div class="w-96 flex-shrink-0 nx-section overflow-hidden flex flex-col">
      <div class="nx-section-header">
        <span class="text-sm font-medium text-nx-text">{t('scheduler.task_list')}</span>
        <span class="ml-2 text-xs text-nx-text-muted">({tasks.length})</span>
      </div>

      <div class="flex-1 overflow-y-auto">
        {#if loading}
          <div class="flex items-center justify-center py-12">
            <span class="material-symbols-outlined nx-animate-spin text-nx-text-muted text-3xl">progress_activity</span>
          </div>
        {:else if tasks.length === 0}
          <div class="nx-empty p-12">
            <span class="material-symbols-outlined text-nx-text-muted text-4xl">schedule</span>
            <div class="mt-4 text-sm text-nx-text-muted">{t('scheduler.no_tasks')}</div>
            <div class="mt-1 text-xs text-nx-text-muted">{t('scheduler.no_tasks_desc')}</div>
          </div>
        {:else}
          {#each tasks as task}
            <div
              role="button" tabindex="0"
              class="border-b border-nx-border px-4 py-3 cursor-pointer transition-colors
                     {selectedTaskId === task.id ? 'bg-nx-accent/10' : 'hover:bg-nx-bg'}"
              onclick={() => selectTask(task)}
              onkeydown={(e) => e.key === 'Enter' && selectTask(task)}>
              <div class="flex items-center justify-between">
                <div class="flex-1 min-w-0">
                  <div class="flex items-center gap-2">
                    <span class="text-sm font-medium text-nx-text truncate">{task.name}</span>
                    <span class="nx-pill px-1.5 py-0.5 text-[10px] font-medium rounded {statusColor(task.status)}">
                      {statusLabel(task.status)}
                    </span>
                  </div>
                  <div class="mt-1 flex items-center gap-3 text-[11px] text-nx-text-secondary">
                    <code class="font-mono">{task.cron_expression}</code>
                    <span class="flex items-center gap-0.5 bg-nx-text-secondary/10 px-1.5 py-0.5 rounded">
                      <span class="material-symbols-outlined text-[10px]">{taskTypeIcon(task.task_type)}</span>
                      {taskTypeLabel(task.task_type)}
                    </span>
                  </div>
                  <div class="mt-1 flex items-center gap-3 text-[11px] text-nx-text-muted">
                    {#if task.next_run}
                      <span>{t('scheduler.next')}: {task.next_run}</span>
                    {/if}
                    <span>{task.run_count} {t('scheduler.runs')}</span>
                  </div>
                </div>
              </div>
            </div>
          {/each}
        {/if}
      </div>
    </div>

    <!-- Right: Detail Panel -->
    <div class="flex-1 nx-section overflow-hidden flex flex-col">
      {#if selectedTaskId === null}
        <div class="flex flex-1 items-center justify-center text-nx-text-muted">
          <div class="nx-empty">
            <span class="material-symbols-outlined text-5xl mb-2">touch_app</span>
            <div class="text-sm">{t('scheduler.select_task_hint')}</div>
          </div>
        </div>
      {:else}
        {#each tasks as task}
          {#if task.id === selectedTaskId}
            <!-- Task Header -->
            <div class="nx-section-header">
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-3">
                  <h3 class="text-sm font-semibold text-nx-text">{task.name}</h3>
                  <span class="nx-pill px-2 py-0.5 text-xs font-medium rounded {statusColor(task.status)}">
                    {statusLabel(task.status)}
                  </span>
                  {#if task.last_status}
                    <span class="text-xs text-nx-text-muted">
                      {t('scheduler.last_result')}:
                      <span class={logStatusColor(task.last_status)}>{logStatusLabel(task.last_status)}</span>
                    </span>
                  {/if}
                </div>
                <div class="flex items-center gap-1">
                  <button
                    class="p-1.5 text-nx-text-secondary hover:text-nx-accent"
                    title={t('scheduler.execute_now')}
                    onclick={() => executeTask(task.id)}>
                    <span class="material-symbols-outlined text-lg">play_arrow</span>
                  </button>
                  <button
                    class="p-1.5 text-nx-text-secondary hover:text-nx-text"
                    title={t('scheduler.edit')}
                    onclick={() => openEditModal(task)}>
                    <span class="material-symbols-outlined text-lg">edit</span>
                  </button>
                  <button
                    class="p-1.5 text-nx-text-secondary"
                    title={task.status === 'disabled' ? t('scheduler.enable') : t('scheduler.disable')}
                    onclick={() => toggleTask(task.id)}>
                    <span class="material-symbols-outlined text-lg">
                      {task.status === 'disabled' ? 'play_circle' : 'pause_circle'}
                    </span>
                  </button>
                  <button
                    class="p-1.5 text-nx-text-secondary hover:text-nx-error"
                    title={t('common.delete')}
                    onclick={() => deleteTask(task.id)}>
                    <span class="material-symbols-outlined text-lg">delete</span>
                  </button>
                </div>
              </div>
              <div class="mt-2 flex items-center gap-4 text-xs text-nx-text-secondary">
                <div class="flex items-center gap-1">
                  <span class="material-symbols-outlined text-sm">schedule</span>
                  <code class="font-mono">{task.cron_expression}</code>
                </div>
                {#if task.last_run}
                  <div class="flex items-center gap-1">
                    <span class="material-symbols-outlined text-sm">history</span>
                    <span>{t('scheduler.last')}: {task.last_run}</span>
                  </div>
                {/if}
                <div class="flex items-center gap-1">
                  <span class="material-symbols-outlined text-sm">timer</span>
                  <span>{t('scheduler.timeout')}: {task.timeout_secs}s</span>
                </div>
              </div>
            </div>

            <!-- Tabs -->
            <div class="flex border-b border-nx-border">
              <button
                class="px-4 py-2 text-sm font-medium border-b-2 transition-colors
                       {activeTab === 'logs' ? 'border-nx-accent text-nx-accent' : 'border-transparent text-nx-text-muted hover:text-nx-text'}"
                onclick={() => activeTab = 'logs'}>
                {t('scheduler.execution_logs')}
              </button>
              <button
                class="px-4 py-2 text-sm font-medium border-b-2 transition-colors
                       {activeTab === 'edit' ? 'border-nx-accent text-nx-accent' : 'border-transparent text-nx-text-muted hover:text-nx-text'}"
                onclick={() => switchToEdit(task)}>
                {t('scheduler.edit')}
              </button>
            </div>

            <!-- Tab Content -->
            <div class="flex-1 overflow-hidden">
              {#if activeTab === 'logs'}
                <div class="flex h-full">
                  <!-- Log List -->
                  <div class="w-64 border-r border-nx-border overflow-y-auto">
                    <div class="flex items-center justify-between border-b border-nx-border px-3 py-2">
                      <span class="text-xs font-medium text-nx-text">{t('scheduler.history')}</span>
                      {#if taskLogs.length > 0}
                        <button
                          class="text-[10px] text-nx-text-muted hover:text-nx-error"
                          onclick={() => clearLogs(task.id)}>
                          {t('scheduler.clear')}
                        </button>
                      {/if}
                    </div>
                    {#if taskLogs.length === 0}
                      <div class="p-6 text-center text-xs text-nx-text-muted">
                        {t('scheduler.no_logs')}
                      </div>
                    {:else}
                      {#each taskLogs as log}
                        <div
                          role="button" tabindex="0"
                          class="border-b border-nx-border px-3 py-2 cursor-pointer transition-colors
                                 {selectedLog && selectedLog.id === log.id ? 'bg-nx-accent/10' : 'hover:bg-nx-bg'}"
                          onclick={() => selectedLog = log}
                          onkeydown={(e) => e.key === 'Enter' && (selectedLog = log)}>
                          <div class="flex items-center justify-between">
                            <span class="text-xs font-medium text-nx-text">#{log.id}</span>
                            <span class="text-[10px] {logStatusColor(log.status)}">{logStatusLabel(log.status)}</span>
                          </div>
                          <div class="mt-1 text-[10px] text-nx-text-muted">
                            {log.started_at}
                            {#if log.duration_ms}
                              · {(log.duration_ms / 1000).toFixed(1)}s
                            {/if}
                          </div>
                        </div>
                      {/each}
                    {/if}
                  </div>

                  <!-- Log Detail -->
                  <div class="flex-1 overflow-y-auto">
                    {#if selectedLog === null}
                      <div class="flex h-full items-center justify-center text-nx-text-muted text-sm">
                        {t('scheduler.select_log_hint')}
                      </div>
                    {:else}
                      <div class="p-4">
                        <div class="mb-3 flex items-center gap-4 text-xs">
                          <span class="font-medium text-nx-text">{t('scheduler.status')}:
                            <span class={logStatusColor(selectedLog.status)}>{logStatusLabel(selectedLog.status)}</span>
                          </span>
                          {#if selectedLog.exit_code !== null}
                            <span>{t('scheduler.exit_code')}: {selectedLog.exit_code}</span>
                          {/if}
                          {#if selectedLog.duration_ms}
                            <span>{t('scheduler.duration')}: {(selectedLog.duration_ms / 1000).toFixed(1)}s</span>
                          {/if}
                        </div>

                        {#if selectedLog.stdout}
                          <div class="mb-3">
                            <div class="mb-1 text-xs font-medium text-nx-text">{t('scheduler.stdout')}</div>
                            <pre class="bg-nx-bg border border-nx-border p-3 text-xs font-mono whitespace-pre-wrap text-nx-text overflow-x-auto">{selectedLog.stdout}</pre>
                          </div>
                        {/if}

                        {#if selectedLog.stderr}
                          <div class="mb-3">
                            <div class="mb-1 text-xs font-medium text-nx-text">{t('scheduler.stderr')}</div>
                            <pre class="bg-nx-bg border border-nx-error/20 p-3 text-xs font-mono whitespace-pre-wrap text-nx-error overflow-x-auto">{selectedLog.stderr}</pre>
                          </div>
                        {/if}

                        {#if !selectedLog.stdout && !selectedLog.stderr}
                          <div class="text-xs text-nx-text-muted">{t('scheduler.no_output')}</div>
                        {/if}
                      </div>
                    {/if}
                  </div>
                </div>
              {:else if activeTab === 'edit'}
                <div class="p-4 space-y-4 overflow-y-auto" style="max-height: calc(100vh - 280px);">
                  <!-- Task Name -->
                  <div>
                    <label class="mb-1 block text-xs text-nx-text-muted" for="sched-name-edit">{t('scheduler.name')} *</label>
                    <input
                      type="text"
                      id="sched-name-edit"
                      bind:value={taskName}
                      class="nx-input w-full"
                    />
                  </div>

                  <!-- Cron -->
                  <div>
                    <label class="mb-1 block text-xs text-nx-text-muted" for="sched-cron-edit">{t('scheduler.cron')} *</label>
                    <input
                      type="text"
                      id="sched-cron-edit"
                      bind:value={cronExpression}
                      class="nx-input w-full font-mono"
                    />
                    <div class="mt-2 flex flex-wrap gap-2">
                      {#each cronPresets as preset}
                        <button
                          class="nx-btn nx-btn-ghost px-2 py-1 text-xs"
                          onclick={() => cronExpression = preset.value}>
                          {preset.label}
                        </button>
                      {/each}
                    </div>
                  </div>

                  <!-- Task Type -->
                  <div>
                    <div class="mb-1 block text-xs text-nx-text-muted">{t('scheduler.type')}</div>
                    <div class="flex gap-2" role="radiogroup" aria-label={t('scheduler.type')}>
                      {#each taskTypes as tt}
                        <button
                          class="flex items-center gap-1.5 border px-3 py-2 text-sm transition-colors
                                 {taskType === tt.value ? 'border-nx-accent bg-nx-accent/10 text-nx-accent' : 'border-nx-border bg-nx-bg text-nx-text-secondary hover:border-nx-text-secondary'}"
                          onclick={() => taskType = tt.value}>
                          <span class="material-symbols-outlined text-sm">{tt.icon}</span>
                          {tt.label}
                        </button>
                      {/each}
                    </div>
                  </div>

                  <!-- Content (only for shell/python) -->
                  {#if taskType === 'shell' || taskType === 'python'}
                    <div>
                      <label class="mb-1 block text-xs text-nx-text-muted" for="sched-content-edit">{t('scheduler.content')} *</label>
                      <textarea
                        id="sched-content-edit"
                        bind:value={content}
                        rows="10"
                        class="nx-input w-full font-mono"
                      ></textarea>
                    </div>
                  {:else}
                    <div class="nx-card p-4 text-sm text-nx-text-muted">
                      <span class="material-symbols-outlined text-lg align-middle mr-1">info</span>
                      {t('scheduler.shutdown_hint')}
                    </div>
                  {/if}

                  <!-- Timeout (only for shell/python) -->
                  {#if taskType === 'shell' || taskType === 'python'}
                    <div>
                      <label class="mb-1 block text-xs text-nx-text-muted" for="sched-timeout-edit">{t('scheduler.timeout_secs')}</label>
                      <input
                        type="number"
                        id="sched-timeout-edit"
                        bind:value={timeoutSecs}
                        min="1"
                        max="3600"
                        class="nx-input w-32"
                      />
                    </div>
                  {/if}

                  <!-- Actions -->
                  <div class="flex gap-3 pt-2">
                    <button
                      class="nx-btn nx-btn-primary"
                      onclick={updateTaskData}>
                      {t('scheduler.save')}
                    </button>
                    <button
                      class="nx-btn nx-btn-ghost"
                      onclick={() => { editingTask = null; activeTab = 'logs'; }}
                    >
                      {t('scheduler.cancel')}
                    </button>
                  </div>
                </div>
              {/if}
            </div>
          {/if}
        {/each}
      {/if}
    </div>
  </div>
</div>

<!-- Add Task Modal -->
{#if showAddModal}
  <div class="nx-dialog-overlay" role="button" tabindex="0" onkeydown={(e) => e.key === 'Escape' && (showAddModal = false)} onclick={() => showAddModal = false}>
    <div class="nx-dialog max-h-[90vh] overflow-y-auto" role="dialog" aria-modal="true" tabindex="-1" onkeydown={(e) => e.stopPropagation()} onclick={(e) => e.stopPropagation()}>
      <div class="nx-dialog-header">
        <h2 class="text-lg font-semibold text-nx-text">{t('scheduler.add_new_task')}</h2>
      </div>

      <div class="nx-dialog-body space-y-4">
        <div>
          <label class="mb-1 block text-xs text-nx-text-muted" for="sched-name-add">{t('scheduler.name')} *</label>
          <input
            type="text"
            id="sched-name-add"
            bind:value={taskName}
            class="nx-input w-full"
          />
        </div>

        <div>
          <label class="mb-1 block text-xs text-nx-text-muted" for="sched-cron-add">{t('scheduler.cron')} *</label>
          <input
            type="text"
            id="sched-cron-add"
            bind:value={cronExpression}
            class="nx-input w-full font-mono"
          />
          <div class="mt-2 flex flex-wrap gap-2">
            {#each cronPresets as preset}
              <button
                class="nx-btn nx-btn-ghost px-2 py-1 text-xs"
                onclick={() => cronExpression = preset.value}>
                {preset.label}
              </button>
            {/each}
          </div>
        </div>

        <div>
          <div class="mb-1 block text-xs text-nx-text-muted">{t('scheduler.type')}</div>
          <div class="flex gap-2" role="radiogroup" aria-label={t('scheduler.type')}>
            {#each taskTypes as tt}
              <button
                class="flex items-center gap-1.5 border px-3 py-2 text-sm
                       {taskType === tt.value ? 'border-nx-accent bg-nx-accent/10 text-nx-accent' : 'border-nx-border bg-nx-bg text-nx-text-secondary'}"
                onclick={() => taskType = tt.value}>
                <span class="material-symbols-outlined text-sm">{tt.icon}</span>
                {tt.label}
              </button>
            {/each}
          </div>
        </div>

        {#if taskType === 'shell' || taskType === 'python'}
          <div>
            <label class="mb-1 block text-xs text-nx-text-muted" for="sched-content-add">{t('scheduler.content')} *</label>
            <textarea
              id="sched-content-add"
              bind:value={content}
              rows="8"
              class="nx-input w-full font-mono"
            ></textarea>
          </div>
          <div>
            <label class="mb-1 block text-xs text-nx-text-muted" for="sched-timeout-add">{t('scheduler.timeout_secs')}</label>
            <input
              type="number"
              id="sched-timeout-add"
              bind:value={timeoutSecs}
              min="1"
              max="3600"
              class="nx-input w-32"
            />
          </div>
        {:else}
          <div class="nx-card p-4 text-sm text-nx-text-muted">
            <span class="material-symbols-outlined text-lg align-middle mr-1">info</span>
            {t('scheduler.shutdown_hint')}
          </div>
        {/if}
      </div>

      <div class="nx-dialog-footer">
        <button
          class="nx-btn nx-btn-ghost"
          onclick={() => showAddModal = false}>
          {t('scheduler.cancel')}
        </button>
        <button
          class="nx-btn nx-btn-primary"
          onclick={addTask}>
          {t('scheduler.create')}
        </button>
      </div>
    </div>
  </div>
{/if}

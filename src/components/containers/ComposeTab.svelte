<script>
  import { t } from "../../lib/i18n.svelte.js";
  import ContainerIcons from "../../icons/ContainerIcons.svelte";

  let {
    file = "",
    project = "",
    loading = false,
    error = null,
    containers = [],
    logs = "",
    onFileInput,
    onProjectInput,
    onUp,
    onDown,
    onPs,
    onLogs,
    onClearLogs,
  } = $props();

  function statusLabel(status) {
    const map = {
      running: t("docker.status_running"), exited: t("docker.status_exited"),
      paused: t("docker.status_paused"), created: t("docker.status_created"),
    };
    return map[status] || status;
  }
</script>

<div>
  <div class="mb-4 grid grid-cols-2 gap-3">
    <div>
      <label for="compose-file" class="mb-1.5 block text-xs text-nx-text-muted">{t("docker.compose_file")}</label>
      <input id="compose-file" type="text" value={file} oninput={(e) => onFileInput(e.currentTarget.value)} placeholder="docker-compose.yml"
        class="nx-input w-full h-9 text-sm" />
    </div>
    <div>
      <label for="compose-project" class="mb-1.5 block text-xs text-nx-text-muted">{t("docker.compose_project")}</label>
      <input id="compose-project" type="text" value={project} oninput={(e) => onProjectInput(e.currentTarget.value)} placeholder={t("docker.compose_project_ph")}
        class="nx-input w-full h-9 text-sm" />
    </div>
  </div>

  <div class="mb-4 flex items-center gap-2">
    <button class="nx-btn text-xs h-8 text-nx-success" onclick={onUp} disabled={loading}>
      <span class="material-symbols-outlined text-sm">play_arrow</span>{t("docker.compose_up")}
    </button>
    <button class="nx-btn text-xs h-8 text-nx-danger" onclick={onDown} disabled={loading}>
      <span class="material-symbols-outlined text-sm">stop</span>{t("docker.compose_down")}
    </button>
    <button class="nx-btn text-xs h-8" onclick={onPs} disabled={loading}>
      <span class="material-symbols-outlined text-sm">list</span>{t("docker.compose_ps")}
    </button>
    <button class="nx-btn text-xs h-8" onclick={onLogs} disabled={loading}>
      <span class="material-symbols-outlined text-sm">list_alt</span>{t("docker.compose_logs")}
    </button>
  </div>

  {#if error}
    <div class="mb-4 nx-section">
      <div class="nx-section-body">
        <pre class="font-mono text-xs text-nx-danger whitespace-pre-wrap">{error}</pre>
      </div>
    </div>
  {/if}

  {#if containers.length > 0}
    <div class="mb-4 nx-section">
      <div class="nx-section-header">
        <span class="text-xs font-medium uppercase tracking-wider text-nx-text-muted">{t("docker.compose_services")}</span>
      </div>
      <table class="nx-table">
        <thead>
          <tr>
            <th>{t("docker.name")}</th>
            <th>{t("docker.image")}</th>
            <th>{t("docker.status")}</th>
            <th>{t("docker.ports")}</th>
          </tr>
        </thead>
        <tbody>
          {#each containers as c}
            <tr>
              <td class="text-sm text-nx-text">{c.name}</td>
              <td class="font-mono text-xs text-nx-text-secondary">{c.image}</td>
              <td>
                <span class="inline-flex items-center gap-1 text-xs {c.status === 'running' ? 'text-nx-success' : 'text-nx-text-muted'}">
                  <ContainerIcons name={c.status === 'running' ? 'container-running' : 'container-exited'} size={12} />
                  {statusLabel(c.status)}
                </span>
              </td>
              <td class="font-mono text-xs text-nx-text-muted">{c.ports || "-"}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}

  {#if logs}
    <div class="nx-section">
      <div class="nx-section-header">
        <span class="text-xs font-medium uppercase tracking-wider text-nx-text-muted">{t("docker.logs")}</span>
        <button class="text-nx-text-muted hover:text-nx-text" onclick={onClearLogs}>
          <span class="material-symbols-outlined text-sm">close</span>
        </button>
      </div>
      <pre class="max-h-[400px] overflow-auto p-4 font-mono text-xs text-nx-text-secondary whitespace-pre-wrap">{logs}</pre>
    </div>
  {/if}
</div>

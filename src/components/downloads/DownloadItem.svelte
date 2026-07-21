<script>
  import { createEventDispatcher } from 'svelte';

  let { download } = $props();

  const dispatch = createEventDispatcher();

  let status = $derived(download.status);
  let percentage = $derived((download.downloaded_size / download.total_size * 100).toFixed(1));
  let speed = $derived(formatSpeed(download.speed));
  let eta = $derived(download.eta_seconds ? formatETA(download.eta_seconds) : '--');
  let sizeInfo = $derived(`${formatSize(download.downloaded_size)} / ${formatSize(download.total_size)}`);

  function formatSpeed(bytesPerSecond) {
    if (!bytesPerSecond) return '0 B/s';
    if (bytesPerSecond < 1024) return `${bytesPerSecond.toFixed(1)} B/s`;
    if (bytesPerSecond < 1024 * 1024) return `${(bytesPerSecond / 1024).toFixed(1)} KB/s`;
    if (bytesPerSecond < 1024 * 1024 * 1024) return `${(bytesPerSecond / (1024 * 1024)).toFixed(1)} MB/s`;
    return `${(bytesPerSecond / (1024 * 1024 * 1024)).toFixed(1)} GB/s`;
  }

  function formatSize(bytes) {
    if (!bytes) return '0 B';
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  }

  function formatETA(seconds) {
    if (seconds < 60) return `${seconds}s`;
    if (seconds < 3600) {
      const mins = Math.floor(seconds / 60);
      const secs = seconds % 60;
      return `${mins}m ${secs}s`;
    }
    const hours = Math.floor(seconds / 3600);
    const mins = Math.floor((seconds % 3600) / 60);
    return `${hours}h ${mins}m`;
  }

  function handlePause() {
    dispatch('pause', download.id);
  }

  function handleResume() {
    dispatch('resume', download.id);
  }

  function handleCancel() {
    dispatch('cancel', download.id);
  }

  function handleDelete() {
    dispatch('delete', { taskId: download.id, deleteFile: true });
  }
</script>

<div class="download-item" class:downloading={status === 'Downloading'} class:paused={status === 'Paused'} class:completed={status === 'Completed'} class:failed={status === 'Failed'}>
  <div class="info">
    <div class="filename">{download.filename}</div>
    <div class="stats">
      <span class="percentage">{percentage}%</span>
      <span class="speed">{speed}</span>
      <span class="eta">ETA: {eta}</span>
      <span class="size">{sizeInfo}</span>
    </div>

    <div class="progress-bar">
      <div class="progress" style="width: {percentage}%"></div>
    </div>

    {#if download.error}
      <div class="error">{download.error}</div>
    {/if}
  </div>

  <div class="actions">
    {#if status === 'Downloading'}
      <button class="btn pause-btn" onclick={handlePause} title="Pause">⏸</button>
      <button class="btn cancel-btn" onclick={handleCancel} title="Cancel">✕</button>
    {:else if status === 'Paused' || status === 'Pending'}
      <button class="btn resume-btn" onclick={handleResume} title="Resume">▶</button>
      <button class="btn cancel-btn" onclick={handleCancel} title="Cancel">✕</button>
    {:else if status === 'Failed'}
      <button class="btn resume-btn" onclick={handleResume} title="Retry">↻</button>
      <button class="btn delete-btn" onclick={handleDelete} title="Delete">🗑</button>
    {:else if status === 'Completed'}
      <button class="btn delete-btn" onclick={handleDelete} title="Delete">🗑</button>
    {/if}
  </div>
</div>

<style>
  .download-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px;
    margin-bottom: 8px;
    background: #f8f9fa;
    border-radius: 8px;
    border-left: 4px solid #3b82f6;
  }

  .download-item.downloading {
    border-left-color: #3b82f6;
    background: #eff6ff;
  }

  .download-item.paused {
    border-left-color: #f59e0b;
    background: #fffbeb;
  }

  .download-item.completed {
    border-left-color: #10b981;
    background: #ecfdf5;
  }

  .download-item.failed {
    border-left-color: #ef4444;
    background: #fef2f2;
  }

  .info {
    flex: 1;
    min-width: 0;
  }

  .filename {
    font-weight: 500;
    margin-bottom: 4px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .stats {
    display: flex;
    gap: 12px;
    font-size: 12px;
    color: #666;
    margin-bottom: 4px;
  }

  .progress-bar {
    height: 4px;
    background: #e5e7eb;
    border-radius: 2px;
    overflow: hidden;
  }

  .progress {
    height: 100%;
    background: #3b82f6;
    transition: width 0.3s ease;
  }

  .error {
    margin-top: 4px;
    font-size: 12px;
    color: #ef4444;
  }

  .actions {
    display: flex;
    gap: 4px;
    margin-left: 12px;
  }

  .btn {
    width: 32px;
    height: 32px;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    background: transparent;
  }

  .pause-btn:hover { background: #fee2e2; }
  .resume-btn:hover { background: #dbeafe; }
  .cancel-btn:hover { background: #fee2e2; }
  .delete-btn:hover { background: #fee2e2; }
</style>

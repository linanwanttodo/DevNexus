<script>
  import { formatSpeed, formatBytes } from '../../lib/downloads.svelte.js';

  let { totalDownloads = 0, activeDownloads = 0, totalSpeed = 0 } = $props();
</script>

<div class="download-stats">
  <div class="stat-item">
    <div class="stat-value">{totalDownloads}</div>
    <div class="stat-label">Total Downloads</div>
  </div>
  <div class="stat-item">
    <div class="stat-value">{activeDownloads}</div>
    <div class="stat-label">Active</div>
  </div>
  <div class="stat-item">
    <div class="stat-value">{formatSpeed(totalSpeed)}</div>
    <div class="stat-label">Total Speed</div>
  </div>
</div>

<style>
  .download-stats {
    display: flex;
    gap: 20px;
    margin-bottom: 20px;
  }

  .stat-item {
    flex: 1;
    padding: 15px;
    background: #f7fafc;
    border-radius: 8px;
    text-align: center;
  }

  .stat-value {
    font-size: 24px;
    font-weight: bold;
    color: #2d3748;
  }

  .stat-label {
    font-size: 14px;
    color: #718096;
    margin-top: 5px;
  }
</style>

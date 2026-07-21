<script>
  import { DownloadProgress } from '../../lib/downloads';

  let { percentage = 0, speed = 0, eta = null } = $props();

  function formatSpeed(bytesPerSecond) {
    return DownloadProgress.formatSpeed(bytesPerSecond);
  }

  function formatEta(seconds) {
    if (!seconds) return '--:--';
    return DownloadProgress.formatEta(seconds);
  }
</script>

<div class="progress-container">
  <div class="progress-bar">
    <div class="progress-fill" style="width: {percentage}%"></div>
  </div>

  <div class="progress-info">
    <span class="percentage">{percentage.toFixed(1)}%</span>
    <span class="speed">{formatSpeed(speed)}</span>
    <span class="eta">ETA: {formatEta(eta)}</span>
  </div>
</div>

<style>
  .progress-container {
    width: 100%;
  }

  .progress-bar {
    height: 4px;
    background: #333;
    border-radius: 2px;
    overflow: hidden;
    margin-bottom: 8px;
  }

  .progress-fill {
    height: 100%;
    background: #3b82f6;
    transition: width 0.3s ease;
  }

  .progress-info {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
    color: #999;
  }
</style>

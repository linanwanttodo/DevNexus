<script>
  import { createEventDispatcher } from 'svelte';
  import DownloadItem from './DownloadItem.svelte';

  let { downloads = [] } = $props();

  const dispatch = createEventDispatcher();
</script>

<div class="download-list">
  {#each downloads as download (download.id)}
    <DownloadItem
      {download}
      on:pause={(e) => dispatch('pause', e.detail)}
      on:resume={(e) => dispatch('resume', e.detail)}
      on:cancel={(e) => dispatch('cancel', e.detail)}
      on:delete={(e) => dispatch('delete', e.detail)}
    />
  {/each}

  {#if downloads.length === 0}
    <div class="empty-state">
      <p>No downloads yet. Click "Add Download" to start.</p>
    </div>
  {/if}
</div>

<style>
  .download-list {
    flex: 1;
    overflow-y: auto;
  }

  .empty-state {
    display: flex;
    justify-content: center;
    align-items: center;
    height: 200px;
    color: #666;
  }
</style>

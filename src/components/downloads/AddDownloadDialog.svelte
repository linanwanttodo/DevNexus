<script>
  import { createEventDispatcher } from 'svelte';
  
  export let url = '';
  export let savePath = '';
  
  const dispatch = createEventDispatcher();
  
  function handleSubmit() {
    if (url.trim()) {
      dispatch('add', { url: url.trim(), savePath: savePath.trim() || null });
    }
  }
  
  function handleKeydown(e) {
    if (e.key === 'Enter') {
      handleSubmit();
    }
  }
</script>

<div class="dialog-overlay" on:click={() => dispatch('close')}>
  <div class="dialog" on:click|stopPropagation>
    <h2>Add Download</h2>
    
    <div class="form-group">
      <label>URL</label>
      <input 
        type="text" 
        bind:value={url} 
        placeholder="https://example.com/file.zip"
        on:keydown={handleKeydown}
        autofocus
      />
    </div>
    
    <div class="form-group">
      <label>Save Path (optional)</label>
      <input 
        type="text" 
        bind:value={savePath} 
        placeholder="Leave empty for default download folder"
      />
    </div>
    
    <div class="actions">
      <button class="cancel-btn" on:click={() => dispatch('close')}>Cancel</button>
      <button class="add-btn" on:click={handleSubmit}>Start Download</button>
    </div>
  </div>
</div>

<style>
  .dialog-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }
  
  .dialog {
    background: #1e1e1e;
    border-radius: 8px;
    padding: 20px;
    width: 90%;
    max-width: 500px;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
  }
  
  h2 {
    margin: 0 0 20px 0;
    color: #ffffff;
  }
  
  .form-group {
    margin-bottom: 15px;
  }
  
  label {
    display: block;
    margin-bottom: 5px;
    color: #cccccc;
  }
  
  input {
    width: 100%;
    padding: 8px 12px;
    background: #2d2d2d;
    border: 1px solid #444;
    border-radius: 4px;
    color: #ffffff;
    font-size: 14px;
  }
  
  input:focus {
    outline: none;
    border-color: #3b82f6;
  }
  
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 20px;
  }
  
  button {
    padding: 8px 16px;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-size: 14px;
  }
  
  .cancel-btn {
    background: #4b5563;
    color: white;
  }
  
  .add-btn {
    background: #3b82f6;
    color: white;
  }
  
  .add-btn:hover {
    background: #2563eb;
  }
</style>
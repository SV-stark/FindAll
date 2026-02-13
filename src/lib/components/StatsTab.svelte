<script lang="ts">
  import { fade } from "svelte/transition";
  import { appState } from '$lib/state.svelte';
  import { formatBytes } from '$lib/types';
</script>

<div class="stats-tab" in:fade={{ duration: 200 }}>
  <div class="stats-header">
    <h1>📊 Index Statistics</h1>
    <button class="btn-secondary" onclick={() => appState.loadStatistics()}>🔄 Refresh</button>
  </div>
  
  {#if appState.indexStats}
    <div class="stats-grid">
      <div class="stat-card">
        <div class="stat-icon">📄</div>
        <div class="stat-value">{appState.indexStats.total_documents.toLocaleString()}</div>
        <div class="stat-label">Files Indexed</div>
      </div>
      <div class="stat-card">
        <div class="stat-icon">💾</div>
        <div class="stat-value">{formatBytes(appState.indexStats.total_size_bytes)}</div>
        <div class="stat-label">Total Size</div>
      </div>
      <div class="stat-card">
        <div class="stat-icon">📌</div>
        <div class="stat-value">{appState.pinnedFiles.length}</div>
        <div class="stat-label">Pinned Files</div>
      </div>
    </div>
    
    <div class="stats-section">
      <h3>Quick Actions</h3>
      <div class="stats-actions">
        <button class="btn-primary" onclick={() => appState.startIndexing()}>
          ⚡ Rebuild Index
        </button>
        <button class="btn-secondary" onclick={() => appState.activeTab = "search"}>
          🔍 Start Searching
        </button>
      </div>
    </div>
  {:else}
    <div class="stats-loading">
      <div class="spinner-large"></div>
      <p>Loading statistics...</p>
    </div>
  {/if}
</div>

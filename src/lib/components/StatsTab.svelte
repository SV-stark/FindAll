<script lang="ts">
  import { fade } from "svelte/transition";
  import { appState } from '$lib/state.svelte';
  import { formatBytes } from '$lib/types';
  import Icon from './Icon.svelte';
</script>

<div class="stats-tab" in:fade={{ duration: 200 }}>
  <div class="stats-header">
    <h1>
      <Icon icon="chart" size={24} color="var(--primary)" />
      <span>Index Statistics</span>
    </h1>
    <button class="btn-secondary" onclick={() => appState.loadStatistics()}>
      <Icon icon="refresh" size={16} />
      <span>Refresh</span>
    </button>
  </div>
  
  {#if appState.indexStats}
    <div class="stats-grid">
      <div class="stat-card">
        <div class="stat-icon">
          <Icon icon="file" size={32} color="var(--primary)" />
        </div>
        <div class="stat-value">{appState.indexStats.total_documents.toLocaleString()}</div>
        <div class="stat-label">Files Indexed</div>
      </div>
      <div class="stat-card">
        <div class="stat-icon">
          <Icon icon="hard-drive" size={32} color="var(--accent-purple)" />
        </div>
        <div class="stat-value">{formatBytes(appState.indexStats.total_size_bytes)}</div>
        <div class="stat-label">Total Size</div>
      </div>
      <div class="stat-card">
        <div class="stat-icon">
          <Icon icon="pin" size={32} color="var(--accent-orange)" />
        </div>
        <div class="stat-value">{appState.pinnedFiles.length}</div>
        <div class="stat-label">Pinned Files</div>
      </div>
    </div>
    
    <div class="stats-section">
      <h3>Quick Actions</h3>
      <div class="stats-actions">
        <button class="btn-primary" onclick={() => appState.startIndexing()}>
          <Icon icon="lightning" size={16} />
          <span>Rebuild Index</span>
        </button>
        <button class="btn-secondary" onclick={() => appState.activeTab = "search"}>
          <Icon icon="magnifying-glass" size={16} />
          <span>Start Searching</span>
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

<style>
  .stats-header h1 {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 20px;
    font-weight: 600;
  }

  .stat-icon {
    margin-bottom: 12px;
  }

  .spinner-large {
    width: 40px;
    height: 40px;
    border: 3px solid var(--border);
    border-top-color: var(--primary);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
    margin-bottom: 16px;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .stats-loading {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 80px 20px;
  }

  .stats-loading p {
    color: var(--text-secondary);
  }
</style>

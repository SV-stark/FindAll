<script lang="ts">
  import { onMount } from "svelte";
  import { appState } from '$lib/state.svelte';
  import SearchTab from '$lib/components/SearchTab.svelte';
  import SettingsTab from '$lib/components/SettingsTab.svelte';
  import StatsTab from '$lib/components/StatsTab.svelte';
  import QuickLookModal from '$lib/components/QuickLookModal.svelte';
  import ShortcutsModal from '$lib/components/ShortcutsModal.svelte';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import Icon from '$lib/components/Icon.svelte';
  import { formatEta } from '$lib/types';

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      if (appState.isQuickLookOpen) {
        appState.isQuickLookOpen = false;
        return;
      }
      if (appState.showShortcuts) {
        appState.showShortcuts = false;
        return;
      }
      appState.query = "";
      appState.results = [];
      appState.selectedPath = null;
      appState.previewContent = null;
      appState.selectedIndex = -1;
      return;
    }

    if (event.key === "?" || (event.shiftKey && event.key === "/")) {
      if (document.activeElement?.tagName !== "INPUT") {
        event.preventDefault();
        appState.showShortcuts = !appState.showShortcuts;
        return;
      }
    }

    if (event.code === "Space" && appState.selectedPath && !appState.isQuickLookOpen && document.activeElement?.tagName !== "INPUT") {
      event.preventDefault();
      appState.isQuickLookOpen = true;
      return;
    }

    if (appState.results.length > 0) {
      if (event.key === "ArrowDown") {
        event.preventDefault();
        appState.selectedIndex = Math.min(appState.selectedIndex + 1, appState.results.length - 1);
        const result = appState.results[appState.selectedIndex];
        if (result) {
          appState.showPreview(result.file_path, appState.selectedIndex);
        }
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        appState.selectedIndex = Math.max(appState.selectedIndex - 1, 0);
        const result = appState.results[appState.selectedIndex];
        if (result) {
          appState.showPreview(result.file_path, appState.selectedIndex);
        }
      } else if (event.key === "Enter" && appState.selectedIndex >= 0) {
        event.preventDefault();
        const result = appState.results[appState.selectedIndex];
        if (result) {
          appState.openFile(result.file_path);
        }
      }
    }

    if ((event.ctrlKey || event.metaKey) && event.key === "p" && appState.selectedPath) {
      event.preventDefault();
      if (appState.pinnedFiles.includes(appState.selectedPath)) {
        appState.unpinFile(appState.selectedPath);
      } else {
        appState.pinFile(appState.selectedPath);
      }
    }
  }

  function handleDragOver(event: DragEvent) {
    event.preventDefault();
    event.stopPropagation();
    appState.isDragging = true;
  }

  function handleDragLeave(event: DragEvent) {
    event.preventDefault();
    event.stopPropagation();
    appState.isDragging = false;
  }

  async function handleDrop(event: DragEvent) {
    event.preventDefault();
    event.stopPropagation();
    appState.isDragging = false;
    
    const files = event.dataTransfer?.files;
    if (files && files.length > 0) {
      const path = files[0].path || files[0].name;
      if (confirm(`Index folder: ${path}?`)) {
        await appState.startIndexing(path);
      }
    }
  }

  onMount(async () => {
    window.addEventListener("keydown", handleKeydown);
    await appState.init();
    
    return () => {
      window.removeEventListener("keydown", handleKeydown);
    };
  });
</script>

<main 
  class="app-layout"
  class:dragging={appState.isDragging}
  ondragover={handleDragOver}
  ondragleave={handleDragLeave}
  ondrop={handleDrop}
>
  {#if appState.isDragging}
    <div class="drop-overlay" transition:fade>
      <div class="drop-message">
        <span class="drop-icon">
          <Icon icon="folder-open" size={64} color="var(--primary)" />
        </span>
        <p>Drop folder here to index</p>
      </div>
    </div>
  {/if}

  <!-- Sidebar -->
  <Sidebar />

  <!-- Main Content Area -->
  <div class="main-content">
    <!-- Top Bar -->
    <header class="top-bar">
      <div class="tabs">
        <button class:active={appState.activeTab === "search"} onclick={() => appState.activeTab = "search"} aria-label="Search tab">
          <Icon icon="magnifying-glass" size={16} />
          <span>Search</span>
        </button>
        <button class:active={appState.activeTab === "settings"} onclick={() => appState.activeTab = "settings"} aria-label="Settings tab">
          <Icon icon="settings" size={16} />
          <span>Settings</span>
          {#if appState.hasChanges}
            <span class="unsaved-indicator">●</span>
          {/if}
        </button>
        <button class:active={appState.activeTab === "stats"} onclick={() => { appState.activeTab = "stats"; appState.loadStatistics(); }} aria-label="Stats tab">
          <Icon icon="chart" size={16} />
          <span>Stats</span>
        </button>
      </div>

      <div class="top-bar-actions">
        <button 
          class="icon-btn" 
          onclick={() => appState.showShortcuts = true}
          title="Keyboard shortcuts (?)]"
          aria-label="Keyboard shortcuts"
        >
          <Icon icon="keyboard" size={18} />
        </button>
      </div>
    </header>

    <!-- Tab Content -->
    <div class="tab-content">
      {#if appState.activeTab === "search"}
        <SearchTab />
      {:else if appState.activeTab === "stats"}
        <StatsTab />
      {:else}
        <SettingsTab />
      {/if}
    </div>
  </div>

  <QuickLookModal />
  <ShortcutsModal />

  {#if appState.showSaveSuccess}
    <div class="toast success" transition:fade>
      <Icon icon="check" size={16} color="var(--success)" />
      <span>Settings saved successfully</span>
    </div>
  {/if}

  <!-- Status Indicator (Bottom Right) -->
  <div class="status-indicator" onclick={() => appState.showProgressPopup = !appState.showProgressPopup}>
    <div class="status-dots">
      {#if appState.indexProgress.status === "idle" || appState.indexProgress.status === "done"}
        <span class="dot green" title="Ready"></span>
      {:else if appState.indexProgress.status === "scanning"}
        <span class="dot yellow" title="Scanning..."></span>
        <span class="dot-count">{appState.progressPercentage}%</span>
      {:else if appState.indexProgress.status === "indexing"}
        <span class="dot red" title="Indexing..."></span>
        <span class="dot-count">{appState.progressPercentage}%</span>
      {/if}
    </div>
  </div>

  <!-- Progress Popup -->
  {#if appState.showProgressPopup}
    <div class="progress-popup-backdrop" onclick={() => appState.showProgressPopup = false}></div>
    <div class="progress-popup">
      <div class="popup-header">
        <span class="popup-title">
          {#if appState.indexProgress.status === "idle"}
            <Icon icon="check-circle" size={16} color="var(--success)" />
            Index Ready
          {:else if appState.indexProgress.status === "scanning"}
            <Icon icon="folder-search" size={16} color="var(--warning)" />
            Scanning...
          {:else if appState.indexProgress.status === "indexing"}
            <Icon icon="database" size={16} color="var(--error)" />
            Indexing...
          {:else}
            <Icon icon="check-circle" size={16} color="var(--success)" />
            Index Complete
          {/if}
        </span>
        <button class="popup-close" onclick={() => appState.showProgressPopup = false}>
          <Icon icon="x" size={14} />
        </button>
      </div>
      
      {#if appState.indexProgress.status !== "idle" && appState.indexProgress.status !== "done"}
        <div class="popup-progress-bar">
          <div class="popup-progress-fill" style="width: {appState.progressPercentage}%"></div>
        </div>
        <div class="popup-stats">
          <div class="popup-stat">
            <span class="stat-label">Progress</span>
            <span class="stat-value">{appState.indexProgress.processed.toLocaleString()} / {appState.indexProgress.total.toLocaleString()}</span>
          </div>
          {#if appState.indexProgress.status === "indexing"}
            <div class="popup-stat">
              <span class="stat-label">Speed</span>
              <span class="stat-value">{appState.indexProgress.files_per_second?.toFixed(1) || "0"} files/s</span>
            </div>
            <div class="popup-stat">
              <span class="stat-label">ETA</span>
              <span class="stat-value">{formatEta(appState.indexProgress.eta_seconds)}</span>
            </div>
          {/if}
        </div>
      {/if}
      
      {#if appState.indexProgress.currentFile && appState.indexProgress.status !== "done"}
        <div class="popup-current">
          <Icon icon="file" size={12} />
          <span class="current-file-name" title={appState.indexProgress.currentFile}>
            {appState.indexProgress.currentFile.split(/[\\/]/).pop()}
          </span>
        </div>
      {/if}
      
      {#if appState.indexProgress.current_folder}
        <div class="popup-folder">
          <Icon icon="folder" size={12} />
          <span class="current-folder-name" title={appState.indexProgress.current_folder}>
            {appState.indexProgress.current_folder}
          </span>
        </div>
      {/if}
      
      <div class="popup-actions">
        {#if appState.indexProgress.status === "idle" || appState.indexProgress.status === "done"}
          <button class="popup-btn" onclick={() => appState.startIndexing()}>
            <Icon icon="refresh" size={12} />
            Rebuild Index
          </button>
        {:else}
          <button class="popup-btn secondary" onclick={() => {}}>
            <Icon icon="x" size={12} />
            Cancel
          </button>
        {/if}
      </div>
    </div>
  {/if}
</main>

<style>
  .app-layout {
    display: flex;
    height: 100vh;
    max-width: none;
    padding: 0;
    position: relative;
  }

  .app-layout.dragging {
    outline: 2px dashed var(--primary);
    outline-offset: -10px;
  }

  .main-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    padding: 16px;
    overflow: hidden;
  }

  .top-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 16px;
    gap: 16px;
  }

  .top-bar .tabs {
    margin-bottom: 0;
    background: var(--bg-card);
    padding: 4px;
    border-radius: var(--radius-lg);
  }

  .top-bar-actions {
    display: flex;
    gap: 8px;
  }

  .icon-btn {
    width: 36px;
    height: 36px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    color: var(--text-secondary);
    cursor: pointer;
    transition: all 0.2s;
  }

  .icon-btn:hover {
    background: var(--bg);
    color: var(--text);
    border-color: var(--text-secondary);
  }

  .tab-content {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .toast {
    position: fixed;
    bottom: 24px;
    right: 24px;
    padding: 12px 20px;
    border-radius: var(--radius-md);
    background: var(--bg-card);
    border: 1px solid var(--border);
    box-shadow: var(--shadow-lg);
    z-index: 1001;
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .toast.success {
    border-color: var(--success);
    background: linear-gradient(135deg, var(--bg-card), rgba(16, 185, 129, 0.1));
  }

  /* Status Indicator */
  .status-indicator {
    position: fixed;
    bottom: 16px;
    right: 16px;
    z-index: 100;
    cursor: pointer;
    padding: 8px 12px;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 20px;
    box-shadow: var(--shadow-md);
    transition: all 0.2s;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .status-indicator:hover {
    background: var(--bg-elevated);
    border-color: var(--text-secondary);
    transform: scale(1.02);
  }

  .status-dots {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    animation: pulse 2s infinite;
  }

  .dot.green {
    background: var(--success);
    animation: none;
  }

  .dot.yellow {
    background: var(--warning);
  }

  .dot.red {
    background: var(--error);
  }

  .dot-count {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
  }

  /* Progress Popup */
  .progress-popup-backdrop {
    position: fixed;
    inset: 0;
    z-index: 100;
  }

  .progress-popup {
    position: fixed;
    bottom: 60px;
    right: 16px;
    width: 280px;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: var(--shadow-lg);
    z-index: 101;
    overflow: hidden;
    animation: slideUp 0.2s ease;
  }

  @keyframes slideUp {
    from {
      opacity: 0;
      transform: translateY(10px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .popup-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 14px;
    border-bottom: 1px solid var(--border);
    background: var(--bg);
  }

  .popup-title {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text);
  }

  .popup-close {
    background: none;
    border: none;
    padding: 4px;
    cursor: pointer;
    color: var(--text-muted);
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s;
  }

  .popup-close:hover {
    background: var(--border);
    color: var(--text);
  }

  .popup-progress-bar {
    height: 4px;
    background: var(--border);
    border-radius: 0;
  }

  .popup-progress-fill {
    height: 100%;
    background: linear-gradient(90deg, var(--primary), var(--accent-purple));
    transition: width 0.3s ease;
  }

  .popup-stats {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
    padding: 12px 14px;
    border-bottom: 1px solid var(--border);
  }

  .popup-stat {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .popup-stat .stat-label {
    font-size: 10px;
    text-transform: uppercase;
    color: var(--text-muted);
    font-weight: 500;
    letter-spacing: 0.3px;
  }

  .popup-stat .stat-value {
    font-size: 12px;
    font-weight: 600;
    color: var(--text);
  }

  .popup-current, .popup-folder {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 14px;
    font-size: 12px;
    color: var(--text-secondary);
    border-bottom: 1px solid var(--border);
  }

  .popup-current {
    color: var(--primary);
  }

  .current-file-name, .current-folder-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
  }

  .popup-actions {
    padding: 10px 14px;
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .popup-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    background: var(--primary);
    color: white;
    border: none;
    border-radius: 6px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s;
  }

  .popup-btn:hover {
    background: var(--primary-hover);
  }

  .popup-btn.secondary {
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
  }

  .popup-btn.secondary:hover {
    background: var(--border);
  }
</style>

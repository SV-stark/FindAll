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

  {#if appState.indexProgress.status !== "idle"}
    <div class="progress-toast" class:done={appState.indexProgress.status === "done"}>
      <div class="progress-header">
        <span class="status-text">
          {#if appState.indexProgress.status === "scanning"}
            <Icon icon="folder" size={16} />
            <span>Scanning directories...</span>
          {:else if appState.indexProgress.status === "done"}
            <Icon icon="check" size={16} color="var(--success)" />
            <span>Indexing completed!</span>
          {:else}
            <Icon icon="lightning" size={16} />
            <span>Indexing files...</span>
          {/if}
        </span>
        <span class="percentage">{appState.progressPercentage}%</span>
      </div>
      <div class="progress-bar">
        <div class="progress-fill" style="width: {appState.progressPercentage}%"></div>
      </div>
      <div class="progress-details">
        <div class="progress-stat">
          <span class="stat-label">Files:</span>
          <span class="stat-value">{appState.indexProgress.processed.toLocaleString()} / {appState.indexProgress.total.toLocaleString()}</span>
        </div>
        {#if appState.indexProgress.status === "indexing" && appState.indexProgress.processed > 0}
          <div class="progress-stat">
            <span class="stat-label">Speed:</span>
            <span class="stat-value">{appState.indexProgress.files_per_second?.toFixed(1) || "0"} files/sec</span>
          </div>
          <div class="progress-stat">
            <span class="stat-label">ETA:</span>
            <span class="stat-value">{formatEta(appState.indexProgress.eta_seconds)}</span>
          </div>
        {/if}
      </div>
      {#if appState.indexProgress.currentFile && appState.indexProgress.status !== "done"}
        <div class="current-file" title={appState.indexProgress.currentFile}>
          <Icon icon="file" size={14} />
          <span>{appState.indexProgress.currentFile.split(/[\\/]/).pop()}</span>
        </div>
      {/if}
      {#if appState.indexProgress.current_folder}
        <div class="current-folder" title={appState.indexProgress.current_folder}>
          <Icon icon="folder" size={14} />
          <span>{appState.indexProgress.current_folder}</span>
        </div>
      {/if}
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
</style>

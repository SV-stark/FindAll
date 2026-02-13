<script lang="ts">
  import { onMount } from "svelte";
  import { appState } from '$lib/state.svelte';
  import SearchTab from '$lib/components/SearchTab.svelte';
  import SettingsTab from '$lib/components/SettingsTab.svelte';
  import StatsTab from '$lib/components/StatsTab.svelte';
  import QuickLookModal from '$lib/components/QuickLookModal.svelte';
  import { formatEta } from '$lib/types';

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      if (appState.isQuickLookOpen) {
        appState.isQuickLookOpen = false;
        return;
      }
      appState.query = "";
      appState.results = [];
      appState.selectedPath = null;
      appState.previewContent = null;
      appState.selectedIndex = -1;
      return;
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
  class="container"
  class:dragging={appState.isDragging}
  ondragover={handleDragOver}
  ondragleave={handleDragLeave}
  ondrop={handleDrop}
>
  {#if appState.isDragging}
    <div class="drop-overlay" transition:fade>
      <div class="drop-message">
        <span class="drop-icon">📁</span>
        <p>Drop folder here to index</p>
      </div>
    </div>
  {/if}

  <nav class="tabs">
    <button class:active={appState.activeTab === "search"} onclick={() => appState.activeTab = "search"}>
      <span class="tab-icon">🔍</span> Search
    </button>
    <button class:active={appState.activeTab === "settings"} onclick={() => appState.activeTab = "settings"}>
      <span class="tab-icon">⚙️</span> Settings
      {#if appState.hasChanges}
        <span class="unsaved-indicator">●</span>
      {/if}
    </button>
    <button class:active={appState.activeTab === "stats"} onclick={() => { appState.activeTab = "stats"; appState.loadStatistics(); }}>
      <span class="tab-icon">📊</span> Stats
    </button>
  </nav>

  {#if appState.activeTab === "search"}
    <SearchTab />
  {:else if appState.activeTab === "stats"}
    <StatsTab />
  {:else}
    <SettingsTab />
  {/if}

  {#if appState.showSaveSuccess}
    <div class="toast success" transition:fade>
      <span>✓ Settings saved successfully</span>
    </div>
  {/if}

  <QuickLookModal />

  {#if appState.indexProgress.status !== "idle"}
    <div class="progress-toast" class:done={appState.indexProgress.status === "done"}>
      <div class="progress-header">
        <span class="status-text">
          {appState.indexProgress.status === "scanning" ? "🔍 Scanning directories..." : 
           appState.indexProgress.status === "done" ? "✅ Indexing completed!" : 
           `⚡ Indexing files...`}
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
          📄 {appState.indexProgress.currentFile.split(/[\\/]/).pop()}
        </div>
      {/if}
      {#if appState.indexProgress.current_folder}
        <div class="current-folder" title={appState.indexProgress.current_folder}>
          📁 {appState.indexProgress.current_folder}
        </div>
      {/if}
    </div>
  {/if}
</main>

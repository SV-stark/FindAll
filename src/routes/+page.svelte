<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { openPath } from "@tauri-apps/plugin-opener";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";

  // Types
  interface SearchResult {
    file_path: string;
    title: string | null;
    score: number;
  }

  // State
  let activeTab = $state<"search" | "settings">("search");
  let query = $state("");
  let results = $state<SearchResult[]>([]);
  let isSearching = $state(false);
  let isIndexing = $state(false);
  
  // Search filters
  let minSize = $state<number | null>(null);
  let maxSize = $state<number | null>(null);
  let showFilters = $state(false);
  
  // Settings state
  let settings = $state({
    index_dirs: [] as string[],
    exclude_patterns: [] as string[],
    theme: "auto",
    max_results: 50
  });

  async function loadSettings() {
    try {
      settings = await invoke("get_settings");
    } catch (e) {
      console.error("Failed to load settings:", e);
    }
  }

  async function saveSettings() {
    try {
      await invoke("save_settings", { settings });
    } catch (e) {
      console.error("Failed to save settings:", e);
    }
  }

  function addIndexDir(path: string) {
    if (path && !settings.index_dirs.includes(path)) {
      settings.index_dirs = [...settings.index_dirs, path];
      saveSettings();
    }
  }

  function removeIndexDir(path: string) {
    settings.index_dirs = settings.index_dirs.filter(p => p !== path);
    saveSettings();
  }

  function addExcludePattern(pattern: string) {
    if (pattern && !settings.exclude_patterns.includes(pattern)) {
      settings.exclude_patterns = [...settings.exclude_patterns, pattern];
      saveSettings();
    }
  }

  function removeExcludePattern(pattern: string) {
    settings.exclude_patterns = settings.exclude_patterns.filter(p => p !== pattern);
    saveSettings();
  }

  let selectedPath = $state<string | null>(null);
  let previewContent = $state<string | null>(null);
  
  // Indexing progress state
  let indexProgress = $state({
    total: 0,
    processed: 0,
    currentFile: "",
    status: "idle" as "idle" | "scanning" | "indexing" | "done"
  });

  let progressPercentage = $derived(
    indexProgress.total > 0 
      ? Math.round((indexProgress.processed / indexProgress.total) * 100) 
      : 0
  );

  // Debounce search
  let debounceTimer: ReturnType<typeof setTimeout>;

  async function performSearch() {
    if (!query.trim()) {
      results = [];
      return;
    }

    isSearching = true;
    try {
      results = await invoke<SearchResult[]>("search_query", { 
        query, 
        limit: settings.max_results,
        min_size: minSize ? minSize * 1024 * 1024 : null, // Convert MB to Bytes
        max_size: maxSize ? maxSize * 1024 * 1024 : null
      });
    } catch (e) {
      console.error("Search failed:", e);
      results = [];
    } finally {
      isSearching = false;
    }
  }

  function debouncedSearch() {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(performSearch, 300);
  }

  async function startIndexing() {
    isIndexing = true;
    try {
      // For demo, index the user's documents folder
      // In production, this would be configurable
      const homeDir = await invoke<string>("get_home_dir").catch(() => "./");
      await invoke("start_indexing", { path: homeDir });
    } catch (e) {
      console.error("Indexing failed:", e);
    } finally {
      // Note: indexing runs in background, so we set this to false immediately
      // In production, you'd track actual indexing status
      isIndexing = false;
    }
  }

  async function openFile(path: string) {
    try {
      await openPath(path);
    } catch (e) {
      console.error("Failed to open file:", e);
    }
  }

  async function showPreview(path: string) {
    selectedPath = path;
    try {
      previewContent = await invoke<string>("get_file_preview", { path });
    } catch (e) {
      previewContent = "Failed to load preview";
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      query = "";
      results = [];
      selectedPath = null;
      previewContent = null;
    }
  }

  const effectiveTheme = $derived(
    settings.theme === "auto" 
      ? (window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light")
      : settings.theme
  );

  $effect(() => {
    document.body.setAttribute("data-theme", effectiveTheme);
  });

  onMount(async () => {
    window.addEventListener("keydown", handleKeydown);
    await loadSettings();

    const unlisten = await listen<{
      total: number,
      processed: number,
      current_file: string,
      status: string
    }>("indexing-progress", (event) => {
      indexProgress.total = event.payload.total;
      indexProgress.processed = event.payload.processed;
      indexProgress.currentFile = event.payload.current_file;
      indexProgress.status = event.payload.status as any;

      if (indexProgress.status === "done") {
        setTimeout(() => {
          indexProgress.status = "idle";
        }, 5000);
      }
    });

    return () => {
      window.removeEventListener("keydown", handleKeydown);
      unlisten();
    };
  });
</script>

<main class="container">
  <nav class="tabs">
    <button class:active={activeTab === "search"} onclick={() => activeTab = "search"}>
      <span class="tab-icon">🔍</span> Search
    </button>
    <button class:active={activeTab === "settings"} onclick={() => activeTab = "settings"}>
      <span class="tab-icon">⚙️</span> Settings
    </button>
  </nav>

  {#if activeTab === "search"}
    <div class="anytxt-layout" in:fade={{ duration: 200 }}>
      <!-- AnyTXT Header Style -->
      <header class="app-header">
        <div class="search-section">
          <div class="search-input-wrapper">
            <span class="search-icon">🔍</span>
            <input
              type="text"
              class="anytxt-search-input"
              placeholder="Search everything or specific words..."
              bind:value={query}
              oninput={debouncedSearch}
            />
            {#if isSearching}
              <div class="mini-spinner"></div>
            {/if}
          </div>
          <button class="anytxt-btn primary" onclick={performSearch}>Search</button>
        </div>
      </header>

      <!-- Toolbar / Filters -->
      <div class="app-toolbar">
        <div class="filter-group">
          <label>Filter:</label>
          <select class="toolbar-select">
            <option>All File Types</option>
            <option>Documents (*.docx, *.pdf...)</option>
            <option>Code Files (*.rs, *.js...)</option>
            <option>Text Files (*.txt, *.md)</option>
          </select>
        </div>
        <div class="spacer"></div>
        <div class="toolbar-actions">
          <button class="toolbar-btn" class:active={showFilters} onclick={() => showFilters = !showFilters}>
            ⚙️ Filters
          </button>
          <button class="toolbar-btn" onclick={startIndexing} disabled={isIndexing}>
            {isIndexing ? "Indexing..." : "⚡ Rebuild Index"}
          </button>
        </div>
      </div>

      {#if showFilters}
        <div class="filter-panel" transition:fade>
          <div class="filter-item">
            <label>Min Size (MB):</label>
            <input type="number" bind:value={minSize} oninput={debouncedSearch} placeholder="Any" />
          </div>
          <div class="filter-item">
            <label>Max Size (MB):</label>
            <input type="number" bind:value={maxSize} oninput={debouncedSearch} placeholder="Any" />
          </div>
          <button class="clear-filters" onclick={() => { minSize = null; maxSize = null; debouncedSearch(); }}>Clear</button>
        </div>
      {/if}

      <!-- Main Content Area -->
      <div class="workspace">
        <div class="results-pane">
          <div class="results-table-header">
            <div class="col-name">Name</div>
            <div class="col-path">Path</div>
            <div class="col-score">Score</div>
          </div>
          <div class="results-scroller">
            {#if results.length === 0 && query.trim()}
              <div class="empty-state">No matching documents found.</div>
            {:else if results.length > 0}
              {#each results as result}
                <div
                  class="table-row"
                  class:active={selectedPath === result.file_path}
                  onclick={() => showPreview(result.file_path)}
                  ondblclick={() => openFile(result.file_path)}
                  role="button"
                  tabindex="0"
                >
                  <div class="col-name">
                    <span class="file-icon">{result.file_path.split('.').pop()?.toUpperCase() || '📄'}</span>
                    <span class="file-title">{result.title || result.file_path.split(/[\\/]/).pop()}</span>
                  </div>
                  <div class="col-path">{result.file_path}</div>
                  <div class="col-score">
                    <button class="icon-btn" onclick={(e) => { e.stopPropagation(); invoke('open_folder', { path: result.file_path }); }} title="Open Location">📂</button>
                    {result.score.toFixed(1)}
                  </div>
                </div>
              {/each}
            {/if}
          </div>
          <footer class="results-footer">
            {#if results.length > 0}
              {results.length} results found in the index
            {:else}
              Ready to search
            {/if}
          </footer>
        </div>

        <!-- Preview Pane -->
        {#if selectedPath}
          <div class="preview-pane" transition:fade>
            <div class="preview-toolbar">
              <span class="preview-title">Quick Preview: {selectedPath.split(/[\\/]/).pop()}</span>
              <button class="close-preview" onclick={() => { selectedPath = null; previewContent = null; }}>✕</button>
            </div>
            <div class="preview-body">
              {#if previewContent}
                <pre>{previewContent}</pre>
              {:else}
                <div class="preview-loading">Loading content...</div>
              {/if}
            </div>
          </div>
        {/if}
      </div>
    </div>
  {:else}
    <div class="settings-tab" in:fade={{ duration: 200 }}>
      <h1>Settings</h1>
      
      <section class="settings-section">
        <h2>Appearance</h2>
        <div class="setting-item">
          <label>Theme</label>
          <select bind:value={settings.theme} onchange={saveSettings}>
            <option value="auto">System Default</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
          </select>
        </div>
      </section>

      <section class="settings-section">
        <h2>Index Locations</h2>
        <div class="dir-list">
          {#each settings.index_dirs as dir}
            <div class="dir-item">
              <span>{dir}</span>
              <button class="remove-btn" onclick={() => removeIndexDir(dir)}>✕</button>
            </div>
          {/each}
          {#if settings.index_dirs.length === 0}
            <p class="empty-hint">No specific directories added (Home is indexed by default)</p>
          {/if}
        </div>
        <button class="add-btn" onclick={async () => {
          try {
            const selected = await invoke<string | null>("select_folder");
            if (selected) {
              addIndexDir(selected);
            }
          } catch (e) {
            console.error("Failed to pick folder:", e);
          }
        }}>Browse Folder...</button>
      </section>

      <section class="settings-section">
        <h2>Exclusions</h2>
        <div class="pattern-list">
          {#each settings.exclude_patterns as pattern}
            <div class="pattern-pill">
              {pattern}
              <button onclick={() => removeExcludePattern(pattern)}>✕</button>
            </div>
          {/each}
        </div>
        <div class="add-pattern">
          <input type="text" placeholder="Add pattern (e.g. *.tmp or backup/)" 
            onkeydown={(e) => {
              if (e.key === 'Enter') {
                addExcludePattern(e.currentTarget.value);
                e.currentTarget.value = '';
              }
            }} />
        </div>
      </section>

      <section class="settings-section">
        <h2>Search Performance</h2>
        <div class="setting-item">
          <label>Max Results</label>
          <input type="number" bind:value={settings.max_results} onchange={saveSettings} min="10" max="1000" />
        </div>
      </section>
    </div>
  {/if}

  {#if indexProgress.status !== "idle"}
    <div class="progress-bar-container" class:done={indexProgress.status === "done"}>
      <div class="progress-info">
        <span class="status-text">
          {indexProgress.status === "scanning" ? "Scanning files..." : 
           indexProgress.status === "done" ? "Indexing completed" : 
           `Indexing: ${indexProgress.processed} / ${indexProgress.total}`}
        </span>
        <span class="percentage">{progressPercentage}%</span>
      </div>
      <div class="progress-track">
        <div class="progress-fill" style="width: {progressPercentage}%"></div>
      </div>
      {#if indexProgress.currentFile && indexProgress.status !== "done"}
        <div class="current-file">{indexProgress.currentFile.split(/[\\/]/).pop()}</div>
      {/if}
    </div>
  {/if}
</main>

<style>
  :global(body) {
    font-family: "Segoe UI", Tahoma, sans-serif;
    background: #fdfdfd;
    color: #333;
    overflow: hidden; /* App-like feel */
    height: 100vh;
    margin: 0;
  }

  :global(body[data-theme="dark"]) {
    background: #1e1e1e;
    color: #d4d4d4;
  }

  .container {
    display: flex;
    flex-direction: column;
    height: 100vh;
    max-width: 100vw;
    padding: 0;
  }

  /* Navigation Tabs */
  .tabs {
    display: flex;
    background: #f3f3f3;
    border-bottom: 1px solid #d1d1d1;
    padding: 0 10px;
    gap: 2px;
  }

  :global(body[data-theme="dark"]) .tabs {
    background: #252526;
    border-color: #3e3e42;
  }

  .tabs button {
    background: transparent;
    border: none;
    padding: 8px 16px;
    font-size: 13px;
    color: #666;
    cursor: pointer;
    border-bottom: 2px solid transparent;
    transition: all 0.2s;
  }

  .tabs button:hover {
    color: #333;
    background: #e9e9e9;
  }

  :global(body[data-theme="dark"]) .tabs button:hover {
    color: #fff;
    background: #2d2d2d;
  }

  .tabs button.active {
    color: #0078d4;
    border-bottom-color: #0078d4;
    font-weight: 600;
    background: #fff;
  }

  :global(body[data-theme="dark"]) .tabs button.active {
    color: #569cd6;
    border-bottom-color: #569cd6;
    background: #1e1e1e;
  }

  .tab-icon {
    margin-right: 6px;
  }

  /* Header / Search Area */
  .app-header {
    background: #fff;
    padding: 20px 20px;
    border-bottom: 1px solid #e1e1e1;
  }

  :global(body[data-theme="dark"]) .app-header {
    background: #2d2d2d;
    border-color: #3e3e42;
  }

  .search-section {
    display: flex;
    gap: 12px;
    max-width: 900px;
    margin: 0 auto;
  }

  .search-input-wrapper {
    position: relative;
    flex: 1;
    display: flex;
    align-items: center;
  }

  .search-icon {
    position: absolute;
    left: 12px;
    color: #888;
  }

  .anytxt-search-input {
    width: 100%;
    padding: 10px 15px 10px 38px;
    font-size: 14px;
    border: 1px solid #ccc;
    border-radius: 4px;
    outline: none;
    transition: border-color 0.2s;
  }

  .anytxt-search-input:focus {
    border-color: #0078d4;
    box-shadow: 0 0 0 2px rgba(0, 120, 212, 0.1);
  }

  :global(body[data-theme="dark"]) .anytxt-search-input {
    background: #3c3c3c;
    border-color: #555;
    color: #fff;
  }

  .anytxt-btn {
    padding: 8px 24px;
    font-size: 14px;
    border-radius: 4px;
    cursor: pointer;
    border: 1px solid transparent;
    transition: background 0.2s;
  }

  .anytxt-btn.primary {
    background: #0078d4;
    color: #fff;
  }

  .anytxt-btn.primary:hover {
    background: #005a9e;
  }

  /* Toolbar */
  .app-toolbar {
    background: #f9f9f9;
    padding: 8px 20px;
    display: flex;
    align-items: center;
    border-bottom: 1px solid #e1e1e1;
    font-size: 13px;
  }

  :global(body[data-theme="dark"]) .app-toolbar {
    background: #252526;
    border-color: #3e3e42;
    color: #ccc;
  }

  .filter-group {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .toolbar-select {
    padding: 4px 8px;
    border: 1px solid #ccc;
    background: #fff;
    border-radius: 3px;
    font-size: 12px;
  }

  :global(body[data-theme="dark"]) .toolbar-select {
    background: #3c3c3c;
    border-color: #555;
    color: #fff;
  }

  .toolbar-btn {
    background: transparent;
    border: 1px solid #ccc;
    padding: 4px 12px;
    border-radius: 3px;
    cursor: pointer;
  }

  .toolbar-btn.active {
    background: #e1e1e1;
    border-color: #0078d4;
    color: #0078d4;
  }

  :global(body[data-theme="dark"]) .toolbar-btn.active {
    background: #3c3c3c;
    border-color: #569cd6;
    color: #569cd6;
  }

  .filter-panel {
    background: #fdfdfd;
    border-bottom: 1px solid #d1d1d1;
    padding: 10px 20px;
    display: flex;
    gap: 20px;
    align-items: center;
    font-size: 12px;
  }

  :global(body[data-theme="dark"]) .filter-panel {
    background: #252526;
    border-color: #3e3e42;
    color: #ccc;
  }

  .filter-item {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .filter-item input {
    width: 80px;
    padding: 4px 8px;
    border: 1px solid #ccc;
    border-radius: 3px;
    font-size: 12px;
  }

  :global(body[data-theme="dark"]) .filter-item input {
    background: #3c3c3c;
    border-color: #555;
    color: #fff;
  }

  .clear-filters {
    background: #eee;
    border: none;
    padding: 4px 10px;
    border-radius: 3px;
    cursor: pointer;
    font-size: 11px;
  }

  .clear-filters:hover {
    background: #ddd;
  }

  :global(body[data-theme="dark"]) .toolbar-btn {
    border-color: #555;
    color: #ccc;
  }

  .spacer { flex: 1; }

  /* Workspace / Results */
  .workspace {
    flex: 1;
    display: flex;
    overflow: hidden; /* Header and Toolbar stay fixed */
  }

  .results-pane {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .results-table-header {
    display: flex;
    background: #f3f3f3;
    border-bottom: 1px solid #d1d1d1;
    font-weight: 600;
    font-size: 12px;
    padding: 5px 0;
    color: #555;
  }

  :global(body[data-theme="dark"]) .results-table-header {
    background: #2d2d2d;
    border-color: #3e3e42;
    color: #aaa;
  }

  .results-scroller {
    flex: 1;
    overflow-y: auto;
  }

  .table-row {
    display: flex;
    border-bottom: 1px solid #f0f0f0;
    padding: 8px 0;
    font-size: 13px;
    cursor: pointer;
    transition: background 0.1s;
  }

  :global(body[data-theme="dark"]) .table-row {
    border-color: #2d2d2d;
  }

  .table-row:hover {
    background: #f0f7ff;
  }

  .table-row.active {
    background: #e5f1ff;
    border-left: 3px solid #0078d4;
  }

  :global(body[data-theme="dark"]) .table-row:hover {
    background: #2a2d2e;
  }

  :global(body[data-theme="dark"]) .table-row.active {
    background: #37373d;
    border-left-color: #0078d4;
  }

  .col-name { flex: 2; padding-left: 15px; display: flex; align-items: center; gap: 8px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .col-path { flex: 3; color: #888; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .col-score { width: 100px; text-align: center; color: #aaa; display: flex; align-items: center; justify-content: center; gap: 5px; }

  .icon-btn {
    background: transparent;
    border: none;
    padding: 4px;
    cursor: pointer;
    font-size: 14px;
    border-radius: 4px;
    opacity: 0.6;
    transition: all 0.2s;
  }

  .icon-btn:hover {
    opacity: 1;
    background: rgba(0,0,0,0.05);
  }

  :global(body[data-theme="dark"]) .icon-btn:hover {
    background: rgba(255,255,255,0.1);
  }

  .file-icon {
    font-size: 10px;
    background: #e1e1e1;
    padding: 2px 4px;
    border-radius: 2px;
    font-weight: bold;
    color: #555;
    min-width: 35px;
    text-align: center;
  }

  :global(body[data-theme="dark"]) .file-icon {
    background: #3c3c3c;
    color: #aaa;
  }

  .results-footer {
    padding: 5px 20px;
    font-size: 11px;
    background: #f3f3f3;
    border-top: 1px solid #d1d1d1;
    color: #888;
  }

  :global(body[data-theme="dark"]) .results-footer {
    background: #252526;
    border-color: #3e3e42;
  }

  /* Preview Pane */
  .preview-pane {
    width: 450px;
    border-left: 1px solid #d1d1d1;
    display: flex;
    flex-direction: column;
    background: #fff;
    z-index: 10;
  }

  :global(body[data-theme="dark"]) .preview-pane {
    background: #1e1e1e;
    border-color: #3e3e42;
  }

  .preview-toolbar {
    padding: 10px 15px;
    background: #f9f9f9;
    border-bottom: 1px solid #e1e1e1;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  :global(body[data-theme="dark"]) .preview-toolbar {
    background: #2d2d2d;
    border-color: #3e3e42;
  }

  .preview-title { font-size: 12px; font-weight: 600; color: #555; }
  .close-preview { background: transparent; border: none; font-size: 16px; cursor: pointer; color: #888; }

  .preview-body {
    flex: 1;
    padding: 20px;
    overflow-y: auto;
    font-family: Consolas, monospace;
    font-size: 12px;
    line-height: 1.5;
    white-space: pre-wrap;
  }

  /* Settings Page Polish */
  .settings-tab {
    max-width: 800px;
    margin: 40px auto;
    padding: 0 20px;
    overflow-y: auto;
  }

  .settings-section {
    background: #fff;
    border: 1px solid #e1e1e1;
    border-radius: 4px;
    padding: 20px;
    margin-bottom: 20px;
  }

  :global(body[data-theme="dark"]) .settings-section {
    background: #2d2d2d;
    border-color: #3e3e42;
  }

  .settings-section h2 { font-size: 16px; margin-bottom: 15px; border-bottom: 1px solid #eee; padding-bottom: 10px; }
  
  /* Progress Bar at bottom */
  .progress-bar-container {
    position: fixed;
    bottom: 25px;
    right: 25px;
    width: 320px;
    background: #fff;
    border: 1px solid #d1d1d1;
    box-shadow: 0 4px 12px rgba(0,0,0,0.15);
    border-radius: 8px;
    padding: 15px;
    z-index: 100;
  }

  :global(body[data-theme="dark"]) .progress-bar-container {
    background: #2d2d2d;
    border-color: #3e3e42;
  }

  .progress-info { display: flex; justify-content: space-between; font-size: 12px; margin-bottom: 8px; }
  .progress-track { height: 6px; background: #eee; border-radius: 3px; overflow: hidden; }
  .progress-fill { height: 100%; background: #0078d4; transition: width 0.3s; }
  .current-file { font-size: 10px; color: #888; margin-top: 5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

  .empty-state {
    text-align: center;
    padding: 100px 20px;
    color: #aaa;
    font-style: italic;
  }

  .mini-spinner {
    width: 14px;
    height: 14px;
    border: 2px solid #0078d4;
    border-top-color: transparent;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
    position: absolute;
    right: 12px;
  }

  @keyframes spin { to { transform: rotate(360deg); } }

  /* Misc */
  .spacer { flex: 1; }
  
  .anytxt-btn:disabled, .toolbar-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .pattern-list { display: flex; flex-wrap: wrap; gap: 5px; margin-bottom: 10px; }
  .pattern-pill { background: #eee; padding: 2px 8px; border-radius: 12px; font-size: 11px; display: flex; align-items: center; gap: 5px; border: 1px solid #ddd; }
  :global(body[data-theme="dark"]) .pattern-pill { background: #333; border-color: #444; color: #aaa; }
  .pattern-pill button { background: transparent; border: none; cursor: pointer; color: #888; }

  .dir-list { display: flex; flex-direction: column; gap: 5px; margin-bottom: 10px; }
  .dir-item { display: flex; justify-content: space-between; background: #f9f9f9; padding: 5px 10px; border-radius: 4px; border: 1px solid #eee; font-size: 12px; }
  :global(body[data-theme="dark"]) .dir-item { background: #3c3c3c; border-color: #444; color: #aaa; }
  .remove-btn { color: #f44336; background: transparent; border: none; cursor: pointer; }

  .add-pattern input { width: 100%; padding: 6px 10px; border: 1px solid #ccc; border-radius: 4px; font-size: 12px; outline: none; }
  :global(body[data-theme="dark"]) .add-pattern input { background: #3c3c3c; border-color: #555; color: #fff; }

  .setting-item { display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px; font-size: 13px; }
  .setting-item label { color: #666; }
  :global(body[data-theme="dark"]) .setting-item label { color: #aaa; }
</style>


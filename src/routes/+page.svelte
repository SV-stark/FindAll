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
  
  // Settings state
  let settings = $state({
    index_dirs: [] as string[],
    exclude_patterns: [] as string[],
    theme: "auto",
    max_results: 50
  });

  async fn loadSettings() {
    try {
      settings = await invoke("get_settings");
    } catch (e) {
      console.error("Failed to load settings:", e);
    }
  }

  async fn saveSettings() {
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
        limit: settings.max_results 
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
    <div class="search-tab" in:fade={{ duration: 200 }}>
      <div class="search-header">
        <h1>Flash Search</h1>
        <p class="subtitle">Ultrafast local full-text search</p>
      </div>

    <div class="search-box">
      <input
        type="text"
        class="search-input"
        placeholder="Search files..."
        bind:value={query}
        oninput={debouncedSearch}
      />
      {#if isSearching}
        <span class="loading">Searching...</span>
      {/if}
    </div>

    <div class="actions">
      <button onclick={startIndexing} disabled={isIndexing}>
        {isIndexing ? "Indexing..." : "Start Indexing"}
      </button>
      <span class="hint">Press ESC to clear</span>
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
          const home = await invoke<string>("get_home_dir");
          // Here we could use a directory picker plugin if available
          addIndexDir(home);
        }}>Add Folder</button>
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
  </div>

  <div class="results-container">
    <div class="results-list">
      {#if results.length === 0 && query.trim()}
        <div class="no-results">No results found for "{query}"</div>
      {:else if results.length > 0}
        <div class="results-count">{results.length} results</div>
        {#each results as result}
          <div
            class="result-item"
            class:selected={selectedPath === result.file_path}
            onclick={() => showPreview(result.file_path)}
            ondblclick={() => openFile(result.file_path)}
            role="button"
            tabindex="0"
          >
            <div class="result-title">{result.title || result.file_path.split("/").pop()}</div>
            <div class="result-path">{result.file_path}</div>
            <div class="result-score">Score: {result.score.toFixed(2)}</div>
          </div>
        {/each}
      {/if}
    </div>

    {#if previewContent}
      <div class="preview-panel">
        <div class="preview-header">
          <span>Preview</span>
          <button onclick={() => { selectedPath = null; previewContent = null; }}>✕</button>
        </div>
        <pre class="preview-content">{previewContent}</pre>
      </div>
    {/if}
  </div>

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
  :global(*) {
    box-sizing: border-box;
    margin: 0;
    padding: 0;
  }

  :global(body) {
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
    min-height: 100vh;
    margin: 0;
  }

  :global(body[data-theme="dark"]) {
    background: #1a1a2e;
    color: #eaeaea;
  }

  :global(body[data-theme="light"]) {
    background: #f5f5f7;
    color: #1a1a2e;
  }


  .container {
    max-width: 1200px;
    margin: 0 auto;
    padding: 1rem 2rem 2rem 2rem;
  }

  .tabs {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 2rem;
    background: #252542;
    padding: 0.4rem;
    border-radius: 12px;
    width: fit-content;
    margin-left: auto;
    margin-right: auto;
    border: 1px solid #3a3a5c;
  }

  .tabs button {
    background: transparent;
    padding: 0.5rem 1.2rem;
    font-size: 0.9rem;
    font-weight: 500;
    color: #6b6b8c;
    border-radius: 8px;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    transition: all 0.2s;
  }

  .tabs button.active {
    background: #30305a;
    color: #fff;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
  }

  .tab-icon {
    font-size: 1rem;
  }

  .search-header {
    text-align: center;
    margin-bottom: 2rem;
  }

  h1 {
    font-size: 2.5rem;
    font-weight: 700;
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    margin-bottom: 0.5rem;
  }

  .subtitle {
    color: #a0a0a0;
    margin-bottom: 2rem;
  }

  .search-box {
    position: relative;
    max-width: 600px;
    margin: 0 auto;
  }

  .search-input {
    width: 100%;
    padding: 1rem 1.5rem;
    font-size: 1.1rem;
    border: 2px solid #3a3a5c;
    border-radius: 12px;
    background: #252542;
    color: #fff;
    outline: none;
    transition: all 0.3s ease;
  }

  .search-input:focus {
    border-color: #667eea;
    box-shadow: 0 0 0 3px rgba(102, 126, 234, 0.1);
  }

  .search-input::placeholder {
    color: #6b6b8c;
  }

  .loading {
    position: absolute;
    right: 1rem;
    top: 50%;
    transform: translateY(-50%);
    color: #667eea;
    font-size: 0.875rem;
  }

  .actions {
    margin-top: 1rem;
    display: flex;
    gap: 1rem;
    justify-content: center;
    align-items: center;
  }

  button {
    padding: 0.75rem 1.5rem;
    font-size: 0.9rem;
    border: none;
    border-radius: 8px;
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    color: white;
    cursor: pointer;
    transition: transform 0.2s, box-shadow 0.2s;
  }

  button:hover:not(:disabled) {
    transform: translateY(-2px);
    box-shadow: 0 4px 12px rgba(102, 126, 234, 0.4);
  }

  button:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .hint {
    color: #6b6b8c;
    font-size: 0.8rem;
  }

  .results-container {
    display: grid;
    grid-template-columns: 1fr;
    gap: 1rem;
  }

  .results-container:has(.preview-panel) {
    grid-template-columns: 1fr 1fr;
  }

  .results-list {
    max-height: 600px;
    overflow-y: auto;
  }

  .results-count {
    color: #6b6b8c;
    font-size: 0.875rem;
    margin-bottom: 1rem;
    padding-bottom: 0.5rem;
    border-bottom: 1px solid #3a3a5c;
  }

  .no-results {
    text-align: center;
    color: #6b6b8c;
    padding: 2rem;
  }

  .result-item {
    padding: 1rem;
    margin-bottom: 0.5rem;
    background: #252542;
    border-radius: 8px;
    cursor: pointer;
    transition: all 0.2s;
    border: 1px solid transparent;
  }

  .result-item:hover {
    background: #2f2f52;
    border-color: #667eea;
  }

  .result-item.selected {
    border-color: #667eea;
    background: #30305a;
  }

  .result-title {
    font-weight: 600;
    color: #eaeaea;
    margin-bottom: 0.25rem;
    font-size: 0.95rem;
  }

  .result-path {
    font-size: 0.8rem;
    color: #6b6b8c;
    margin-bottom: 0.25rem;
    word-break: break-all;
  }

  .result-score {
    font-size: 0.75rem;
    color: #667eea;
  }

  .preview-panel {
    background: #252542;
    border-radius: 12px;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    max-height: 600px;
  }

  .preview-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem;
    background: #30305a;
    border-bottom: 1px solid #3a3a5c;
  }

  .preview-header button {
    padding: 0.25rem 0.5rem;
    background: transparent;
    color: #6b6b8c;
    font-size: 1.2rem;
  }

  .preview-content {
    padding: 1rem;
    overflow-y: auto;
    white-space: pre-wrap;
    font-family: "Consolas", "Monaco", monospace;
    font-size: 0.85rem;
    line-height: 1.5;
    color: #d0d0e0;
    max-height: 550px;
  }

  @media (max-width: 900px) {
    .results-container:has(.preview-panel) {
      grid-template-columns: 1fr;
    }
  }

  :global(body[data-theme="light"]) .search-input {
    background: #ffffff;
    color: #1a1a2e;
    border-color: #e0e0e5;
  }

  :global(body[data-theme="light"]) .search-input::placeholder {
    color: #999;
  }

  :global(body[data-theme="light"]) .result-item {
    background: #ffffff;
  }

  :global(body[data-theme="light"]) .result-item:hover {
    background: #f0f0f5;
  }

  :global(body[data-theme="light"]) .result-path {
    color: #666;
  }

  :global(body[data-theme="light"]) .preview-panel {
    background: #ffffff;
  }

  :global(body[data-theme="light"]) .preview-content {
    color: #333;
  }

  :global(body[data-theme="light"]) .settings-section {
    background: #ffffff;
    border-color: #e0e0e5;
  }

  :global(body[data-theme="light"]) .dir-item,
  :global(body[data-theme="light"]) .pattern-pill,
  :global(body[data-theme="light"]) .tabs,
  :global(body[data-theme="light"]) .progress-bar-container {
    background: #fcfcfd;
    border-color: #e0e0e5;
  }

  :global(body[data-theme="light"]) select,
  :global(body[data-theme="light"]) input[type="number"],
  :global(body[data-theme="light"]) .add-pattern input {
    background: #f5f5f7;
    color: #1a1a2e;
    border-color: #e0e0e5;
  }

  :global(body[data-theme="light"]) .tabs button.active {
    background: #667eea;
    color: #fff;
  }


  /* Progress Bar Styles */
  .progress-bar-container {
    position: fixed;
    bottom: 24px;
    right: 24px;
    width: 320px;
    background: #252542;
    border: 1px solid #3a3a5c;
    border-radius: 12px;
    padding: 16px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
    z-index: 1000;
    transition: all 0.3s ease;
    animation: slideUp 0.3s ease-out;
    backdrop-filter: blur(10px);
  }

  .progress-bar-container.done {
    border-color: #48bb78;
  }

  .progress-info {
    display: flex;
    justify-content: space-between;
    margin-bottom: 8px;
    font-size: 0.85rem;
  }

  .status-text {
    font-weight: 600;
    color: #eaeaea;
  }

  .percentage {
    color: #667eea;
    font-weight: 700;
  }

  .progress-track {
    width: 100%;
    height: 6px;
    background: #1a1a2e;
    border-radius: 3px;
    overflow: hidden;
    margin-bottom: 8px;
  }

  .progress-fill {
    height: 100%;
    background: linear-gradient(90deg, #667eea, #764ba2);
    border-radius: 3px;
    transition: width 0.3s ease;
  }

  .current-file {
    font-size: 0.75rem;
    color: #6b6b8c;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  @keyframes slideUp {
    from { transform: translateY(20px); opacity: 0; }
    to { transform: translateY(0); opacity: 1; }
  }

  /* Settings Tab Styles */
  .settings-tab {
    max-width: 700px;
    margin: 0 auto;
    animation: fadeIn 0.3s ease;
  }

  .settings-tab h1 {
    margin-bottom: 1.5rem;
    font-size: 1.8rem;
  }

  .settings-section {
    background: #252542;
    border-radius: 12px;
    padding: 1.5rem;
    margin-bottom: 1.5rem;
    border: 1px solid #3a3a5c;
  }

  .settings-section h2 {
    font-size: 1.1rem;
    margin-bottom: 1.2rem;
    color: #667eea;
  }

  .setting-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
  }

  .setting-item label {
    font-weight: 500;
  }

  select, input[type="number"] {
    background: #1a1a2e;
    color: #fff;
    border: 1px solid #3a3a5c;
    padding: 0.4rem 0.8rem;
    border-radius: 6px;
    outline: none;
  }

  .dir-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    margin-bottom: 1rem;
  }

  .dir-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    background: #1a1a2e;
    padding: 0.6rem 0.8rem;
    border-radius: 6px;
    font-size: 0.85rem;
  }

  .remove-btn {
    background: rgba(255, 100, 100, 0.1);
    color: #ff6b6b;
    padding: 0.2rem 0.5rem;
    font-size: 0.8rem;
  }

  .remove-btn:hover {
    background: #ff6b6b;
    color: #fff;
  }

  .add-btn {
    width: 100%;
    background: #30305a;
    font-size: 0.85rem;
    padding: 0.5rem;
  }

  .pattern-list {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    margin-bottom: 1rem;
  }

  .pattern-pill {
    background: #30305a;
    padding: 0.3rem 0.6rem;
    border-radius: 6px;
    font-size: 0.8rem;
    display: flex;
    align-items: center;
    gap: 0.4rem;
    border: 1px solid #3a3a5c;
  }

  .pattern-pill button {
    background: transparent;
    padding: 0;
    color: #6b6b8c;
    font-size: 0.75rem;
  }

  .add-pattern input {
    width: 100%;
    background: #1a1a2e;
    border: 1px solid #3a3a5c;
    padding: 0.6rem;
    border-radius: 6px;
    color: #fff;
    font-size: 0.85rem;
  }

  .empty-hint {
    font-size: 0.85rem;
    color: #6b6b8c;
    text-align: center;
    padding: 1rem;
    border: 1px dashed #3a3a5c;
    border-radius: 6px;
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }
</style>

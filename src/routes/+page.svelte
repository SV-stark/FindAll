<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { openPath } from "@tauri-apps/plugin-opener";
  import { onMount } from "svelte";

  // Types
  interface SearchResult {
    file_path: string;
    title: string | null;
    score: number;
  }

  // State
  let query = $state("");
  let results = $state<SearchResult[]>([]);
  let isSearching = $state(false);
  let isIndexing = $state(false);
  let selectedPath = $state<string | null>(null);
  let previewContent = $state<string | null>(null);

  // Debounce search
  let debounceTimer: ReturnType<typeof setTimeout>;

  async function performSearch() {
    if (!query.trim()) {
      results = [];
      return;
    }

    isSearching = true;
    try {
      results = await invoke<SearchResult[]>("search_query", { query });
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

  onMount(() => {
    window.addEventListener("keydown", handleKeydown);
    return () => {
      window.removeEventListener("keydown", handleKeydown);
    };
  });
</script>

<main class="container">
  <div class="search-container">
    <h1>Flash Search</h1>
    <p class="subtitle">Ultrafast local full-text search</p>

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
</main>

<style>
  :global(*) {
    box-sizing: border-box;
    margin: 0;
    padding: 0;
  }

  :global(body) {
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
    background: #1a1a2e;
    color: #eaeaea;
    min-height: 100vh;
  }

  .container {
    max-width: 1200px;
    margin: 0 auto;
    padding: 2rem;
  }

  .search-container {
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

  @media (prefers-color-scheme: light) {
    :global(body) {
      background: #f5f5f7;
      color: #1a1a2e;
    }

    .search-input {
      background: #ffffff;
      color: #1a1a2e;
      border-color: #e0e0e5;
    }

    .search-input::placeholder {
      color: #999;
    }

    .result-item {
      background: #ffffff;
    }

    .result-item:hover {
      background: #f0f0f5;
    }

    .result-path {
      color: #666;
    }

    .preview-panel {
      background: #ffffff;
    }

    .preview-content {
      color: #333;
    }
  }
</style>

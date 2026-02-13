<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { fade, slide } from "svelte/transition";
  import { appState } from '$lib/state.svelte';
  import { formatBytes } from '$lib/types';
  import PreviewPanel from './PreviewPanel.svelte';

  let showOperatorHelp = $state(false);

  function parseSearchOperators(input: string): { query: string, chips: {type: string, value: string, label: string}[] } {
    const chips: {type: string, value: string, label: string}[] = [];
    let cleanQuery = input;
    
    const extMatch = input.match(/ext:(\w+)/i);
    if (extMatch) {
      chips.push({ type: "ext", value: extMatch[1], label: `Type: ${extMatch[1]}` });
      cleanQuery = cleanQuery.replace(extMatch[0], "").trim();
    }
    
    const pathMatch = input.match(/path:([^\s]+)/i);
    if (pathMatch) {
      chips.push({ type: "path", value: pathMatch[1], label: `Path: ${pathMatch[1]}` });
      cleanQuery = cleanQuery.replace(pathMatch[0], "").trim();
    }
    
    const titleMatch = input.match(/title:([^\s]+)/i);
    if (titleMatch) {
      chips.push({ type: "title", value: titleMatch[1], label: `Title: ${titleMatch[1]}` });
      cleanQuery = cleanQuery.replace(titleMatch[0], "").trim();
    }
    
    const sizeMatch = input.match(/size:([<>]?)(\d+)(MB|KB|GB)?/i);
    if (sizeMatch) {
      const op = sizeMatch[1] || "=";
      const val = sizeMatch[2];
      const unit = sizeMatch[3] || "B";
      chips.push({ type: "size", value: sizeMatch[0], label: `Size ${op}${val}${unit}` });
      cleanQuery = cleanQuery.replace(sizeMatch[0], "").trim();
    }
    
    return { query: cleanQuery || "*", chips };
  }

  function removeChip(index: number) {
    const chip = appState.searchChips[index];
    appState.searchChips = appState.searchChips.filter((_, i) => i !== index);
    appState.query = appState.query.replace(new RegExp(chip.type + ":" + chip.value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), "gi")).trim();
    appState.debouncedSearch();
  }

  async function handleBuildIndex() {
    if (confirm("Filename index not built yet. Build now for instant filename search?")) {
      await appState.buildFilenameIndex();
    }
  }
</script>

<div class="search-tab" in:fade={{ duration: 200 }}>
  <header class="search-header">
    <div class="search-box">
      <span class="search-icon">🔍</span>
      <input
        type="text"
        class="search-input"
        placeholder={appState.searchMode === "filename" ? "Search filenames... (fast mode)" : "Search files... Try: ext:pdf path:docs report"}
        bind:value={appState.query}
        oninput={() => appState.debouncedSearch()}
        onfocus={() => appState.showRecentSearches = appState.recentSearches.length > 0 && appState.settings.search_history_enabled}
        onblur={() => setTimeout(() => appState.showRecentSearches = false, 200)}
      />
      {#if appState.isSearching}
        <div class="spinner"></div>
      {/if}
      <button 
        class="operator-help-btn" 
        onclick={() => showOperatorHelp = !showOperatorHelp}
        title="Search operators"
      >
        ?
      </button>
      
      {#if appState.showRecentSearches && appState.recentSearches.length > 0}
        <div class="recent-searches-dropdown">
          <div class="recent-searches-header">
            <span>Recent Searches</span>
            <button class="clear-recent" onclick={() => { invoke("clear_recent_searches"); appState.recentSearches = []; }}>Clear</button>
          </div>
          {#each appState.recentSearches as search}
            <button 
              class="recent-search-item" 
              onclick={() => { appState.query = search; appState.performSearch(); appState.showRecentSearches = false; }}
            >
              {search}
            </button>
          {/each}
        </div>
      {/if}
    </div>
    
    <div class="search-mode-toggle">
      <button 
        class="mode-btn" 
        class:active={appState.searchMode === "content"}
        onclick={() => appState.searchMode = "content"}
        title="Full-text search (slower but comprehensive)"
      >
        📝 Content
      </button>
      <button 
        class="mode-btn" 
        class:active={appState.searchMode === "filename"}
        onclick={() => {
          if (!appState.filenameIndexReady) {
            handleBuildIndex();
          } else {
            appState.searchMode = "filename";
          }
        }}
        title={appState.filenameIndexReady ? "Filename only (instant results)" : "Click to build filename index"}
      >
        📁 Filename {!appState.filenameIndexReady && "⚠️"}
      </button>
    </div>
    
    <button class="btn-primary" onclick={() => appState.performSearch()}>
      {appState.searchMode === "filename" ? "⚡" : "🔍"} Search
    </button>
  </header>

  {#if showOperatorHelp}
    <div class="operator-help" transition:slide>
      <h4>Search Operators</h4>
      <div class="operator-list">
        <div class="operator-item"><code>ext:pdf</code> - Filter by extension</div>
        <div class="operator-item"><code>path:docs</code> - Search in path</div>
        <div class="operator-item"><code>title:report</code> - Search in title</div>
        <div class="operator-item"><code>size:&gt;1MB</code> - Size filter</div>
      </div>
    </div>
  {/if}

  {#if appState.searchChips.length > 0}
    <div class="search-chips" transition:slide>
      {#each appState.searchChips as chip, i}
        <span class="chip {chip.type}">
          {chip.label}
          <button onclick={() => removeChip(i)}>×</button>
        </span>
      {/each}
    </div>
  {/if}

  <div class="toolbar">
    <div class="filter-group">
      <label>File Type:</label>
      <select class="select-input" bind:value={appState.selectedFileType} onchange={() => { if (appState.query.trim()) appState.performSearch(); }}>
        <option value="all">All Files</option>
        <option value="documents">Documents (*.docx, *.pdf...)</option>
        <option value="code">Code Files (*.rs, *.js...)</option>
        <option value="text">Text Files (*.txt, *.md)</option>
      </select>
    </div>
    
    <div class="quick-access">
      <button class="toolbar-btn" onclick={() => appState.showPinned = !appState.showPinned}>
        📌 Pinned ({appState.pinnedFiles.length})
      </button>
      <button class="toolbar-btn" onclick={() => appState.showRecentFiles = !appState.showRecentFiles}>
        🕐 Recent
      </button>
    </div>
    
    <div class="spacer"></div>
    
    {#if appState.results.length > 0}
      <div class="export-group">
        <button class="toolbar-btn" onclick={() => appState.exportResults('csv')} title="Export as CSV">
          📊 Export CSV
        </button>
        <button class="toolbar-btn" onclick={() => appState.exportResults('json')} title="Export as JSON">
          📋 Export JSON
        </button>
      </div>
    {/if}
    
    <button class="toolbar-btn" class:active={appState.showFilters} onclick={() => appState.showFilters = !appState.showFilters}>
      ⚙️ Filters
    </button>
    <button class="toolbar-btn" onclick={() => appState.startIndexing()} disabled={appState.isIndexing}>
      {appState.isIndexing ? "⏳ Indexing..." : "⚡ Rebuild Index"}
    </button>
  </div>

  {#if appState.showPinned && appState.pinnedFiles.length > 0}
    <div class="pinned-panel" transition:slide>
      <h4>📌 Pinned Files</h4>
      <div class="pinned-list">
        {#each appState.pinnedFiles as path}
          <div class="pinned-item">
            <span class="file-icon">{path.split('.').pop() ? '📄' : '📁'}</span>
            <span class="pinned-path" onclick={() => appState.showPreview(path)}>{path.split(/[\\/]/).pop()}</span>
            <button class="unpin-btn" onclick={() => appState.unpinFile(path)} title="Unpin">×</button>
          </div>
        {/each}
      </div>
    </div>
  {/if}

  {#if appState.showRecentFiles && appState.recentFiles.length > 0}
    <div class="recent-files-panel" transition:slide>
      <h4>🕐 Recently Modified</h4>
      <div class="recent-files-list">
        {#each appState.recentFiles as file}
          <div class="recent-file-item" onclick={() => appState.showPreview(file.path)}>
            <span class="file-icon">📄</span>
            <span class="recent-file-name">{file.title || file.path.split(/[\\/]/).pop()}</span>
            <span class="recent-file-info">{new Date(file.modified * 1000).toLocaleDateString()} • {formatBytes(file.size)}</span>
          </div>
        {/each}
      </div>
    </div>
  {/if}

  {#if appState.showFilters}
    <div class="filter-panel" transition:slide>
      <div class="filter-row">
        <div class="filter-field">
          <label>Min Size (MB)</label>
          <input type="number" bind:value={appState.minSize} oninput={() => appState.debouncedSearch()} placeholder="Any" />
        </div>
        <div class="filter-field">
          <label>Max Size (MB)</label>
          <input type="number" bind:value={appState.maxSize} oninput={() => appState.debouncedSearch()} placeholder="Any" />
        </div>
        <button class="btn-secondary" onclick={() => { appState.minSize = null; appState.maxSize = null; appState.debouncedSearch(); }}>
          Clear Filters
        </button>
      </div>
    </div>
  {/if}

  <div class="content-area">
    <div class="results-panel">
      <div class="results-header">
        <span class="col-name">Name</span>
        <span class="col-path">Path</span>
        <span class="col-actions">Actions</span>
      </div>
      <div class="results-list">
        {#if appState.results.length === 0 && appState.query.trim()}
          <div class="empty-state">
            <div class="empty-icon">🔍</div>
            <p>No results found for "{appState.query}"</p>
            <span class="empty-hint">Try different keywords or adjust filters</span>
          </div>
        {:else if appState.results.length > 0}
          {#each appState.results as result, i}
            <div
              class="result-item"
              class:active={appState.selectedIndex === i}
              data-result-index={i}
              onclick={() => appState.showPreview(result.file_path, i)}
              ondblclick={() => appState.openFile(result.file_path)}
              role="button"
              tabindex="0"
            >
              <div class="col-name">
                <span class="file-icon">📄</span>
                <span class="file-title">{result.title || result.file_path.split(/[\\/]/).pop()}</span>
                {#if appState.pinnedFiles.includes(result.file_path)}
                  <span class="pin-indicator" title="Pinned">📌</span>
                {/if}
              </div>
              <div class="col-path">{result.file_path}</div>
              <div class="col-actions">
                <button 
                  class="action-btn" 
                  title={appState.pinnedFiles.includes(result.file_path) ? "Unpin file" : "Pin file"}
                  onclick={(e) => { 
                    e.stopPropagation(); 
                    appState.pinnedFiles.includes(result.file_path) ? appState.unpinFile(result.file_path) : appState.pinFile(result.file_path);
                  }}
                >
                  {appState.pinnedFiles.includes(result.file_path) ? "📌" : "📍"}
                </button>
                <button 
                  class="action-btn" 
                  title="Copy Path"
                  onclick={(e) => { e.stopPropagation(); appState.copyToClipboard(result.file_path); }}
                >
                  📋
                </button>
                <button 
                  class="action-btn" 
                  title="Open Location"
                  onclick={(e) => { e.stopPropagation(); appState.openFile(result.file_path); }}
                >
                  📂
                </button>
              </div>
            </div>
          {/each}
        {:else}
          <div class="empty-state">
            <div class="empty-icon">📁</div>
            <p>Ready to search</p>
            <span class="empty-hint">Type above to search through indexed files<br>or drag & drop a folder to index</span>
          </div>
        {/if}
      </div>
      <div class="results-footer">
        {#if appState.results.length > 0}
          {appState.results.length} results found
          {#if appState.selectedIndex >= 0}
            • Selected: {appState.selectedIndex + 1} of {appState.results.length} (↑↓ to navigate, Enter to open, Space for quick look)
          {/if}
        {:else}
          No active search
        {/if}
      </div>
    </div>

    {#if appState.selectedPath && appState.settings.show_preview_panel}
      <PreviewPanel />
    {/if}
  </div>
</div>

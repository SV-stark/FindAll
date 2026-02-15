<script lang="ts">
  import { api } from '$lib/api';
  import { fade, slide } from "svelte/transition";
  import { appState } from '$lib/state.svelte';
  import { formatBytes } from '$lib/types';
  import PreviewPanel from './PreviewPanel.svelte';
  import Icon from './Icon.svelte';
  import { getFileColor, getFileIcon } from '$lib/utils/fileTypes';

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
      <span class="search-icon">
        <Icon icon="magnifying-glass" size={18} color="var(--text-secondary)" />
      </span>
      <div class="search-input-wrapper">
        <div class="search-highlight" aria-hidden="true">
          {#each appState.query.matchAll(/(ext:|path:|title:|size:|>|<|=|"[^"]+")/g) as match}
            <span class="operator">{match[0]}</span>
          {/each}
          {#if appState.query && !appState.query.match(/ext:|path:|title:|size:/)}
            <span class="text">{appState.query}</span>
          {/if}
        </div>
        <input
          type="text"
          class="search-input"
          placeholder={appState.searchMode === "filename" ? "Search filenames... (fast mode)" : "Search files... Try: ext:pdf path:docs report"}
          bind:value={appState.query}
          oninput={() => appState.debouncedSearch()}
          onfocus={() => appState.showRecentSearches = appState.recentSearches.length > 0 && appState.settings.search_history_enabled}
          onblur={() => setTimeout(() => appState.showRecentSearches = false, 200)}
        />
      </div>
      {#if appState.isSearching}
        <div class="spinner"></div>
      {/if}
      <button 
        class="operator-help-btn" 
        onclick={() => showOperatorHelp = !showOperatorHelp}
        title="Search operators"
        aria-label="Toggle search help"
      >
        <Icon icon="question" size={16} />
      </button>
      
      {#if appState.showRecentSearches && appState.recentSearches.length > 0}
        <div class="recent-searches-dropdown">
          <div class="recent-searches-header">
            <span>Recent Searches</span>
            <button class="clear-recent" onclick={() => { appState.recentSearches = []; }}>Clear</button>
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
        aria-label="Content search mode"
      >
        <Icon icon="text" size={16} />
        <span>Content</span>
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
        aria-label="Filename search mode"
      >
        <Icon icon="folder-simple" size={16} />
        <span>Filename</span>
        {#if !appState.filenameIndexReady}
          <Icon icon="warning" size={12} color="var(--warning)" />
        {/if}
      </button>
    </div>
    
    <button class="btn-primary" onclick={() => appState.performSearch()}>
      <Icon icon={appState.searchMode === "filename" ? "lightning" : "magnifying-glass"} size={16} />
      <span>Search</span>
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
    
    <div class="toolbar-actions">
      <!-- Sort Dropdown -->
      {#if appState.results.length > 0}
        <div class="dropdown">
          <button class="toolbar-btn" onclick={() => appState.showSortDropdown = !appState.showSortDropdown}>
            <Icon icon="sort" size={14} />
            <span>Sort: {appState.sortBy === 'relevance' ? 'Relevance' : appState.sortBy === 'name' ? 'Name' : appState.sortBy === 'date' ? 'Date' : 'Size'}</span>
            <Icon icon="chevron-down" size={12} />
          </button>
          {#if appState.showSortDropdown}
            <div class="dropdown-menu">
              <button onclick={() => { appState.sortBy = 'relevance'; appState.performSearch(); appState.showSortDropdown = false; }}>Relevance</button>
              <button onclick={() => { appState.sortBy = 'name'; appState.performSearch(); appState.showSortDropdown = false; }}>Name</button>
              <button onclick={() => { appState.sortBy = 'date'; appState.performSearch(); appState.showSortDropdown = false; }}>Date Modified</button>
              <button onclick={() => { appState.sortBy = 'size'; appState.performSearch(); appState.showSortDropdown = false; }}>Size</button>
              <div class="dropdown-divider"></div>
              <button onclick={() => { appState.sortOrder = appState.sortOrder === 'asc' ? 'desc' : 'asc'; appState.performSearch(); appState.showSortDropdown = false; }}>
                {appState.sortOrder === 'asc' ? '↑ Ascending' : '↓ Descending'}
              </button>
            </div>
          {/if}
        </div>

        <!-- Export Dropdown -->
        <div class="dropdown">
          <button class="toolbar-btn" onclick={() => appState.showExportDropdown = !appState.showExportDropdown}>
            <Icon icon="download" size={14} />
            <span>Export</span>
            <Icon icon="chevron-down" size={12} />
          </button>
          {#if appState.showExportDropdown}
            <div class="dropdown-menu">
              <button onclick={() => { appState.exportResults('csv'); appState.showExportDropdown = false; }}>
                <Icon icon="file-text" size={14} />
                Export as CSV
              </button>
              <button onclick={() => { appState.exportResults('json'); appState.showExportDropdown = false; }}>
                <Icon icon="code" size={14} />
                Export as JSON
              </button>
            </div>
          {/if}
        </div>
      {/if}
      
      <!-- Filter Toggle -->
      <button class="toolbar-btn" class:active={appState.showFilters} onclick={() => appState.showFilters = !appState.showFilters}>
        <Icon icon="funnel" size={14} />
        <span>Filters</span>
      </button>
      
      <!-- Rebuild Index -->
      <button class="toolbar-btn" onclick={() => appState.startIndexing()} disabled={appState.isIndexing}>
        <Icon icon={appState.isIndexing ? "spinner" : "refresh"} size={14} />
        <span>{appState.isIndexing ? "Indexing..." : "Rebuild Index"}</span>
      </button>
    </div>
  </div>

  {#if appState.showPinned && appState.pinnedFiles.length > 0}
    <div class="pinned-panel" transition:slide>
      <h4>
        <Icon icon="pin" size={16} />
        <span>Pinned Files</span>
      </h4>
      <div class="pinned-list">
        {#each appState.pinnedFiles as path}
          {@const filename = path.split(/[\\/]/).pop() || ''}
          <div class="pinned-item">
            <span class="file-icon" style="color: {getFileColor(filename)}">
              <Icon icon={getFileIcon(filename)} size={16} />
            </span>
            <span class="pinned-path" onclick={() => appState.showPreview(path)}>{filename}</span>
            <button class="unpin-btn" onclick={() => appState.unpinFile(path)} title="Unpin" aria-label="Unpin file">
              <Icon icon="x" size={14} />
            </button>
          </div>
        {/each}
      </div>
    </div>
  {/if}

  {#if appState.showRecentFiles && appState.recentFiles.length > 0}
    <div class="recent-files-panel" transition:slide>
      <h4>
        <Icon icon="clock" size={16} />
        <span>Recently Modified</span>
      </h4>
      <div class="recent-files-list">
        {#each appState.recentFiles as file}
          {@const filename = file.path.split(/[\\/]/).pop() || ''}
          <div class="recent-file-item" onclick={() => appState.showPreview(file.path)}>
            <span class="file-icon" style="color: {getFileColor(filename)}">
              <Icon icon={getFileIcon(filename)} size={16} />
            </span>
            <span class="recent-file-name">{file.title || filename}</span>
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
            <div class="empty-icon">
              <Icon icon="magnifying-glass" size={48} color="var(--text-muted)" />
            </div>
            <p>No results found for "{appState.query}"</p>
            <span class="empty-hint">Try different keywords or adjust filters</span>
          </div>
        {:else if appState.results.length > 0}
          {#each appState.results as result, i}
            {@const filename = result.file_path.split(/[\\/]/).pop() || ''}
            {@const fileColor = getFileColor(filename)}
            {@const fileIcon = getFileIcon(filename)}
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
                <span class="file-icon" style="color: {fileColor}">
                  <Icon icon={fileIcon} size={18} />
                </span>
                <span class="file-title">{result.title || filename}</span>
                {#if appState.pinnedFiles.includes(result.file_path)}
                  <span class="pin-indicator" title="Pinned">
                    <Icon icon="pin-filled" size={12} color="var(--primary)" />
                  </span>
                {/if}
              </div>
              <div class="col-path">{result.file_path}</div>
              <div class="col-actions">
                <button 
                  class="action-btn" 
                  title={appState.pinnedFiles.includes(result.file_path) ? "Unpin file" : "Pin file"}
                  aria-label={appState.pinnedFiles.includes(result.file_path) ? "Unpin file" : "Pin file"}
                  onclick={(e) => { 
                    e.stopPropagation(); 
                    appState.pinnedFiles.includes(result.file_path) ? appState.unpinFile(result.file_path) : appState.pinFile(result.file_path);
                  }}
                >
                  <Icon icon={appState.pinnedFiles.includes(result.file_path) ? "pin-filled" : "pin"} size={14} />
                </button>
                <button 
                  class="action-btn" 
                  title="Copy Path"
                  aria-label="Copy file path"
                  onclick={(e) => { e.stopPropagation(); appState.copyToClipboard(result.file_path); }}
                >
                  <Icon icon="copy" size={14} />
                </button>
                <button 
                  class="action-btn" 
                  title="Open Location"
                  aria-label="Open file location"
                  onclick={(e) => { e.stopPropagation(); appState.openFile(result.file_path); }}
                >
                  <Icon icon="external-link" size={14} />
                </button>
              </div>
            </div>
          {/each}
        {:else}
          <div class="empty-state">
            <div class="empty-icon">
              <Icon icon="folder" size={48} color="var(--text-muted)" />
            </div>
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

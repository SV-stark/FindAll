<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { fade, slide } from "svelte/transition";

  // Types
  interface SearchResult {
    file_path: string;
    title: string | null;
    score: number;
  }

  interface AppSettings {
    // Indexing
    index_dirs: string[];
    exclude_patterns: string[];
    auto_index_on_startup: boolean;
    index_file_size_limit_mb: number;
    
    // Search
    max_results: number;
    search_history_enabled: boolean;
    fuzzy_matching: boolean;
    case_sensitive: boolean;
    default_filters: {
      file_types: string[];
      min_size_mb: number | null;
      max_size_mb: number | null;
      modified_within_days: number | null;
    };
    
    // Appearance
    theme: "auto" | "light" | "dark";
    font_size: "small" | "medium" | "large";
    show_file_extensions: boolean;
    results_per_page: number;
    
    // Behavior
    minimize_to_tray: boolean;
    auto_start_on_boot: boolean;
    double_click_action: "open_file" | "show_in_folder" | "preview";
    show_preview_panel: boolean;
    
    // Performance
    indexing_threads: number;
    memory_limit_mb: number;
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
  let selectedFileType = $state<string>("all");
  let showRecentSearches = $state(false);
  let recentSearches = $state<string[]>([]);
  
  // Settings state with defaults
  let settings = $state<AppSettings>({
    index_dirs: [],
    exclude_patterns: [],
    auto_index_on_startup: true,
    index_file_size_limit_mb: 100,
    max_results: 50,
    search_history_enabled: true,
    fuzzy_matching: true,
    case_sensitive: false,
    default_filters: {
      file_types: [],
      min_size_mb: null,
      max_size_mb: null,
      modified_within_days: null
    },
    theme: "auto",
    font_size: "medium",
    show_file_extensions: true,
    results_per_page: 50,
    minimize_to_tray: true,
    auto_start_on_boot: false,
    double_click_action: "open_file",
    show_preview_panel: true,
    indexing_threads: 4,
    memory_limit_mb: 512
  });

  // Settings expanded sections
  let expandedSections = $state({
    indexing: true,
    search: false,
    appearance: false,
    behavior: false,
    performance: false,
    advanced: false
  });

  // Settings has unsaved changes
  let hasChanges = $state(false);
  let showSaveSuccess = $state(false);

  function toggleSection(section: keyof typeof expandedSections) {
    expandedSections[section] = !expandedSections[section];
  }

  async function loadSettings() {
    try {
      const loaded = await invoke<AppSettings>("get_settings");
      settings = { ...settings, ...loaded };
      hasChanges = false;
    } catch (e) {
      console.error("Failed to load settings:", e);
    }
  }

  async function saveSettings() {
    try {
      await invoke("save_settings", { settings });
      hasChanges = false;
      showSaveSuccess = true;
      setTimeout(() => showSaveSuccess = false, 2000);
    } catch (e) {
      console.error("Failed to save settings:", e);
    }
  }

  function updateSetting<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
    settings[key] = value;
    hasChanges = true;
  }

  function resetToDefaults() {
    if (confirm("Are you sure you want to reset all settings to defaults?")) {
      settings = {
        index_dirs: [],
        exclude_patterns: [".git/", "node_modules/", "target/", "AppData/", "*.tmp", "*.temp", "Thumbs.db", ".DS_Store"],
        auto_index_on_startup: true,
        index_file_size_limit_mb: 100,
        max_results: 50,
        search_history_enabled: true,
        fuzzy_matching: true,
        case_sensitive: false,
        default_filters: { file_types: [], min_size_mb: null, max_size_mb: null, modified_within_days: null },
        theme: "auto",
        font_size: "medium",
        show_file_extensions: true,
        results_per_page: 50,
        minimize_to_tray: true,
        auto_start_on_boot: false,
        double_click_action: "open_file",
        show_preview_panel: true,
        indexing_threads: 4,
        memory_limit_mb: 512
      };
      hasChanges = true;
    }
  }

  // Index directories
  function addIndexDir(path: string) {
    if (path && !settings.index_dirs.includes(path)) {
      settings.index_dirs = [...settings.index_dirs, path];
      hasChanges = true;
    }
  }

  function removeIndexDir(path: string) {
    settings.index_dirs = settings.index_dirs.filter(p => p !== path);
    hasChanges = true;
  }

  // Exclude patterns
  function addExcludePattern(pattern: string) {
    if (pattern && !settings.exclude_patterns.includes(pattern)) {
      settings.exclude_patterns = [...settings.exclude_patterns, pattern];
      hasChanges = true;
    }
  }

  function removeExcludePattern(pattern: string) {
    settings.exclude_patterns = settings.exclude_patterns.filter(p => p !== pattern);
    hasChanges = true;
  }

  // File type filters
  const availableFileTypes = [
    { value: "docx", label: "Word Documents (.docx)" },
    { value: "pdf", label: "PDF Files (.pdf)" },
    { value: "txt", label: "Text Files (.txt)" },
    { value: "md", label: "Markdown (.md)" },
    { value: "rs", label: "Rust (.rs)" },
    { value: "js", label: "JavaScript (.js)" },
    { value: "ts", label: "TypeScript (.ts)" },
    { value: "html", label: "HTML (.html)" },
    { value: "css", label: "CSS (.css)" },
    { value: "py", label: "Python (.py)" },
    { value: "java", label: "Java (.java)" },
    { value: "cpp", label: "C++ (.cpp)" }
  ];

  function toggleFileType(type: string) {
    const current = settings.default_filters.file_types;
    if (current.includes(type)) {
      settings.default_filters.file_types = current.filter(t => t !== type);
    } else {
      settings.default_filters.file_types = [...current, type];
    }
    hasChanges = true;
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
      // Build file extensions filter based on selected file type
      let fileExtensions: string[] | null = null;
      if (selectedFileType !== "all") {
        const fileTypeMap: Record<string, string[]> = {
          documents: ["docx", "pdf", "odt", "txt", "rtf"],
          code: ["rs", "js", "ts", "jsx", "tsx", "py", "java", "cpp", "c", "h", "go", "rb", "php", "swift", "kt"],
          text: ["txt", "md", "json", "xml", "yaml", "yml", "csv"]
        };
        fileExtensions = fileTypeMap[selectedFileType] || null;
      }

      results = await invoke<SearchResult[]>("search_query", { 
        query, 
        limit: settings.max_results,
        min_size: minSize ? minSize * 1024 * 1024 : null,
        max_size: maxSize ? maxSize * 1024 * 1024 : null,
        file_extensions: fileExtensions
      });

      // Add to recent searches
      if (settings.search_history_enabled && query.trim()) {
        await invoke("add_recent_search", { query: query.trim() });
        loadRecentSearches();
      }
    } catch (e) {
      console.error("Search failed:", e);
      results = [];
    } finally {
      isSearching = false;
    }
  }

  async function loadRecentSearches() {
    try {
      recentSearches = await invoke<string[]>("get_recent_searches");
    } catch (e) {
      console.error("Failed to load recent searches:", e);
    }
  }

  async function copyToClipboard(text: string) {
    try {
      await invoke("copy_to_clipboard", { text });
    } catch (e) {
      console.error("Failed to copy to clipboard:", e);
    }
  }

  async function exportResults(format: 'csv' | 'json' | 'txt') {
    try {
      await invoke("export_results", { results, format });
    } catch (e) {
      console.error("Failed to export results:", e);
    }
  }

  function debouncedSearch() {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(performSearch, 300);
  }

  async function startIndexing() {
    isIndexing = true;
    try {
      const homeDir = await invoke<string>("get_home_dir").catch(() => "./");
      await invoke("start_indexing", { path: homeDir });
    } catch (e) {
      console.error("Indexing failed:", e);
    } finally {
      isIndexing = false;
    }
  }

  async function openFile(path: string) {
    try {
      await invoke("open_folder", { path });
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

  const fontSizeClass = $derived({
    small: "font-small",
    medium: "font-medium", 
    large: "font-large"
  }[settings.font_size]);

  $effect(() => {
    document.body.setAttribute("data-theme", effectiveTheme);
    document.body.className = fontSizeClass;
  });

  onMount(async () => {
    window.addEventListener("keydown", handleKeydown);
    await loadSettings();
    await loadRecentSearches();

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
      {#if hasChanges}
        <span class="unsaved-indicator">●</span>
      {/if}
    </button>
  </nav>

  {#if activeTab === "search"}
    <div class="search-tab" in:fade={{ duration: 200 }}>
      <header class="search-header">
        <div class="search-box">
          <span class="search-icon">🔍</span>
          <input
            type="text"
            class="search-input"
            placeholder="Search files by name, content, or path..."
            bind:value={query}
            oninput={debouncedSearch}
            onfocus={() => showRecentSearches = recentSearches.length > 0 && settings.search_history_enabled}
            onblur={() => setTimeout(() => showRecentSearches = false, 200)}
          />
          {#if isSearching}
            <div class="spinner"></div>
          {/if}
          
          <!-- Recent Searches Dropdown -->
          {#if showRecentSearches && recentSearches.length > 0}
            <div class="recent-searches-dropdown">
              <div class="recent-searches-header">
                <span>Recent Searches</span>
                <button class="clear-recent" onclick={() => { invoke("clear_recent_searches"); recentSearches = []; }}>Clear</button>
              </div>
              {#each recentSearches as search}
                <button 
                  class="recent-search-item" 
                  onclick={() => { query = search; performSearch(); showRecentSearches = false; }}
                >
                  {search}
                </button>
              {/each}
            </div>
          {/if}
          {/if}
        </div>
        <button class="btn-primary" onclick={performSearch}>Search</button>
      </header>

      <div class="toolbar">
        <div class="filter-group">
          <label>File Type:</label>
          <select class="select-input" bind:value={selectedFileType} onchange={() => { if (query.trim()) performSearch(); }}>
            <option value="all">All Files</option>
            <option value="documents">Documents (*.docx, *.pdf...)</option>
            <option value="code">Code Files (*.rs, *.js...)</option>
            <option value="text">Text Files (*.txt, *.md)</option>
          </select>
        </div>
        <div class="spacer"></div>
        {#if results.length > 0}
          <div class="export-group">
            <button class="toolbar-btn" onclick={() => exportResults('csv')} title="Export as CSV">
              📊 Export CSV
            </button>
            <button class="toolbar-btn" onclick={() => exportResults('json')} title="Export as JSON">
              📋 Export JSON
            </button>
          </div>
        {/if}
        <button class="toolbar-btn" class:active={showFilters} onclick={() => showFilters = !showFilters}>
          ⚙️ Filters
        </button>
        <button class="toolbar-btn" onclick={startIndexing} disabled={isIndexing}>
          {isIndexing ? "⏳ Indexing..." : "⚡ Rebuild Index"}
        </button>
      </div>

      {#if showFilters}
        <div class="filter-panel" transition:slide>
          <div class="filter-row">
            <div class="filter-field">
              <label>Min Size (MB)</label>
              <input type="number" bind:value={minSize} oninput={debouncedSearch} placeholder="Any" />
            </div>
            <div class="filter-field">
              <label>Max Size (MB)</label>
              <input type="number" bind:value={maxSize} oninput={debouncedSearch} placeholder="Any" />
            </div>
            <button class="btn-secondary" onclick={() => { minSize = null; maxSize = null; debouncedSearch(); }}>
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
            {#if results.length === 0 && query.trim()}
              <div class="empty-state">
                <div class="empty-icon">🔍</div>
                <p>No results found for "{query}"</p>
                <span class="empty-hint">Try different keywords or adjust filters</span>
              </div>
            {:else if results.length > 0}
              {#each results as result}
                <div
                  class="result-item"
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
                  <div class="col-actions">
                    <button 
                      class="action-btn" 
                      title="Copy Path"
                      onclick={(e) => { e.stopPropagation(); copyToClipboard(result.file_path); }}
                    >
                      📋
                    </button>
                    <button 
                      class="action-btn" 
                      title="Copy Content"
                      onclick={async (e) => { 
                        e.stopPropagation(); 
                        const content = await invoke<string>("get_file_preview", { path: result.file_path });
                        copyToClipboard(content);
                      }}
                    >
                      📄
                    </button>
                    <button 
                      class="action-btn" 
                      title="Open Location"
                      onclick={(e) => { e.stopPropagation(); openFile(result.file_path); }}
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
                <span class="empty-hint">Type above to search through indexed files</span>
              </div>
            {/if}
          </div>
          <div class="results-footer">
            {results.length > 0 ? `${results.length} results found` : 'No active search'}
          </div>
        </div>

        {#if selectedPath && settings.show_preview_panel}
          <div class="preview-panel" transition:fade>
            <div class="preview-header">
              <span class="preview-title">{selectedPath.split(/[\\/]/).pop()}</span>
              <button class="close-btn" onclick={() => { selectedPath = null; previewContent = null; }}>✕</button>
            </div>
            <div class="preview-content">
              {#if previewContent}
                <pre>{previewContent}</pre>
              {:else}
                <div class="preview-loading">Loading...</div>
              {/if}
            </div>
          </div>
        {/if}
      </div>
    </div>
  {:else}
    <div class="settings-container" in:fade={{ duration: 200 }}>
      <div class="settings-header">
        <h1>Settings</h1>
        <div class="header-actions">
          <button class="btn-secondary" onclick={resetToDefaults}>Reset to Defaults</button>
          <button class="btn-primary" onclick={saveSettings} disabled={!hasChanges}>
            {hasChanges ? 'Save Changes' : 'Saved'}
          </button>
        </div>
      </div>

      <!-- Indexing Section -->
      <div class="settings-section">
        <button class="section-header" onclick={() => toggleSection('indexing')}>
          <span class="section-icon">📁</span>
          <div class="section-title">
            <h3>Indexing</h3>
            <p>Configure which folders to index and exclusion patterns</p>
          </div>
          <span class="expand-icon">{expandedSections.indexing ? '▼' : '▶'}</span>
        </button>
        
        {#if expandedSections.indexing}
          <div class="section-content" transition:slide>
            <div class="setting-group">
              <label class="setting-label">Index Locations</label>
              <p class="setting-description">Folders that will be scanned and indexed for searching</p>
              <div class="tag-list">
                {#each settings.index_dirs as dir}
                  <div class="tag">
                    <span class="tag-text">{dir}</span>
                    <button class="tag-remove" onclick={() => removeIndexDir(dir)}>✕</button>
                  </div>
                {/each}
                {#if settings.index_dirs.length === 0}
                  <span class="empty-hint">No directories added (Home folder indexed by default)</span>
                {/if}
              </div>
              <button class="btn-secondary" onclick={async () => {
                try {
                  const selected = await invoke<string | null>("select_folder");
                  if (selected) addIndexDir(selected);
                } catch (e) {
                  console.error("Failed to pick folder:", e);
                }
              }}>
                + Add Folder
              </button>
            </div>

            <div class="setting-group">
              <label class="setting-label">Exclusion Patterns</label>
              <p class="setting-description">Files and folders matching these patterns will be skipped</p>
              <div class="tag-list">
                {#each settings.exclude_patterns as pattern}
                  <div class="tag secondary">
                    <span class="tag-text">{pattern}</span>
                    <button class="tag-remove" onclick={() => removeExcludePattern(pattern)}>✕</button>
                  </div>
                {/each}
              </div>
              <div class="input-with-button">
                <input 
                  type="text" 
                  placeholder="Add pattern (e.g., *.tmp, backup/)"
                  onkeydown={(e) => {
                    if (e.key === 'Enter') {
                      addExcludePattern(e.currentTarget.value);
                      e.currentTarget.value = '';
                    }
                  }}
                />
                <button class="btn-secondary" onclick={(e) => {
                  const input = e.currentTarget.previousElementSibling as HTMLInputElement;
                  addExcludePattern(input.value);
                  input.value = '';
                }}>Add</button>
              </div>
            </div>

            <div class="setting-row">
              <div class="setting-info">
                <label class="setting-label">Auto-index on Startup</label>
                <p class="setting-description">Automatically update index when app starts</p>
              </div>
              <label class="toggle">
                <input 
                  type="checkbox" 
                  checked={settings.auto_index_on_startup}
                  onchange={(e) => updateSetting('auto_index_on_startup', e.currentTarget.checked)}
                />
                <span class="toggle-slider"></span>
              </label>
            </div>

            <div class="setting-row">
              <div class="setting-info">
                <label class="setting-label">Max File Size</label>
                <p class="setting-description">Skip files larger than this limit</p>
              </div>
              <div class="input-with-unit">
                <input 
                  type="number" 
                  min="1" 
                  max="1000"
                  value={settings.index_file_size_limit_mb}
                  onchange={(e) => updateSetting('index_file_size_limit_mb', parseInt(e.currentTarget.value))}
                />
                <span class="unit">MB</span>
              </div>
            </div>
          </div>
        {/if}
      </div>

      <!-- Search Section -->
      <div class="settings-section">
        <button class="section-header" onclick={() => toggleSection('search')}>
          <span class="section-icon">🔍</span>
          <div class="section-title">
            <h3>Search</h3>
            <p>Configure search behavior and default filters</p>
          </div>
          <span class="expand-icon">{expandedSections.search ? '▼' : '▶'}</span>
        </button>
        
        {#if expandedSections.search}
          <div class="section-content" transition:slide>
            <div class="setting-row">
              <div class="setting-info">
                <label class="setting-label">Max Results</label>
                <p class="setting-description">Maximum number of search results to display</p>
              </div>
              <input 
                type="number" 
                class="input-small"
                min="10" 
                max="1000"
                value={settings.max_results}
                onchange={(e) => updateSetting('max_results', parseInt(e.currentTarget.value))}
              />
            </div>

            <div class="setting-row">
              <div class="setting-info">
                <label class="setting-label">Results Per Page</label>
                <p class="setting-description">Number of results shown before pagination</p>
              </div>
              <input 
                type="number" 
                class="input-small"
                min="10" 
                max="200"
                value={settings.results_per_page}
                onchange={(e) => updateSetting('results_per_page', parseInt(e.currentTarget.value))}
              />
            </div>

            <div class="setting-row">
              <div class="setting-info">
                <label class="setting-label">Search History</label>
                <p class="setting-description">Remember recent searches for quick access</p>
              </div>
              <label class="toggle">
                <input 
                  type="checkbox" 
                  checked={settings.search_history_enabled}
                  onchange={(e) => updateSetting('search_history_enabled', e.currentTarget.checked)}
                />
                <span class="toggle-slider"></span>
              </label>
            </div>

            <div class="setting-row">
              <div class="setting-info">
                <label class="setting-label">Fuzzy Matching</label>
                <p class="setting-description">Allow approximate matches for typos</p>
              </div>
              <label class="toggle">
                <input 
                  type="checkbox" 
                  checked={settings.fuzzy_matching}
                  onchange={(e) => updateSetting('fuzzy_matching', e.currentTarget.checked)}
                />
                <span class="toggle-slider"></span>
              </label>
            </div>

            <div class="setting-row">
              <div class="setting-info">
                <label class="setting-label">Case Sensitive</label>
                <p class="setting-description">Match exact capitalization in searches</p>
              </div>
              <label class="toggle">
                <input 
                  type="checkbox" 
                  checked={settings.case_sensitive}
                  onchange={(e) => updateSetting('case_sensitive', e.currentTarget.checked)}
                />
                <span class="toggle-slider"></span>
              </label>
            </div>

            <div class="setting-group">
              <label class="setting-label">Default File Types</label>
              <p class="setting-description">Only search these file types by default (empty = all files)</p>
              <div class="checkbox-grid">
                {#each availableFileTypes as type}
                  <label class="checkbox-item">
                    <input 
                      type="checkbox" 
                      checked={settings.default_filters.file_types.includes(type.value)}
                      onchange={() => toggleFileType(type.value)}
                    />
                    <span>{type.label}</span>
                  </label>
                {/each}
              </div>
            </div>
          </div>
        {/if}
      </div>

      <!-- Appearance Section -->
      <div class="settings-section">
        <button class="section-header" onclick={() => toggleSection('appearance')}>
          <span class="section-icon">🎨</span>
          <div class="section-title">
            <h3>Appearance</h3>
            <p>Customize the look and feel of the application</p>
          </div>
          <span class="expand-icon">{expandedSections.appearance ? '▼' : '▶'}</span>
        </button>
        
        {#if expandedSections.appearance}
          <div class="section-content" transition:slide>
            <div class="setting-row">
              <div class="setting-info">
                <label class="setting-label">Theme</label>
                <p class="setting-description">Choose your preferred color scheme</p>
              </div>
              <select 
                class="select-input"
                value={settings.theme}
                onchange={(e) => updateSetting('theme', e.currentTarget.value as any)}
              >
                <option value="auto">🌓 System Default</option>
                <option value="light">☀️ Light</option>
                <option value="dark">🌙 Dark</option>
              </select>
            </div>

            <div class="setting-row">
              <div class="setting-info">
                <label class="setting-label">Font Size</label>
                <p class="setting-description">Adjust text size throughout the app</p>
              </div>
              <select 
                class="select-input"
                value={settings.font_size}
                onchange={(e) => updateSetting('font_size', e.currentTarget.value as any)}
              >
                <option value="small">Small</option>
                <option value="medium">Medium</option>
                <option value="large">Large</option>
              </select>
            </div>

            <div class="setting-row">
              <div class="setting-info">
                <label class="setting-label">Show File Extensions</label>
                <p class="setting-description">Display file extensions in search results</p>
              </div>
              <label class="toggle">
                <input 
                  type="checkbox" 
                  checked={settings.show_file_extensions}
                  onchange={(e) => updateSetting('show_file_extensions', e.currentTarget.checked)}
                />
                <span class="toggle-slider"></span>
              </label>
            </div>

            <div class="setting-row">
              <div class="setting-info">
                <label class="setting-label">Show Preview Panel</label>
                <p class="setting-description">Show file preview on the right side</p>
              </div>
              <label class="toggle">
                <input 
                  type="checkbox" 
                  checked={settings.show_preview_panel}
                  onchange={(e) => updateSetting('show_preview_panel', e.currentTarget.checked)}
                />
                <span class="toggle-slider"></span>
              </label>
            </div>
          </div>
        {/if}
      </div>

      <!-- Behavior Section -->
      <div class="settings-section">
        <button class="section-header" onclick={() => toggleSection('behavior')}>
          <span class="section-icon">⚡</span>
          <div class="section-title">
            <h3>Behavior</h3>
            <p>Configure how the app behaves and responds to actions</p>
          </div>
          <span class="expand-icon">{expandedSections.behavior ? '▼' : '▶'}</span>
        </button>
        
        {#if expandedSections.behavior}
          <div class="section-content" transition:slide>
            <div class="setting-row">
              <div class="setting-info">
                <label class="setting-label">Minimize to Tray</label>
                <p class="setting-description">Keep app running in system tray when closed</p>
              </div>
              <label class="toggle">
                <input 
                  type="checkbox" 
                  checked={settings.minimize_to_tray}
                  onchange={(e) => updateSetting('minimize_to_tray', e.currentTarget.checked)}
                />
                <span class="toggle-slider"></span>
              </label>
            </div>

            <div class="setting-row">
              <div class="setting-info">
                <label class="setting-label">Start on System Boot</label>
                <p class="setting-description">Launch app automatically when computer starts</p>
              </div>
              <label class="toggle">
                <input 
                  type="checkbox" 
                  checked={settings.auto_start_on_boot}
                  onchange={(e) => updateSetting('auto_start_on_boot', e.currentTarget.checked)}
                />
                <span class="toggle-slider"></span>
              </label>
            </div>

            <div class="setting-row">
              <div class="setting-info">
                <label class="setting-label">Double-Click Action</label>
                <p class="setting-description">What happens when you double-click a result</p>
              </div>
              <select 
                class="select-input"
                value={settings.double_click_action}
                onchange={(e) => updateSetting('double_click_action', e.currentTarget.value as any)}
              >
                <option value="open_file">Open File</option>
                <option value="show_in_folder">Show in Folder</option>
                <option value="preview">Show Preview</option>
              </select>
            </div>
          </div>
        {/if}
      </div>

      <!-- Performance Section -->
      <div class="settings-section">
        <button class="section-header" onclick={() => toggleSection('performance')}>
          <span class="section-icon">🚀</span>
          <div class="section-title">
            <h3>Performance</h3>
            <p>Optimize resource usage and indexing speed</p>
          </div>
          <span class="expand-icon">{expandedSections.performance ? '▼' : '▶'}</span>
        </button>
        
        {#if expandedSections.performance}
          <div class="section-content" transition:slide>
            <div class="setting-row">
              <div class="setting-info">
                <label class="setting-label">Indexing Threads</label>
                <p class="setting-description">Number of CPU cores to use for indexing (restart required)</p>
              </div>
              <input 
                type="number" 
                class="input-small"
                min="1" 
                max="16"
                value={settings.indexing_threads}
                onchange={(e) => updateSetting('indexing_threads', parseInt(e.currentTarget.value))}
              />
            </div>

            <div class="setting-row">
              <div class="setting-info">
                <label class="setting-label">Memory Limit</label>
                <p class="setting-description">Maximum RAM usage for the indexer</p>
              </div>
              <div class="input-with-unit">
                <input 
                  type="number" 
                  min="128" 
                  max="4096"
                  value={settings.memory_limit_mb}
                  onchange={(e) => updateSetting('memory_limit_mb', parseInt(e.currentTarget.value))}
                />
                <span class="unit">MB</span>
              </div>
            </div>
          </div>
        {/if}
      </div>

      <!-- Footer info -->
      <div class="settings-footer">
        <p>Flash Search v0.2.0 • <a href="#" onclick={() => invoke('open_folder', { path: '.' })}>Open Data Folder</a></p>
      </div>
    </div>
  {/if}

  <!-- Save Success Toast -->
  {#if showSaveSuccess}
    <div class="toast success" transition:fade>
      <span>✓ Settings saved successfully</span>
    </div>
  {/if}

  <!-- Indexing Progress -->
  {#if indexProgress.status !== "idle"}
    <div class="progress-toast" class:done={indexProgress.status === "done"}>
      <div class="progress-header">
        <span class="status-text">
          {indexProgress.status === "scanning" ? "📂 Scanning files..." : 
           indexProgress.status === "done" ? "✅ Indexing completed" : 
           `🔄 Indexing: ${indexProgress.processed} / ${indexProgress.total}`}
        </span>
        <span class="percentage">{progressPercentage}%</span>
      </div>
      <div class="progress-bar">
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
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
    margin: 0;
    padding: 0;
    background: #f5f5f5;
    color: #333;
    overflow: hidden;
    height: 100vh;
  }

  :global(body[data-theme="dark"]) {
    background: #1a1a1a;
    color: #e0e0e0;
  }

  /* Font sizes */
  :global(.font-small) { font-size: 12px; }
  :global(.font-medium) { font-size: 14px; }
  :global(.font-large) { font-size: 16px; }

  .container {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }

  /* Tabs */
  .tabs {
    display: flex;
    background: #fff;
    border-bottom: 1px solid #e0e0e0;
    padding: 0;
  }

  :global(body[data-theme="dark"]) .tabs {
    background: #252525;
    border-color: #3a3a3a;
  }

  .tabs button {
    background: transparent;
    border: none;
    padding: 14px 24px;
    font-size: 14px;
    color: #666;
    cursor: pointer;
    border-bottom: 2px solid transparent;
    transition: all 0.2s;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .tabs button:hover {
    color: #333;
    background: #f9f9f9;
  }

  :global(body[data-theme="dark"]) .tabs button:hover {
    color: #fff;
    background: #2a2a2a;
  }

  .tabs button.active {
    color: #0078d4;
    border-bottom-color: #0078d4;
    font-weight: 600;
  }

  :global(body[data-theme="dark"]) .tabs button.active {
    color: #4fc3f7;
    border-bottom-color: #4fc3f7;
  }

  .unsaved-indicator {
    color: #ff9800;
    font-size: 8px;
  }

  /* Search Tab */
  .search-tab {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .search-header {
    background: #fff;
    padding: 24px 32px;
    border-bottom: 1px solid #e0e0e0;
    display: flex;
    gap: 12px;
  }

  :global(body[data-theme="dark"]) .search-header {
    background: #252525;
    border-color: #3a3a3a;
  }

  .search-box {
    flex: 1;
    display: flex;
    align-items: center;
    background: #f5f5f5;
    border: 1px solid #e0e0e0;
    border-radius: 8px;
    padding: 0 16px;
    transition: all 0.2s;
    position: relative;
  }

  :global(body[data-theme="dark"]) .search-box {
    background: #1a1a1a;
    border-color: #3a3a3a;
  }

  .search-box:focus-within {
    border-color: #0078d4;
    box-shadow: 0 0 0 3px rgba(0, 120, 212, 0.1);
  }

  .search-icon {
    font-size: 18px;
    margin-right: 12px;
  }

  .search-input {
    flex: 1;
    border: none;
    background: transparent;
    padding: 14px 0;
    font-size: 16px;
    outline: none;
    color: inherit;
  }

  .spinner {
    width: 20px;
    height: 20px;
    border: 2px solid #ddd;
    border-top-color: #0078d4;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  /* Toolbar */
  .toolbar {
    display: flex;
    align-items: center;
    padding: 12px 32px;
    background: #fafafa;
    border-bottom: 1px solid #e0e0e0;
    gap: 16px;
  }

  :global(body[data-theme="dark"]) .toolbar {
    background: #202020;
    border-color: #3a3a3a;
  }

  .filter-group {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .filter-group label {
    font-size: 13px;
    color: #666;
  }

  :global(body[data-theme="dark"]) .filter-group label {
    color: #999;
  }

  .select-input {
    padding: 6px 12px;
    border: 1px solid #ddd;
    border-radius: 6px;
    background: #fff;
    font-size: 13px;
    outline: none;
    color: inherit;
  }

  :global(body[data-theme="dark"]) .select-input {
    background: #1a1a1a;
    border-color: #444;
    color: #e0e0e0;
  }

  .toolbar-btn {
    background: #fff;
    border: 1px solid #ddd;
    padding: 8px 16px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 13px;
    transition: all 0.2s;
  }

  :global(body[data-theme="dark"]) .toolbar-btn {
    background: #1a1a1a;
    border-color: #444;
    color: #e0e0e0;
  }

  .toolbar-btn:hover {
    background: #f5f5f5;
  }

  .toolbar-btn.active {
    border-color: #0078d4;
    color: #0078d4;
  }

  .spacer {
    flex: 1;
  }

  /* Filter Panel */
  .filter-panel {
    background: #fafafa;
    border-bottom: 1px solid #e0e0e0;
    padding: 16px 32px;
  }

  :global(body[data-theme="dark"]) .filter-panel {
    background: #202020;
    border-color: #3a3a3a;
  }

  .filter-row {
    display: flex;
    gap: 16px;
    align-items: flex-end;
  }

  .filter-field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .filter-field label {
    font-size: 12px;
    color: #666;
  }

  .filter-field input {
    width: 120px;
    padding: 8px 12px;
    border: 1px solid #ddd;
    border-radius: 6px;
    font-size: 13px;
  }

  /* Content Area */
  .content-area {
    flex: 1;
    display: flex;
    overflow: hidden;
  }

  .results-panel {
    flex: 1;
    display: flex;
    flex-direction: column;
    background: #fff;
  }

  :global(body[data-theme="dark"]) .results-panel {
    background: #1a1a1a;
  }

  .results-header {
    display: flex;
    padding: 12px 32px;
    background: #fafafa;
    border-bottom: 1px solid #e0e0e0;
    font-weight: 600;
    font-size: 12px;
    color: #666;
  }

  :global(body[data-theme="dark"]) .results-header {
    background: #252525;
    border-color: #3a3a3a;
    color: #999;
  }

  .results-list {
    flex: 1;
    overflow-y: auto;
  }

  .result-item {
    display: flex;
    align-items: center;
    padding: 12px 32px;
    border-bottom: 1px solid #f0f0f0;
    cursor: pointer;
    transition: background 0.15s;
  }

  :global(body[data-theme="dark"]) .result-item {
    border-color: #2a2a2a;
  }

  .result-item:hover {
    background: #f5f5f5;
  }

  :global(body[data-theme="dark"]) .result-item:hover {
    background: #252525;
  }

  .result-item.active {
    background: #e3f2fd;
    border-left: 3px solid #0078d4;
  }

  :global(body[data-theme="dark"]) .result-item.active {
    background: #1e3a5f;
    border-left-color: #4fc3f7;
  }

  .col-name {
    flex: 2;
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }

  .file-icon {
    font-size: 11px;
    background: #e0e0e0;
    padding: 3px 6px;
    border-radius: 4px;
    font-weight: 600;
    color: #555;
    min-width: 36px;
    text-align: center;
  }

  :global(body[data-theme="dark"]) .file-icon {
    background: #3a3a3a;
    color: #aaa;
  }

  .file-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .col-path {
    flex: 3;
    color: #888;
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .col-actions {
    width: 80px;
    text-align: center;
  }

  .action-btn {
    background: transparent;
    border: none;
    padding: 6px;
    cursor: pointer;
    border-radius: 4px;
    opacity: 0.6;
    transition: all 0.2s;
  }

  .action-btn:hover {
    opacity: 1;
    background: rgba(0, 0, 0, 0.05);
  }

  .results-footer {
    padding: 10px 32px;
    background: #fafafa;
    border-top: 1px solid #e0e0e0;
    font-size: 12px;
    color: #888;
  }

  :global(body[data-theme="dark"]) .results-footer {
    background: #252525;
    border-color: #3a3a3a;
  }

  /* Empty State */
  .empty-state {
    text-align: center;
    padding: 80px 20px;
    color: #888;
  }

  .empty-icon {
    font-size: 48px;
    margin-bottom: 16px;
    opacity: 0.5;
  }

  .empty-state p {
    font-size: 16px;
    margin: 0 0 8px;
  }

  .empty-hint {
    font-size: 13px;
    color: #aaa;
  }

  /* Preview Panel */
  .preview-panel {
    width: 400px;
    border-left: 1px solid #e0e0e0;
    background: #fff;
    display: flex;
    flex-direction: column;
  }

  :global(body[data-theme="dark"]) .preview-panel {
    background: #1a1a1a;
    border-color: #3a3a3a;
  }

  .preview-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px;
    border-bottom: 1px solid #e0e0e0;
    background: #fafafa;
  }

  :global(body[data-theme="dark"]) .preview-header {
    background: #252525;
    border-color: #3a3a3a;
  }

  .preview-title {
    font-weight: 600;
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .close-btn {
    background: transparent;
    border: none;
    font-size: 18px;
    cursor: pointer;
    color: #888;
    padding: 4px;
  }

  .preview-content {
    flex: 1;
    overflow: auto;
    padding: 20px;
  }

  .preview-content pre {
    margin: 0;
    font-family: "Consolas", "Monaco", monospace;
    font-size: 12px;
    line-height: 1.6;
    white-space: pre-wrap;
    word-wrap: break-word;
  }

  /* Buttons */
  .btn-primary {
    background: #0078d4;
    color: white;
    border: none;
    padding: 12px 24px;
    border-radius: 6px;
    font-size: 14px;
    cursor: pointer;
    transition: all 0.2s;
    font-weight: 500;
  }

  .btn-primary:hover:not(:disabled) {
    background: #005a9e;
  }

  .btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-secondary {
    background: #fff;
    color: #333;
    border: 1px solid #ddd;
    padding: 8px 16px;
    border-radius: 6px;
    font-size: 13px;
    cursor: pointer;
    transition: all 0.2s;
  }

  :global(body[data-theme="dark"]) .btn-secondary {
    background: #2a2a2a;
    color: #e0e0e0;
    border-color: #444;
  }

  .btn-secondary:hover {
    background: #f5f5f5;
  }

  /* Settings Container */
  .settings-container {
    flex: 1;
    overflow-y: auto;
    padding: 32px;
    max-width: 900px;
    margin: 0 auto;
    width: 100%;
  }

  .settings-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 32px;
    padding-bottom: 24px;
    border-bottom: 1px solid #e0e0e0;
  }

  :global(body[data-theme="dark"]) .settings-header {
    border-color: #3a3a3a;
  }

  .settings-header h1 {
    margin: 0;
    font-size: 28px;
    font-weight: 600;
  }

  .header-actions {
    display: flex;
    gap: 12px;
  }

  /* Settings Section */
  .settings-section {
    background: #fff;
    border: 1px solid #e0e0e0;
    border-radius: 12px;
    margin-bottom: 16px;
    overflow: hidden;
  }

  :global(body[data-theme="dark"]) .settings-section {
    background: #252525;
    border-color: #3a3a3a;
  }

  .section-header {
    width: 100%;
    background: transparent;
    border: none;
    padding: 20px 24px;
    display: flex;
    align-items: center;
    gap: 16px;
    cursor: pointer;
    text-align: left;
    transition: background 0.2s;
  }

  .section-header:hover {
    background: #f9f9f9;
  }

  :global(body[data-theme="dark"]) .section-header:hover {
    background: #2a2a2a;
  }

  .section-icon {
    font-size: 24px;
    width: 40px;
    height: 40px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #f0f0f0;
    border-radius: 8px;
  }

  :global(body[data-theme="dark"]) .section-icon {
    background: #3a3a3a;
  }

  .section-title {
    flex: 1;
  }

  .section-title h3 {
    margin: 0 0 4px;
    font-size: 16px;
    font-weight: 600;
  }

  .section-title p {
    margin: 0;
    font-size: 13px;
    color: #888;
  }

  .expand-icon {
    color: #888;
    font-size: 12px;
  }

  .section-content {
    padding: 0 24px 24px 80px;
    border-top: 1px solid #f0f0f0;
  }

  :global(body[data-theme="dark"]) .section-content {
    border-color: #3a3a3a;
  }

  /* Setting Row */
  .setting-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 0;
    border-bottom: 1px solid #f0f0f0;
  }

  :global(body[data-theme="dark"]) .setting-row {
    border-color: #3a3a3a;
  }

  .setting-row:last-child {
    border-bottom: none;
  }

  .setting-info {
    flex: 1;
  }

  .setting-label {
    display: block;
    font-weight: 500;
    margin-bottom: 4px;
    font-size: 14px;
  }

  .setting-description {
    margin: 0;
    font-size: 12px;
    color: #888;
  }

  /* Setting Group */
  .setting-group {
    padding: 16px 0;
    border-bottom: 1px solid #f0f0f0;
  }

  :global(body[data-theme="dark"]) .setting-group {
    border-color: #3a3a3a;
  }

  .setting-group:last-child {
    border-bottom: none;
  }

  /* Toggle Switch */
  .toggle {
    position: relative;
    display: inline-block;
    width: 48px;
    height: 24px;
  }

  .toggle input {
    opacity: 0;
    width: 0;
    height: 0;
  }

  .toggle-slider {
    position: absolute;
    cursor: pointer;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-color: #ccc;
    transition: 0.3s;
    border-radius: 24px;
  }

  :global(body[data-theme="dark"]) .toggle-slider {
    background-color: #555;
  }

  .toggle-slider:before {
    position: absolute;
    content: "";
    height: 18px;
    width: 18px;
    left: 3px;
    bottom: 3px;
    background-color: white;
    transition: 0.3s;
    border-radius: 50%;
  }

  .toggle input:checked + .toggle-slider {
    background-color: #0078d4;
  }

  .toggle input:checked + .toggle-slider:before {
    transform: translateX(24px);
  }

  /* Tag List */
  .tag-list {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin: 12px 0;
  }

  .tag {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: #e3f2fd;
    color: #0078d4;
    padding: 6px 12px;
    border-radius: 6px;
    font-size: 13px;
    font-weight: 500;
  }

  :global(body[data-theme="dark"]) .tag {
    background: #1e3a5f;
    color: #4fc3f7;
  }

  .tag.secondary {
    background: #f5f5f5;
    color: #666;
  }

  :global(body[data-theme="dark"]) .tag.secondary {
    background: #3a3a3a;
    color: #aaa;
  }

  .tag-text {
    max-width: 300px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tag-remove {
    background: transparent;
    border: none;
    color: inherit;
    cursor: pointer;
    padding: 0;
    font-size: 14px;
    opacity: 0.6;
  }

  .tag-remove:hover {
    opacity: 1;
  }

  /* Input with button */
  .input-with-button {
    display: flex;
    gap: 8px;
  }

  .input-with-button input {
    flex: 1;
    padding: 10px 12px;
    border: 1px solid #ddd;
    border-radius: 6px;
    font-size: 13px;
    outline: none;
    color: inherit;
    background: #fff;
  }

  :global(body[data-theme="dark"]) .input-with-button input {
    background: #1a1a1a;
    border-color: #444;
    color: #e0e0e0;
  }

  .input-with-button input:focus {
    border-color: #0078d4;
  }

  /* Input with unit */
  .input-with-unit {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .input-with-unit input {
    width: 80px;
    padding: 8px 12px;
    border: 1px solid #ddd;
    border-radius: 6px;
    font-size: 13px;
    text-align: right;
    color: inherit;
    background: #fff;
  }

  :global(body[data-theme="dark"]) .input-with-unit input {
    background: #1a1a1a;
    border-color: #444;
    color: #e0e0e0;
  }

  .unit {
    font-size: 13px;
    color: #888;
  }

  .input-small {
    width: 80px;
    padding: 8px 12px;
    border: 1px solid #ddd;
    border-radius: 6px;
    font-size: 13px;
    text-align: center;
    color: inherit;
    background: #fff;
  }

  :global(body[data-theme="dark"]) .input-small {
    background: #1a1a1a;
    border-color: #444;
    color: #e0e0e0;
  }

  /* Checkbox Grid */
  .checkbox-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 10px;
    margin-top: 12px;
  }

  .checkbox-item {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    cursor: pointer;
  }

  .checkbox-item input {
    cursor: pointer;
  }

  /* Settings Footer */
  .settings-footer {
    text-align: center;
    padding: 32px;
    color: #888;
    font-size: 13px;
  }

  .settings-footer a {
    color: #0078d4;
    text-decoration: none;
  }

  .settings-footer a:hover {
    text-decoration: underline;
  }

  /* Toast */
  .toast {
    position: fixed;
    bottom: 24px;
    left: 50%;
    transform: translateX(-50%);
    background: #333;
    color: white;
    padding: 12px 24px;
    border-radius: 8px;
    font-size: 14px;
    z-index: 1000;
  }

  .toast.success {
    background: #4caf50;
  }

  /* Progress Toast */
  .progress-toast {
    position: fixed;
    bottom: 24px;
    right: 24px;
    width: 320px;
    background: #fff;
    border: 1px solid #e0e0e0;
    border-radius: 12px;
    padding: 16px;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.15);
    z-index: 100;
  }

  :global(body[data-theme="dark"]) .progress-toast {
    background: #252525;
    border-color: #3a3a3a;
  }

  .progress-toast.done {
    border-color: #4caf50;
  }

  .progress-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 10px;
    font-size: 13px;
  }

  .status-text {
    font-weight: 500;
  }

  .percentage {
    color: #0078d4;
    font-weight: 600;
  }

  .progress-bar {
    height: 6px;
    background: #e0e0e0;
    border-radius: 3px;
    overflow: hidden;
  }

  :global(body[data-theme="dark"]) .progress-bar {
    background: #3a3a3a;
  }

  .progress-fill {
    height: 100%;
    background: #0078d4;
    transition: width 0.3s;
    border-radius: 3px;
  }

  .current-file {
    margin-top: 8px;
    font-size: 11px;
    color: #888;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Recent Searches Dropdown */
  .recent-searches-dropdown {
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    background: #fff;
    border: 1px solid #e0e0e0;
    border-radius: 0 0 8px 8px;
    box-shadow: 0 4px 12px rgba(0,0,0,0.15);
    z-index: 100;
    max-height: 250px;
    overflow-y: auto;
  }

  :global(body[data-theme="dark"]) .recent-searches-dropdown {
    background: #252525;
    border-color: #3a3a3a;
  }

  .recent-searches-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 16px;
    border-bottom: 1px solid #f0f0f0;
    font-size: 12px;
    color: #888;
  }

  :global(body[data-theme="dark"]) .recent-searches-header {
    border-color: #3a3a3a;
  }

  .clear-recent {
    background: transparent;
    border: none;
    color: #0078d4;
    font-size: 11px;
    cursor: pointer;
  }

  .recent-search-item {
    display: block;
    width: 100%;
    padding: 10px 16px;
    text-align: left;
    background: transparent;
    border: none;
    font-size: 14px;
    cursor: pointer;
    transition: background 0.15s;
  }

  .recent-search-item:hover {
    background: #f5f5f5;
  }

  :global(body[data-theme="dark"]) .recent-search-item:hover {
    background: #2a2a2a;
  }

  /* Export Group */
  .export-group {
    display: flex;
    gap: 8px;
  }

  .export-group .toolbar-btn {
    font-size: 12px;
    padding: 6px 12px;
  }
</style>
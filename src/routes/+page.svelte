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
    matched_terms: string[];
  }

  interface RecentFile {
    path: string;
    title: string | null;
    modified: number;
    size: number;
  }

  interface PreviewResult {
    content: string;
    matched_terms: string[];
  }

  interface IndexStats {
    total_documents: number;
    total_size_bytes: number;
    last_updated: string | null;
  }

  interface AppSettings {
    index_dirs: string[];
    exclude_patterns: string[];
    auto_index_on_startup: boolean;
    index_file_size_limit_mb: number;
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
    theme: "auto" | "light" | "dark";
    font_size: "small" | "medium" | "large";
    show_file_extensions: boolean;
    results_per_page: number;
    minimize_to_tray: boolean;
    auto_start_on_boot: boolean;
    double_click_action: "open_file" | "show_in_folder" | "preview";
    show_preview_panel: boolean;
    indexing_threads: number;
    memory_limit_mb: number;
    pinned_files: string[];
  }

  // State
  let activeTab = $state<"search" | "settings" | "stats">("search");
  let query = $state("");
  let results = $state<SearchResult[]>([]);
  let isSearching = $state(false);
  let isIndexing = $state(false);
  let selectedIndex = $state(-1);
  
  // Search operators and chips
  let searchChips = $state<{type: string, value: string, label: string}[]>([]);
  let showOperatorHelp = $state(false);
  
  // Search filters
  let minSize = $state<number | null>(null);
  let maxSize = $state<number | null>(null);
  let showFilters = $state(false);
  let selectedFileType = $state<string>("all");
  let showRecentSearches = $state(false);
  let recentSearches = $state<string[]>([]);
  
  // Pinned and recent files
  let pinnedFiles = $state<string[]>([]);
  let recentFiles = $state<RecentFile[]>([]);
  let showPinned = $state(false);
  let showRecentFiles = $state(false);
  
  // Statistics
  let indexStats = $state<IndexStats | null>(null);
  
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
    memory_limit_mb: 512,
    pinned_files: []
  });

  let expandedSections = $state({
    indexing: true,
    search: false,
    appearance: false,
    behavior: false,
    performance: false,
    advanced: false
  });

  let hasChanges = $state(false);
  let showSaveSuccess = $state(false);

  // Preview state
  let selectedPath = $state<string | null>(null);
  let previewContent = $state<string | null>(null);
  let highlightedPreview = $state<string>("");
  let isQuickLookOpen = $state(false);
  
  // Drag and drop state
  let isDragging = $state(false);
  
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

  // File type icons mapping
  const fileTypeIcons: Record<string, string> = {
    pdf: "📄",
    docx: "📝",
    xlsx: "📊",
    pptx: "📽️",
    txt: "📃",
    md: "📝",
    rs: "🦀",
    js: "📜",
    ts: "📘",
    html: "🌐",
    css: "🎨",
    py: "🐍",
    java: "☕",
    cpp: "⚙️",
    c: "⚙️",
    go: "🐹",
    rb: "💎",
    php: "🐘",
    swift: "🦉",
    kt: "🎯",
    json: "🔧",
    xml: "📋",
    yaml: "⚙️",
    sql: "🗄️",
    sh: "🐚",
    ps1: "💻",
    default: "📄"
  };

  function getFileIcon(path: string): string {
    const ext = path.split(".").pop()?.toLowerCase() || "";
    return fileTypeIcons[ext] || fileTypeIcons.default;
  }

  function formatBytes(bytes: number): string {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  }

  function formatDate(timestamp: number): string {
    return new Date(timestamp * 1000).toLocaleDateString();
  }

  function toggleSection(section: keyof typeof expandedSections) {
    expandedSections[section] = !expandedSections[section];
  }

  async function loadSettings() {
    try {
      const loaded = await invoke<AppSettings>("get_settings");
      settings = { ...settings, ...loaded };
      pinnedFiles = loaded.pinned_files || [];
      hasChanges = false;
    } catch (e) {
      console.error("Failed to load settings:", e);
    }
  }

  async function loadStatistics() {
    try {
      indexStats = await invoke<IndexStats>("get_index_statistics");
    } catch (e) {
      console.error("Failed to load statistics:", e);
    }
  }

  async function loadRecentFiles() {
    try {
      recentFiles = await invoke<RecentFile[]>("get_recent_files", { limit: 10 });
    } catch (e) {
      console.error("Failed to load recent files:", e);
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
        memory_limit_mb: 512,
        pinned_files: []
      };
      hasChanges = true;
    }
  }

  // Search operators parsing
  function parseSearchOperators(input: string): { query: string, chips: typeof searchChips } {
    const chips: typeof searchChips = [];
    let cleanQuery = input;
    
    // Parse ext: operator
    const extMatch = input.match(/ext:(\w+)/i);
    if (extMatch) {
      chips.push({ type: "ext", value: extMatch[1], label: `Type: ${extMatch[1]}` });
      cleanQuery = cleanQuery.replace(extMatch[0], "").trim();
    }
    
    // Parse path: operator
    const pathMatch = input.match(/path:([^\s]+)/i);
    if (pathMatch) {
      chips.push({ type: "path", value: pathMatch[1], label: `Path: ${pathMatch[1]}` });
      cleanQuery = cleanQuery.replace(pathMatch[0], "").trim();
    }
    
    // Parse title: operator
    const titleMatch = input.match(/title:([^\s]+)/i);
    if (titleMatch) {
      chips.push({ type: "title", value: titleMatch[1], label: `Title: ${titleMatch[1]}` });
      cleanQuery = cleanQuery.replace(titleMatch[0], "").trim();
    }
    
    // Parse size operators
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
    const chip = searchChips[index];
    searchChips = searchChips.filter((_, i) => i !== index);
    // Rebuild query without this chip
    query = query.replace(new RegExp(chip.type + ":" + chip.value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), "gi"), "").trim();
    debouncedSearch();
  }

  async function performSearch() {
    if (!query.trim() && searchChips.length === 0) {
      results = [];
      return;
    }

    isSearching = true;
    selectedIndex = -1;
    
    try {
      // Parse operators from query
      const { query: cleanQuery, chips } = parseSearchOperators(query);
      searchChips = chips;
      
      // Build file extensions filter
      let fileExtensions: string[] | null = null;
      if (selectedFileType !== "all") {
        const fileTypeMap: Record<string, string[]> = {
          documents: ["docx", "pdf", "odt", "txt", "rtf"],
          code: ["rs", "js", "ts", "jsx", "tsx", "py", "java", "cpp", "c", "h", "go", "rb", "php", "swift", "kt"],
          text: ["txt", "md", "json", "xml", "yaml", "yml", "csv"]
        };
        fileExtensions = fileTypeMap[selectedFileType] || null;
      }
      
      // Add extension from chip if present
      const extChip = searchChips.find(c => c.type === "ext");
      if (extChip) {
        fileExtensions = fileExtensions ? [...fileExtensions, extChip.value] : [extChip.value];
      }

      results = await invoke<SearchResult[]>("search_query", { 
        query: query.trim() || "*",
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

  function highlightText(text: string, terms: string[]): string {
    if (!terms.length) return escapeHtml(text);
    
    let highlighted = escapeHtml(text);
    terms.forEach(term => {
      const regex = new RegExp(`(${escapeRegex(term)})`, "gi");
      highlighted = highlighted.replace(regex, '<mark class="search-highlight">$1</mark>');
    });
    
    return highlighted;
  }

  function escapeHtml(text: string): string {
    const div = document.createElement("div");
    div.textContent = text;
    return div.innerHTML;
  }

  function escapeRegex(string: string): string {
    return string.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  }

  async function showPreview(path: string, index?: number) {
    selectedPath = path;
    if (index !== undefined) selectedIndex = index;
    
    try {
      const result = await invoke<PreviewResult>("get_file_preview_highlighted", { 
        path,
        query: query.trim() || "*"
      });
      previewContent = result.content;
      highlightedPreview = highlightText(result.content, result.matched_terms);
    } catch (e) {
      previewContent = "Failed to load preview";
      highlightedPreview = "Failed to load preview";
    }
  }

  function openQuickLook() {
    if (selectedPath) {
      isQuickLookOpen = true;
    }
  }

  function closeQuickLook() {
    isQuickLookOpen = false;
  }

  async function pinFile(path: string) {
    try {
      await invoke("pin_file", { path });
      pinnedFiles = [...pinnedFiles, path];
    } catch (e) {
      console.error("Failed to pin file:", e);
    }
  }

  async function unpinFile(path: string) {
    try {
      await invoke("unpin_file", { path });
      pinnedFiles = pinnedFiles.filter(p => p !== path);
    } catch (e) {
      console.error("Failed to unpin file:", e);
    }
  }

  async function loadRecentSearches() {
    try {
      recentSearches = await invoke<string[]>("get_recent_searches");
    } catch (e) {
      console.error("Failed to load recent searches:", e);
    }
  }

  async function loadPinnedFiles() {
    try {
      pinnedFiles = await invoke<string[]>("get_pinned_files");
    } catch (e) {
      console.error("Failed to load pinned files:", e);
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

  async function startIndexing(path?: string) {
    isIndexing = true;
    try {
      const homeDir = path || await invoke<string>("get_home_dir").catch(() => "./");
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

  // Keyboard navigation
  function handleKeydown(event: KeyboardEvent) {
    // Global shortcuts
    if (event.key === "Escape") {
      if (isQuickLookOpen) {
        closeQuickLook();
        return;
      }
      query = "";
      results = [];
      selectedPath = null;
      previewContent = null;
      selectedIndex = -1;
      return;
    }

    // Quick look with spacebar
    if (event.code === "Space" && selectedPath && !isQuickLookOpen && document.activeElement?.tagName !== "INPUT") {
      event.preventDefault();
      openQuickLook();
      return;
    }

    // Navigation when results exist
    if (results.length > 0) {
      if (event.key === "ArrowDown") {
        event.preventDefault();
        selectedIndex = Math.min(selectedIndex + 1, results.length - 1);
        const result = results[selectedIndex];
        if (result) {
          showPreview(result.file_path, selectedIndex);
          // Scroll into view
          setTimeout(() => {
            const element = document.querySelector(`[data-result-index="${selectedIndex}"]`);
            element?.scrollIntoView({ behavior: "smooth", block: "nearest" });
          }, 10);
        }
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        selectedIndex = Math.max(selectedIndex - 1, 0);
        const result = results[selectedIndex];
        if (result) {
          showPreview(result.file_path, selectedIndex);
          setTimeout(() => {
            const element = document.querySelector(`[data-result-index="${selectedIndex}"]`);
            element?.scrollIntoView({ behavior: "smooth", block: "nearest" });
          }, 10);
        }
      } else if (event.key === "Enter" && selectedIndex >= 0) {
        event.preventDefault();
        const result = results[selectedIndex];
        if (result) {
          openFile(result.file_path);
        }
      }
    }

    // Pin shortcut (Ctrl/Cmd + P)
    if ((event.ctrlKey || event.metaKey) && event.key === "p" && selectedPath) {
      event.preventDefault();
      if (pinnedFiles.includes(selectedPath)) {
        unpinFile(selectedPath);
      } else {
        pinFile(selectedPath);
      }
    }
  }

  // Drag and drop handlers
  function handleDragOver(event: DragEvent) {
    event.preventDefault();
    event.stopPropagation();
    isDragging = true;
  }

  function handleDragLeave(event: DragEvent) {
    event.preventDefault();
    event.stopPropagation();
    isDragging = false;
  }

  async function handleDrop(event: DragEvent) {
    event.preventDefault();
    event.stopPropagation();
    isDragging = false;
    
    const files = event.dataTransfer?.files;
    if (files && files.length > 0) {
      const path = files[0].path || files[0].name;
      if (confirm(`Index folder: ${path}?`)) {
        await startIndexing(path);
      }
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
    await loadPinnedFiles();
    await loadRecentFiles();
    await loadStatistics();

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
          loadStatistics();
          loadRecentFiles();
        }, 5000);
      }
    });

    return () => {
      window.removeEventListener("keydown", handleKeydown);
      unlisten();
    };
  });
</script>

<main 
  class="container"
  class:dragging={isDragging}
  ondragover={handleDragOver}
  ondragleave={handleDragLeave}
  ondrop={handleDrop}
>
  {#if isDragging}
    <div class="drop-overlay" transition:fade>
      <div class="drop-message">
        <span class="drop-icon">📁</span>
        <p>Drop folder here to index</p>
      </div>
    </div>
  {/if}

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
    <button class:active={activeTab === "stats"} onclick={() => { activeTab = "stats"; loadStatistics(); }}>
      <span class="tab-icon">📊</span> Stats
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
            placeholder="Search files... Try: ext:pdf path:docs report"
            bind:value={query}
            oninput={debouncedSearch}
            onfocus={() => showRecentSearches = recentSearches.length > 0 && settings.search_history_enabled}
            onblur={() => setTimeout(() => showRecentSearches = false, 200)}
          />
          {#if isSearching}
            <div class="spinner"></div>
          {/if}
          <button 
            class="operator-help-btn" 
            onclick={() => showOperatorHelp = !showOperatorHelp}
            title="Search operators"
          >
            ?
          </button>
          
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
        </div>
        <button class="btn-primary" onclick={performSearch}>Search</button>
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

      {#if searchChips.length > 0}
        <div class="search-chips" transition:slide>
          {#each searchChips as chip, i}
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
          <select class="select-input" bind:value={selectedFileType} onchange={() => { if (query.trim()) performSearch(); }}>
            <option value="all">All Files</option>
            <option value="documents">Documents (*.docx, *.pdf...)</option>
            <option value="code">Code Files (*.rs, *.js...)</option>
            <option value="text">Text Files (*.txt, *.md)</option>
          </select>
        </div>
        
        <div class="quick-access">
          <button class="toolbar-btn" onclick={() => showPinned = !showPinned}>
            📌 Pinned ({pinnedFiles.length})
          </button>
          <button class="toolbar-btn" onclick={() => showRecentFiles = !showRecentFiles}>
            🕐 Recent
          </button>
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
        <button class="toolbar-btn" onclick={() => startIndexing()} disabled={isIndexing}>
          {isIndexing ? "⏳ Indexing..." : "⚡ Rebuild Index"}
        </button>
      </div>

      {#if showPinned && pinnedFiles.length > 0}
        <div class="pinned-panel" transition:slide>
          <h4>📌 Pinned Files</h4>
          <div class="pinned-list">
            {#each pinnedFiles as path}
              <div class="pinned-item">
                <span class="file-icon">{getFileIcon(path)}</span>
                <span class="pinned-path" onclick={() => showPreview(path)}>{path.split(/[\\/]/).pop()}</span>
                <button class="unpin-btn" onclick={() => unpinFile(path)} title="Unpin">×</button>
              </div>
            {/each}
          </div>
        </div>
      {/if}

      {#if showRecentFiles && recentFiles.length > 0}
        <div class="recent-files-panel" transition:slide>
          <h4>🕐 Recently Modified</h4>
          <div class="recent-files-list">
            {#each recentFiles as file}
              <div class="recent-file-item" onclick={() => showPreview(file.path)}>
                <span class="file-icon">{getFileIcon(file.path)}</span>
                <span class="recent-file-name">{file.title || file.path.split(/[\\/]/).pop()}</span>
                <span class="recent-file-info">{formatDate(file.modified)} • {formatBytes(file.size)}</span>
              </div>
            {/each}
          </div>
        </div>
      {/if}

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
              {#each results as result, i}
                <div
                  class="result-item"
                  class:active={selectedIndex === i}
                  data-result-index={i}
                  onclick={() => showPreview(result.file_path, i)}
                  ondblclick={() => openFile(result.file_path)}
                  role="button"
                  tabindex="0"
                >
                  <div class="col-name">
                    <span class="file-icon">{getFileIcon(result.file_path)}</span>
                    <span class="file-title">{result.title || result.file_path.split(/[\\/]/).pop()}</span>
                    {#if pinnedFiles.includes(result.file_path)}
                      <span class="pin-indicator" title="Pinned">📌</span>
                    {/if}
                  </div>
                  <div class="col-path">{result.file_path}</div>
                  <div class="col-actions">
                    <button 
                      class="action-btn" 
                      title={pinnedFiles.includes(result.file_path) ? "Unpin file" : "Pin file"}
                      onclick={(e) => { 
                        e.stopPropagation(); 
                        pinnedFiles.includes(result.file_path) ? unpinFile(result.file_path) : pinFile(result.file_path);
                      }}
                    >
                      {pinnedFiles.includes(result.file_path) ? "📌" : "📍"}
                    </button>
                    <button 
                      class="action-btn" 
                      title="Copy Path"
                      onclick={(e) => { e.stopPropagation(); copyToClipboard(result.file_path); }}
                    >
                      📋
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
                <span class="empty-hint">Type above to search through indexed files<br>or drag & drop a folder to index</span>
              </div>
            {/if}
          </div>
          <div class="results-footer">
            {#if results.length > 0}
              {results.length} results found
              {#if selectedIndex >= 0}
                • Selected: {selectedIndex + 1} of {results.length} (↑↓ to navigate, Enter to open, Space for quick look)
              {/if}
            {:else}
              No active search
            {/if}
          </div>
        </div>

        {#if selectedPath && settings.show_preview_panel}
          <div class="preview-panel" transition:fade>
            <div class="preview-header">
              <span class="preview-title">{selectedPath.split(/[\\/]/).pop()}</span>
              <div class="preview-actions">
                <button class="preview-btn" onclick={openQuickLook} title="Quick Look (Space)">👁️</button>
                <button class="preview-btn" onclick={() => { selectedPath = null; previewContent = null; selectedIndex = -1; }}>✕</button>
              </div>
            </div>
            <div class="preview-content">
              {#if highlightedPreview}
                <!-- eslint-disable-next-line svelte/no-at-html-tags -->
                <pre>{@html highlightedPreview}</pre>
              {:else}
                <div class="preview-loading">Loading...</div>
              {/if}
            </div>
          </div>
        {/if}
      </div>
    </div>
  {:else if activeTab === "stats"}
    <div class="stats-tab" in:fade={{ duration: 200 }}>
      <div class="stats-header">
        <h1>📊 Index Statistics</h1>
        <button class="btn-secondary" onclick={loadStatistics}>🔄 Refresh</button>
      </div>
      
      {#if indexStats}
        <div class="stats-grid">
          <div class="stat-card">
            <div class="stat-icon">📄</div>
            <div class="stat-value">{indexStats.total_documents.toLocaleString()}</div>
            <div class="stat-label">Files Indexed</div>
          </div>
          <div class="stat-card">
            <div class="stat-icon">💾</div>
            <div class="stat-value">{formatBytes(indexStats.total_size_bytes)}</div>
            <div class="stat-label">Total Size</div>
          </div>
          <div class="stat-card">
            <div class="stat-icon">📌</div>
            <div class="stat-value">{pinnedFiles.length}</div>
            <div class="stat-label">Pinned Files</div>
          </div>
        </div>
        
        <div class="stats-section">
          <h3>Quick Actions</h3>
          <div class="stats-actions">
            <button class="btn-primary" onclick={() => startIndexing()}>
              ⚡ Rebuild Index
            </button>
            <button class="btn-secondary" onclick={() => activeTab = "search"}>
              🔍 Start Searching
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
  {:else}
    <!-- Settings tab (keep existing) -->
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
                    <button class="tag-remove" onclick={() => {
                      settings.index_dirs = settings.index_dirs.filter(d => d !== dir);
                      hasChanges = true;
                    }}>✕</button>
                  </div>
                {/each}
                {#if settings.index_dirs.length === 0}
                  <span class="empty-hint">No directories added (Home folder indexed by default)</span>
                {/if}
              </div>
              <button class="btn-secondary" onclick={async () => {
                try {
                  const selected = await invoke<string | null>("select_folder");
                  if (selected && !settings.index_dirs.includes(selected)) {
                    settings.index_dirs = [...settings.index_dirs, selected];
                    hasChanges = true;
                  }
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
                    <button class="tag-remove" onclick={() => {
                      settings.exclude_patterns = settings.exclude_patterns.filter(p => p !== pattern);
                      hasChanges = true;
                    }}>✕</button>
                  </div>
                {/each}
              </div>
              <div class="input-with-button">
                <input 
                  type="text" 
                  placeholder="Add pattern (e.g., *.tmp, backup/)"
                  onkeydown={(e) => {
                    if (e.key === 'Enter') {
                      const value = (e.target as HTMLInputElement).value;
                      if (value && !settings.exclude_patterns.includes(value)) {
                        settings.exclude_patterns = [...settings.exclude_patterns, value];
                        hasChanges = true;
                        (e.target as HTMLInputElement).value = '';
                      }
                    }
                  }}
                />
                <button class="btn-secondary" onclick={(e) => {
                  const input = (e.currentTarget.previousElementSibling as HTMLInputElement);
                  if (input.value && !settings.exclude_patterns.includes(input.value)) {
                    settings.exclude_patterns = [...settings.exclude_patterns, input.value];
                    hasChanges = true;
                    input.value = '';
                  }
                }}>Add</button>
              </div>
            </div>
          </div>
        {/if}
      </div>

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
                onchange={(e) => updateSetting('max_results', parseInt((e.target as HTMLInputElement).value))}
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
                  onchange={(e) => updateSetting('search_history_enabled', (e.target as HTMLInputElement).checked)}
                />
                <span class="toggle-slider"></span>
              </label>
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

  <!-- Quick Look Modal -->
  {#if isQuickLookOpen && selectedPath}
    <div class="quicklook-overlay" onclick={closeQuickLook} transition:fade>
      <div class="quicklook-modal" onclick={(e) => e.stopPropagation()}>
        <div class="quicklook-header">
          <span class="quicklook-title">{selectedPath.split(/[\\/]/).pop()}</span>
          <button class="close-btn" onclick={closeQuickLook}>✕</button>
        </div>
        <div class="quicklook-content">
          <!-- eslint-disable-next-line svelte/no-at-html-tags -->
          <pre>{@html highlightedPreview}</pre>
        </div>
      </div>
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

  /* Search Highlighting */
  :global(.search-highlight) {
    background: #ffeb3b;
    color: #000;
    padding: 1px 2px;
    border-radius: 2px;
    font-weight: 600;
  }

  :global(body[data-theme="dark"] .search-highlight) {
    background: #ffc107;
    color: #000;
  }

  .container {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }

  .container.dragging {
    opacity: 0.8;
  }

  .drop-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 120, 212, 0.9);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .drop-message {
    text-align: center;
    color: white;
  }

  .drop-icon {
    font-size: 64px;
    display: block;
    margin-bottom: 16px;
  }

  .drop-message p {
    font-size: 24px;
    font-weight: 600;
    margin: 0;
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

  .operator-help-btn {
    background: transparent;
    border: none;
    color: #999;
    font-size: 16px;
    cursor: pointer;
    padding: 4px 8px;
    border-radius: 4px;
    margin-left: 8px;
  }

  .operator-help-btn:hover {
    background: rgba(0, 0, 0, 0.1);
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

  /* Operator Help */
  .operator-help {
    background: #fff;
    border-bottom: 1px solid #e0e0e0;
    padding: 16px 32px;
  }

  :global(body[data-theme="dark"]) .operator-help {
    background: #252525;
    border-color: #3a3a3a;
  }

  .operator-help h4 {
    margin: 0 0 12px;
    font-size: 14px;
    color: #666;
  }

  .operator-list {
    display: flex;
    gap: 16px;
    flex-wrap: wrap;
  }

  .operator-item {
    font-size: 13px;
  }

  .operator-item code {
    background: #f0f0f0;
    padding: 2px 6px;
    border-radius: 3px;
    font-family: "Consolas", monospace;
    color: #0078d4;
  }

  :global(body[data-theme="dark"]) .operator-item code {
    background: #3a3a3a;
    color: #4fc3f7;
  }

  /* Search Chips */
  .search-chips {
    display: flex;
    gap: 8px;
    padding: 8px 32px;
    background: #fafafa;
    border-bottom: 1px solid #e0e0e0;
    flex-wrap: wrap;
  }

  :global(body[data-theme="dark"]) .search-chips {
    background: #202020;
    border-color: #3a3a3a;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 4px 8px 4px 12px;
    border-radius: 12px;
    font-size: 12px;
    font-weight: 500;
  }

  .chip.ext { background: #e3f2fd; color: #1565c0; }
  .chip.path { background: #f3e5f5; color: #6a1b9a; }
  .chip.title { background: #e8f5e9; color: #2e7d32; }
  .chip.size { background: #fff3e0; color: #ef6c00; }

  :global(body[data-theme="dark"]) .chip.ext { background: #1565c0; color: #fff; }
  :global(body[data-theme="dark"]) .chip.path { background: #6a1b9a; color: #fff; }
  :global(body[data-theme="dark"]) .chip.title { background: #2e7d32; color: #fff; }
  :global(body[data-theme="dark"]) .chip.size { background: #ef6c00; color: #fff; }

  .chip button {
    background: none;
    border: none;
    cursor: pointer;
    font-size: 14px;
    padding: 0 4px;
    opacity: 0.7;
  }

  .chip button:hover {
    opacity: 1;
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

  .quick-access {
    display: flex;
    gap: 8px;
  }

  /* Pinned Panel */
  .pinned-panel, .recent-files-panel {
    background: #fafafa;
    border-bottom: 1px solid #e0e0e0;
    padding: 16px 32px;
  }

  :global(body[data-theme="dark"]) .pinned-panel,
  :global(body[data-theme="dark"]) .recent-files-panel {
    background: #202020;
    border-color: #3a3a3a;
  }

  .pinned-panel h4, .recent-files-panel h4 {
    margin: 0 0 12px;
    font-size: 14px;
    color: #666;
  }

  .pinned-list, .recent-files-list {
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
  }

  .pinned-item, .recent-file-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    background: #fff;
    border: 1px solid #ddd;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.2s;
  }

  :global(body[data-theme="dark"]) .pinned-item,
  :global(body[data-theme="dark"]) .recent-file-item {
    background: #2a2a2a;
    border-color: #444;
  }

  .pinned-item:hover, .recent-file-item:hover {
    background: #f0f0f0;
  }

  .pinned-path {
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .unpin-btn {
    background: none;
    border: none;
    cursor: pointer;
    color: #999;
    font-size: 16px;
    padding: 0 4px;
  }

  .recent-file-info {
    font-size: 11px;
    color: #999;
    margin-left: 8px;
  }

  /* Recent Searches Dropdown */
  .recent-searches-dropdown {
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    background: #fff;
    border: 1px solid #e0e0e0;
    border-radius: 8px;
    margin-top: 8px;
    box-shadow: 0 4px 12px rgba(0,0,0,0.1);
    z-index: 100;
    max-height: 300px;
    overflow-y: auto;
  }

  .recent-searches-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 16px;
    border-bottom: 1px solid #f0f0f0;
    font-size: 12px;
    color: #666;
    font-weight: 600;
  }

  .clear-recent {
    background: none;
    border: none;
    color: #0078d4;
    cursor: pointer;
    font-size: 12px;
  }

  .recent-search-item {
    display: block;
    width: 100%;
    padding: 10px 16px;
    border: none;
    background: none;
    text-align: left;
    cursor: pointer;
    font-size: 14px;
    color: #333;
  }

  .recent-search-item:hover {
    background: #f5f5f5;
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
    font-size: 16px;
    min-width: 24px;
    text-align: center;
  }

  .file-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .pin-indicator {
    font-size: 11px;
    margin-left: 4px;
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
    width: 100px;
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

  .preview-actions {
    display: flex;
    gap: 8px;
  }

  .preview-btn {
    background: transparent;
    border: none;
    cursor: pointer;
    padding: 4px 8px;
    border-radius: 4px;
  }

  .preview-btn:hover {
    background: rgba(0, 0, 0, 0.1);
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

  /* Quick Look Modal */
  .quicklook-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.8);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .quicklook-modal {
    background: #fff;
    border-radius: 12px;
    width: 80%;
    height: 80%;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
  }

  :global(body[data-theme="dark"]) .quicklook-modal {
    background: #252525;
  }

  .quicklook-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 24px;
    border-bottom: 1px solid #e0e0e0;
  }

  :global(body[data-theme="dark"]) .quicklook-header {
    border-color: #3a3a3a;
  }

  .quicklook-title {
    font-weight: 600;
    font-size: 16px;
  }

  .quicklook-content {
    flex: 1;
    overflow: auto;
    padding: 24px;
  }

  .quicklook-content pre {
    margin: 0;
    font-family: "Consolas", "Monaco", monospace;
    font-size: 14px;
    line-height: 1.8;
    white-space: pre-wrap;
    word-wrap: break-word;
  }

  /* Stats Tab */
  .stats-tab {
    flex: 1;
    overflow-y: auto;
    padding: 32px;
  }

  .stats-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 32px;
  }

  .stats-header h1 {
    margin: 0;
    font-size: 28px;
  }

  .stats-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 24px;
    margin-bottom: 32px;
  }

  .stat-card {
    background: #fff;
    border: 1px solid #e0e0e0;
    border-radius: 12px;
    padding: 24px;
    text-align: center;
    transition: transform 0.2s;
  }

  :global(body[data-theme="dark"]) .stat-card {
    background: #252525;
    border-color: #3a3a3a;
  }

  .stat-card:hover {
    transform: translateY(-2px);
  }

  .stat-icon {
    font-size: 40px;
    margin-bottom: 12px;
  }

  .stat-value {
    font-size: 32px;
    font-weight: 700;
    color: #0078d4;
    margin-bottom: 4px;
  }

  :global(body[data-theme="dark"]) .stat-value {
    color: #4fc3f7;
  }

  .stat-label {
    font-size: 14px;
    color: #888;
  }

  .stats-section {
    background: #fff;
    border: 1px solid #e0e0e0;
    border-radius: 12px;
    padding: 24px;
  }

  :global(body[data-theme="dark"]) .stats-section {
    background: #252525;
    border-color: #3a3a3a;
  }

  .stats-section h3 {
    margin: 0 0 16px;
    font-size: 18px;
  }

  .stats-actions {
    display: flex;
    gap: 12px;
  }

  .stats-loading {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 80px;
    color: #888;
  }

  .spinner-large {
    width: 48px;
    height: 48px;
    border: 4px solid #ddd;
    border-top-color: #0078d4;
    border-radius: 50%;
    animation: spin 1s linear infinite;
    margin-bottom: 16px;
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
    padding: 6px 12px;
    background: #e3f2fd;
    color: #1565c0;
    border-radius: 4px;
    font-size: 13px;
  }

  .tag.secondary {
    background: #f5f5f5;
    color: #666;
  }

  :global(body[data-theme="dark"]) .tag {
    background: #1e3a5f;
    color: #4fc3f7;
  }

  .tag-remove {
    background: none;
    border: none;
    cursor: pointer;
    color: inherit;
    padding: 0 2px;
    font-size: 14px;
    opacity: 0.6;
  }

  .tag-remove:hover {
    opacity: 1;
  }

  /* Progress Toast */
  .progress-toast {
    position: fixed;
    bottom: 24px;
    right: 24px;
    background: #fff;
    border: 1px solid #e0e0e0;
    border-radius: 12px;
    padding: 16px 20px;
    box-shadow: 0 4px 20px rgba(0,0,0,0.15);
    min-width: 300px;
    z-index: 1000;
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
    margin-bottom: 8px;
  }

  .progress-bar {
    height: 6px;
    background: #e0e0e0;
    border-radius: 3px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: #0078d4;
    transition: width 0.3s ease;
  }

  .current-file {
    margin-top: 8px;
    font-size: 11px;
    color: #888;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Toast */
  .toast {
    position: fixed;
    bottom: 24px;
    right: 24px;
    padding: 12px 24px;
    border-radius: 8px;
    font-weight: 500;
    z-index: 1000;
  }

  .toast.success {
    background: #4caf50;
    color: white;
  }
</style>

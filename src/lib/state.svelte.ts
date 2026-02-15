import { api } from './api';
import type { 
  SearchResult, 
  RecentFile, 
  IndexStats, 
  AppSettings,
  IndexProgress,
  PreviewResult 
} from './types';
import { defaultSettings } from './types';

// Global app state using Svelte 5 runes
class AppState {
  // Search state
  query = $state("");
  results = $state<SearchResult[]>([]);
  isSearching = $state(false);
  searchMode = $state<"content" | "filename">("content");
  filenameIndexReady = $state(false);
  searchChips = $state<{type: string, value: string, label: string}[]>([]);
  selectedIndex = $state(-1);
  searchVersion = $state(0);
  
  // Filters
  minSize = $state<number | null>(null);
  maxSize = $state<number | null>(null);
  selectedFileType = $state("all");
  showFilters = $state(false);
  
  // Sorting
  sortBy = $state<"relevance" | "name" | "date" | "size">("relevance");
  sortOrder = $state<"asc" | "desc">("desc");
  showSortDropdown = $state(false);
  showExportDropdown = $state(false);
  
  // Recent searches
  showRecentSearches = $state(false);
  recentSearches = $state<string[]>([]);
  
  // Pinned and recent files
  pinnedFiles = $state<string[]>([]);
  recentFiles = $state<RecentFile[]>([]);
  showPinned = $state(false);
  showRecentFiles = $state(false);
  
  // Statistics
  indexStats = $state<IndexStats | null>(null);
  
  // Settings
  settings = $state<AppSettings>({...defaultSettings});
  hasChanges = $state(false);
  showSaveSuccess = $state(false);
  
  // UI state
  activeTab = $state<"search" | "settings" | "stats">("search");
  showShortcuts = $state(false);
  sidebarCollapsed = $state(false);
  activeFolder = $state<string | null>(null);
  expandedSections = $state({
    indexing: true,
    search: false,
    appearance: false,
    behavior: false,
    performance: false,
    advanced: false
  });
  
  // Preview
  selectedPath = $state<string | null>(null);
  previewContent = $state<string | null>(null);
  highlightedPreview = $state("");
  isQuickLookOpen = $state(false);
  
  // Indexing progress
  indexProgress = $state<IndexProgress>({
    total: 0,
    processed: 0,
    currentFile: "",
    status: "idle",
    files_per_second: 0,
    eta_seconds: 0,
    current_folder: ""
  });
  
  // Drag and drop
  isDragging = $state(false);
  isIndexing = $state(false);
  
  // Debounce
  private debounceTimer: ReturnType<typeof setTimeout> | null = null;
  
  // Computed values
  get progressPercentage() {
    return this.indexProgress.total > 0 
      ? Math.round((this.indexProgress.processed / this.indexProgress.total) * 100) 
      : 0;
  }
  
  // Initialize event listeners
  async init() {
    await this.loadSettings();
    await this.loadRecentSearches();
    await this.loadPinnedFiles();
    await this.loadRecentFiles();
    await this.loadStatistics();
    await this.checkFilenameIndex();
  }
  
  // Load settings from backend
  async loadSettings() {
    try {
      const loaded = await api.getSettings() as Promise<AppSettings>;
      this.settings = { ...defaultSettings, ...loaded };
      this.hasChanges = false;
    } catch (e) {
      console.error("Failed to load settings:", e);
    }
  }
  
  // Save settings to backend
  async saveSettings() {
    try {
      await api.saveSettings(this.settings);
      this.hasChanges = false;
      this.showSaveSuccess = true;
      setTimeout(() => this.showSaveSuccess = false, 2000);
    } catch (e) {
      console.error("Failed to save settings:", e);
    }
  }
  
  // Update a setting
  updateSetting<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
    (this.settings as any)[key] = value;
    this.hasChanges = true;
  }
  
  // Reset to defaults
  resetToDefaults() {
    this.settings = { ...defaultSettings };
    this.hasChanges = true;
  }
  
  // Load recent searches
  async loadRecentSearches() {
    try {
      this.recentSearches = await api.getRecentSearches() as Promise<string[]>;
    } catch (e) {
      console.error("Failed to load recent searches:", e);
    }
  }
  
  // Load pinned files
  async loadPinnedFiles() {
    try {
      this.pinnedFiles = await api.getPinnedFiles() as Promise<string[]>;
    } catch (e) {
      console.error("Failed to load pinned files:", e);
    }
  }
  
  // Load recent files
  async loadRecentFiles() {
    try {
      this.recentFiles = await api.getRecentFiles(10) as Promise<RecentFile[]>;
    } catch (e) {
      console.error("Failed to load recent files:", e);
    }
  }
  
  // Load statistics
  async loadStatistics() {
    try {
      this.indexStats = await api.getStatistics() as Promise<IndexStats>;
    } catch (e) {
      console.error("Failed to load statistics:", e);
    }
  }
  
  // Check if filename index exists
  async checkFilenameIndex() {
    try {
      await api.getFilenameIndexStats() as Promise<{total_files: number, index_size_bytes: number}>;
      this.filenameIndexReady = true;
    } catch (e) {
      console.log("Filename index not available:", e);
      this.filenameIndexReady = false;
    }
  }
  
  // Search functions
  debouncedSearch() {
    if (this.debounceTimer) clearTimeout(this.debounceTimer);
    this.debounceTimer = setTimeout(() => this.performSearch(), 300);
  }
  
  async performSearch() {
    const currentVersion = ++this.searchVersion;
    
    if (!this.query.trim() && this.searchChips.length === 0) {
      if (this.searchVersion === currentVersion) {
        this.results = [];
      }
      return;
    }

    this.isSearching = true;
    this.selectedIndex = -1;
    
    try {
      if (this.searchMode === "filename") {
        const filenameResults = await api.searchFilenames(this.query.trim(), this.settings.max_results) as Promise<{file_path: string, file_name: string}[]>;
        
        if (this.searchVersion !== currentVersion) return;
        
        this.results = filenameResults.map(r => ({
          file_path: r.file_path,
          title: r.file_name,
          score: 1.0,
          matched_terms: []
        }));
        
        if (this.settings.search_history_enabled && this.query.trim()) {
          await api.addRecentSearch(this.query.trim());
          this.loadRecentSearches();
        }
        
        this.isSearching = false;
        return;
      }
      
      // Content search
      let fileExtensions: string[] | null = null;
      if (this.selectedFileType !== "all") {
        const fileTypeMap: Record<string, string[]> = {
          documents: ["docx", "pdf", "odt", "txt", "rtf"],
          code: ["rs", "js", "ts", "jsx", "tsx", "py", "java", "cpp", "c", "h", "go", "rb", "php", "swift", "kt"],
          text: ["txt", "md", "json", "xml", "yaml", "yml", "csv"]
        };
        fileExtensions = fileTypeMap[this.selectedFileType] || null;
      }

      const searchResults = await api.search(
        this.query.trim() || "*",
        this.settings.max_results,
        this.minSize ? this.minSize * 1024 * 1024 : undefined,
        this.maxSize ? this.maxSize * 1024 * 1024 : undefined,
        fileExtensions || undefined
      ) as Promise<SearchResult[]>;

      if (this.searchVersion !== currentVersion) return;
      
      this.results = searchResults;

      if (this.settings.search_history_enabled && this.query.trim()) {
        await api.addRecentSearch(this.query.trim());
        this.loadRecentSearches();
      }
    } catch (e) {
      console.error("Search failed:", e);
      if (currentVersion === this.searchVersion) {
        this.results = [];
      }
    } finally {
      if (currentVersion === this.searchVersion) {
        this.isSearching = false;
      }
    }
  }
  
  // Show preview
  async showPreview(path: string, index?: number) {
    this.selectedPath = path;
    if (index !== undefined) this.selectedIndex = index;
    
    try {
      const result = await api.getPreviewHighlighted(path, this.query.trim() || "*") as Promise<PreviewResult>;
      this.previewContent = result.content;
      this.highlightedPreview = this.highlightText(result.content, result.matched_terms);
    } catch (e) {
      this.previewContent = "Failed to load preview";
      this.highlightedPreview = "Failed to load preview";
    }
  }
  
  // Highlight text
  highlightText(text: string, terms: string[]): string {
    if (!terms.length) return this.escapeHtml(text);
    
    let highlighted = this.escapeHtml(text);
    terms.forEach(term => {
      const regex = new RegExp(`(${this.escapeRegex(term)})`, "gi");
      highlighted = highlighted.replace(regex, '<mark class="search-highlight">$1</mark>');
    });
    
    return highlighted;
  }
  
  escapeHtml(text: string): string {
    const div = document.createElement("div");
    div.textContent = text;
    return div.innerHTML;
  }
  
  escapeRegex(string: string): string {
    return string.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  }
  
  // Pin/unpin files
  async pinFile(path: string) {
    try {
      await api.pinFile(path);
      this.pinnedFiles = [...this.pinnedFiles, path];
    } catch (e) {
      console.error("Failed to pin file:", e);
    }
  }
  
  async unpinFile(path: string) {
    try {
      await api.unpinFile(path);
      this.pinnedFiles = this.pinnedFiles.filter(p => p !== path);
    } catch (e) {
      console.error("Failed to unpin file:", e);
    }
  }
  
  // Open file
  async openFile(path: string) {
    try {
      window.open(`file://${path}`, '_blank');
    } catch (e) {
      console.error("Failed to open file:", e);
    }
  }
  
  // Copy to clipboard
  async copyToClipboard(text: string) {
    try {
      await navigator.clipboard.writeText(text);
    } catch (e) {
      console.error("Failed to copy to clipboard:", e);
    }
  }
  
  // Start indexing
  async startIndexing(path?: string) {
    this.isIndexing = true;
    try {
      const homeDir = path || ".";
      await api.startIndexing(homeDir);
    } catch (e) {
      console.error("Indexing failed:", e);
    } finally {
      this.isIndexing = false;
    }
  }
  
  // Build filename index
  async buildFilenameIndex() {
    try {
      const homeDir = ".";
      await api.startIndexing(homeDir);
      this.filenameIndexReady = true;
      this.searchMode = "filename";
    } catch (e) {
      console.error("Failed to build filename index:", e);
    }
  }
  
  // Export results
  async exportResults(format: 'csv' | 'json' | 'txt') {
    try {
      api.exportResults(this.results, format);
    } catch (e) {
      console.error("Failed to export results:", e);
    }
  }
  
  // Toggle section
  toggleSection(section: keyof typeof this.expandedSections) {
    (this.expandedSections as any)[section] = !(this.expandedSections as any)[section];
  }
}

// Create singleton instance
export const appState = new AppState();

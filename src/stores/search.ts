import { writable, derived, get } from 'svelte/store';
import { invoke } from "@tauri-apps/api/core";

// Types
export interface SearchResult {
  file_path: string;
  title: string | null;
  score: number;
}

export interface AppSettings {
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
}

// Default settings
export const defaultSettings: AppSettings = {
  index_dirs: [],
  exclude_patterns: [".git/", "node_modules/", "target/", "AppData/", "*.tmp", "*.temp", "Thumbs.db", ".DS_Store"],
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
};

// Search state store
function createSearchStore() {
  const { subscribe, set, update } = writable({
    query: '',
    results: [] as SearchResult[],
    isSearching: false,
    isIndexing: false,
    selectedPath: null as string | null,
    previewContent: null as string | null,
    showFilters: false,
    showRecentSearches: false,
    recentSearches: [] as string[],
  });

  let debounceTimer: ReturnType<typeof setTimeout> | null = null;
  let abortController: AbortController | null = null;

  return {
    subscribe,
    setQuery: (query: string) => {
      update(state => ({ ...state, query }));
      
      // Cancel previous search if running
      if (abortController) {
        abortController.abort();
      }
      abortController = new AbortController();
    },
    
    debouncedSearch: async (
      query: string, 
      settings: AppSettings,
      filters?: {
        minSize: number | null;
        maxSize: number | null;
        fileType: string;
      }
    ) => {
      if (debounceTimer) {
        clearTimeout(debounceTimer);
      }

      if (!query.trim()) {
        update(state => ({ ...state, results: [], isSearching: false }));
        return;
      }

      debounceTimer = setTimeout(async () => {
        update(state => ({ ...state, isSearching: true }));
        
        try {
          // Build file extensions filter
          let fileExtensions: string[] | null = null;
          if (filters?.fileType && filters.fileType !== "all") {
            const fileTypeMap: Record<string, string[]> = {
              documents: ["docx", "pdf", "odt", "txt", "rtf"],
              code: ["rs", "js", "ts", "jsx", "tsx", "py", "java", "cpp", "c", "h", "go", "rb", "php", "swift", "kt"],
              text: ["txt", "md", "json", "xml", "yaml", "yml", "csv"]
            };
            fileExtensions = fileTypeMap[filters.fileType] || null;
          }

          const results = await invoke<SearchResult[]>("search_query", { 
            query: query.trim(), 
            limit: settings.max_results,
            min_size: filters?.minSize ? filters.minSize * 1024 * 1024 : null,
            max_size: filters?.maxSize ? filters.maxSize * 1024 * 1024 : null,
            file_extensions: fileExtensions
          });

          update(state => ({ ...state, results, isSearching: false }));

          // Add to recent searches
          if (settings.search_history_enabled) {
            await invoke("add_recent_search", { query: query.trim() });
            loadRecentSearches();
          }
        } catch (e) {
          console.error("Search failed:", e);
          update(state => ({ ...state, results: [], isSearching: false }));
        }
      }, 300);
    },

    selectResult: async (path: string) => {
      update(state => ({ 
        ...state, 
        selectedPath: path,
        previewContent: null 
      }));
      
      try {
        const content = await invoke<string>("get_file_preview", { path });
        update(state => ({ ...state, previewContent: content }));
      } catch (e) {
        update(state => ({ ...state, previewContent: "Failed to load preview" }));
      }
    },

    clearSelection: () => {
      update(state => ({ 
        ...state, 
        selectedPath: null, 
        previewContent: null 
      }));
    },

    setIndexing: (isIndexing: boolean) => {
      update(state => ({ ...state, isIndexing }));
    },

    toggleFilters: () => {
      update(state => ({ ...state, showFilters: !state.showFilters }));
    },

    setShowRecentSearches: (show: boolean) => {
      update(state => ({ ...state, showRecentSearches: show }));
    },

    loadRecentSearches: async () => {
      try {
        const searches = await invoke<string[]>("get_recent_searches");
        update(state => ({ ...state, recentSearches: searches }));
      } catch (e) {
        console.error("Failed to load recent searches:", e);
      }
    },

    clearRecentSearches: async () => {
      try {
        await invoke("clear_recent_searches");
        update(state => ({ ...state, recentSearches: [] }));
      } catch (e) {
        console.error("Failed to clear recent searches:", e);
      }
    },

    reset: () => {
      if (debounceTimer) {
        clearTimeout(debounceTimer);
      }
      if (abortController) {
        abortController.abort();
      }
      set({
        query: '',
        results: [],
        isSearching: false,
        isIndexing: false,
        selectedPath: null,
        previewContent: null,
        showFilters: false,
        showRecentSearches: false,
        recentSearches: [],
      });
    }
  };
}

// Settings store with change tracking
function createSettingsStore() {
  const { subscribe, set, update } = writable<AppSettings>(defaultSettings);
  const { subscribe: subscribeChanges, set: setChanges } = writable(false);

  return {
    subscribe,
    subscribeChanges,
    
    load: async () => {
      try {
        const loaded = await invoke<AppSettings>("get_settings");
        set({ ...defaultSettings, ...loaded });
        setChanges(false);
      } catch (e) {
        console.error("Failed to load settings:", e);
        set(defaultSettings);
      }
    },

    save: async (settings: AppSettings) => {
      try {
        await invoke("save_settings", { settings });
        setChanges(false);
        return true;
      } catch (e) {
        console.error("Failed to save settings:", e);
        return false;
      }
    },

    updateSetting: <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => {
      update(settings => {
        const newSettings = { ...settings, [key]: value };
        setChanges(true);
        return newSettings;
      });
    },

    updateNestedSetting: (
      parent: keyof AppSettings,
      key: string,
      value: any
    ) => {
      update(settings => {
        const newSettings = { 
          ...settings, 
          [parent]: { 
            ...(settings[parent] as object), 
            [key]: value 
          } 
        };
        setChanges(true);
        return newSettings;
      });
    },

    addToArray: <K extends keyof AppSettings>(
      key: K, 
      value: string,
      options?: { unique?: boolean }
    ) => {
      update(settings => {
        const current = (settings[key] as unknown as string[]);
        if (options?.unique && current.includes(value)) {
          return settings;
        }
        const newSettings = { 
          ...settings, 
          [key]: [...current, value] 
        };
        setChanges(true);
        return newSettings;
      });
    },

    removeFromArray: <K extends keyof AppSettings>(
      key: K,
      value: string
    ) => {
      update(settings => {
        const current = (settings[key] as unknown as string[]);
        const newSettings = { 
          ...settings, 
          [key]: current.filter(item => item !== value) 
        };
        setChanges(true);
        return newSettings;
      });
    },

    toggleArrayItem: <K extends keyof AppSettings>(
      key: K,
      value: string
    ) => {
      update(settings => {
        const current = (settings[key] as unknown as string[]);
        const exists = current.includes(value);
        const newSettings = {
          ...settings,
          [key]: exists 
            ? current.filter(item => item !== value)
            : [...current, value]
        };
        setChanges(true);
        return newSettings;
      });
    },

    reset: () => {
      set(defaultSettings);
      setChanges(true);
    },

    markSaved: () => {
      setChanges(false);
    }
  };
}

// Indexing progress store
function createIndexingProgressStore() {
  const { subscribe, set, update } = writable({
    total: 0,
    processed: 0,
    currentFile: "",
    status: "idle" as "idle" | "scanning" | "indexing" | "done"
  });

  return {
    subscribe,
    
    updateProgress: (data: {
      total: number;
      processed: number;
      current_file: string;
      status: string;
    }) => {
      update(state => ({
        total: data.total,
        processed: data.processed,
        currentFile: data.current_file,
        status: data.status as typeof state.status
      }));

      // Auto-reset after completion
      if (data.status === "done") {
        setTimeout(() => {
          update(state => ({ ...state, status: "idle" }));
        }, 5000);
      }
    },

    reset: () => {
      set({
        total: 0,
        processed: 0,
        currentFile: "",
        status: "idle"
      });
    },

    getProgressPercentage: () => {
      const state = get({ subscribe });
      return state.total > 0 
        ? Math.round((state.processed / state.total) * 100) 
        : 0;
    }
  };
}

// Export stores
export const searchStore = createSearchStore();
export const settingsStore = createSettingsStore();
export const indexingProgressStore = createIndexingProgressStore();

// Derived stores for computed values
export const effectiveTheme = derived(
  settingsStore,
  $settings => {
    if ($settings.theme === "auto") {
      return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
    }
    return $settings.theme;
  }
);

export const fontSizeClass = derived(
  settingsStore,
  $settings => {
    const classes: Record<string, string> = {
      small: "font-small",
      medium: "font-medium",
      large: "font-large"
    };
    return classes[$settings.font_size] || "font-medium";
  }
);

// Helper function
async function loadRecentSearches() {
  searchStore.loadRecentSearches();
}

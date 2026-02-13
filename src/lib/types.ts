// Types shared across all components
export interface SearchResult {
  file_path: string;
  title: string | null;
  score: number;
  matched_terms: string[];
}

export interface RecentFile {
  path: string;
  title: string | null;
  modified: number;
  size: number;
}

export interface PreviewResult {
  content: string;
  matched_terms: string[];
}

export interface IndexStats {
  total_documents: number;
  total_size_bytes: number;
  last_updated: string | null;
}

export interface AppSettings {
  index_dirs: string[];
  exclude_patterns: string[];
  exclude_folders: string[];
  auto_index_on_startup: boolean;
  index_file_size_limit_mb: number;
  max_results: number;
  search_history_enabled: boolean;
  fuzzy_matching: boolean;
  case_sensitive: boolean;
  default_filters: DefaultFilters;
  theme: Theme;
  font_size: FontSize;
  show_file_extensions: boolean;
  results_per_page: number;
  minimize_to_tray: boolean;
  auto_start_on_boot: boolean;
  double_click_action: DoubleClickAction;
  show_preview_panel: boolean;
  indexing_threads: number;
  memory_limit_mb: number;
  pinned_files: string[];
}

export interface DefaultFilters {
  file_types: string[];
  min_size_mb: number | null;
  max_size_mb: number | null;
  modified_within_days: number | null;
}

export type Theme = "auto" | "light" | "dark";
export type FontSize = "small" | "medium" | "large";
export type DoubleClickAction = "open_file" | "show_in_folder" | "preview";

export interface IndexProgress {
  total: number;
  processed: number;
  currentFile: string;
  status: "idle" | "scanning" | "indexing" | "done";
  files_per_second: number;
  eta_seconds: number;
  current_folder: string;
}

export interface SearchChip {
  type: string;
  value: string;
  label: string;
}

// Default settings
export const defaultSettings: AppSettings = {
  index_dirs: [],
  exclude_patterns: [".git/", "node_modules/", "target/", "AppData/", "*.tmp", "*.temp", "Thumbs.db", ".DS_Store"],
  exclude_folders: ["$RECYCLE.BIN", "System Volume Information"],
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
};

// File type icons
export const fileTypeIcons: Record<string, string> = {
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

export function getFileIcon(path: string): string {
  const ext = path.split(".").pop()?.toLowerCase() || "";
  return fileTypeIcons[ext] || fileTypeIcons.default;
}

export function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
}

export function formatDate(timestamp: number): string {
  return new Date(timestamp * 1000).toLocaleDateString();
}

export function formatEta(seconds: number): string {
  if (!seconds || seconds <= 0) return "calculating...";
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
  const hours = Math.floor(seconds / 3600);
  const mins = Math.floor((seconds % 3600) / 60);
  return `${hours}h ${mins}m`;
}

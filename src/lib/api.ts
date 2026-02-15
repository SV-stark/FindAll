const API_BASE = "/api";

async function fetchApi<T>(endpoint: string, body?: unknown): Promise<T> {
  const response = await fetch(`${API_BASE}${endpoint}`, {
    method: body ? "POST" : "GET",
    headers: { "Content-Type": "application/json" },
    body: body ? JSON.stringify(body) : undefined,
  });
  if (!response.ok) {
    const error = await response.text();
    throw new Error(error);
  }
  return response.json();
}

export const api = {
  search: (query: string, limit = 20, minSize?: number, maxSize?: number, fileExtensions?: string[]) =>
    fetchApi("/search", { query, limit, min_size: minSize, max_size: maxSize, file_extensions: fileExtensions }),

  searchFilenames: (query: string, limit = 20) =>
    fetchApi("/search/filenames", { query, limit }),

  getPreview: (path: string) =>
    fetchApi("/preview", { path }),

  getPreviewHighlighted: (path: string, query: string) =>
    fetchApi("/preview/highlighted", { path, query }),

  startIndexing: (path: string) =>
    fetchApi("/index/start", { path }),

  getStatistics: () =>
    fetchApi("/statistics"),

  getRecentFiles: (limit = 10) =>
    fetchApi(`/recent-files?limit=${limit}`),

  getSettings: () =>
    fetchApi("/settings"),

  saveSettings: (settings: unknown) =>
    fetchApi("/settings", settings),

  getPinnedFiles: () =>
    fetchApi("/pinned"),

  pinFile: (path: string) =>
    fetchApi(`/pinned/${encodeURIComponent(path)}`, {}),

  unpinFile: (path: string) =>
    fetch(`/pinned/${encodeURIComponent(path)}`, { method: "DELETE" }),

  getRecentSearches: () =>
    fetchApi("/recent-searches"),

  addRecentSearch: (query: string) =>
    fetchApi("/recent-searches", { query }),

  getFilenameIndexStats: () =>
    fetchApi("/stats/filenames"),

  clearRecentSearches: () =>
    fetchApi("/recent-searches/clear", {}),

  exportResults: (results: unknown[], format: string) => {
    let content: string;
    let mimeType: string;
    let filename: string;

    if (format === "json") {
      content = JSON.stringify(results, null, 2);
      mimeType = "application/json";
      filename = "search-results.json";
    } else if (format === "csv") {
      const headers = "file_path,title,score\n";
      const rows = (results as {file_path: string, title?: string, score: number}[])
        .map(r => `"${r.file_path}","${r.title || ""}",${r.score}`)
        .join("\n");
      content = headers + rows;
      mimeType = "text/csv";
      filename = "search-results.csv";
    } else {
      content = (results as {file_path: string}[]).map(r => r.file_path).join("\n");
      mimeType = "text/plain";
      filename = "search-results.txt";
    }

    const blob = new Blob([content], { type: mimeType });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = filename;
    a.click();
    URL.revokeObjectURL(url);
  },
};

<script lang="ts">
  import { fade, slide } from "svelte/transition";
  import { appState } from '$lib/state.svelte';
  import Icon from './Icon.svelte';
  import { getFileColor, getFileIcon } from '$lib/utils/fileTypes';

  let expandedFolders = $state<Set<string>>(new Set());

  function toggleFolder(path: string) {
    if (expandedFolders.has(path)) {
      expandedFolders.delete(path);
    } else {
      expandedFolders.add(path);
    }
    expandedFolders = new Set(expandedFolders);
  }

  async function addIndexFolder() {
    try {
      const selected = prompt("Enter folder path to index:");
      if (selected && !appState.settings.index_dirs.includes(selected)) {
        appState.settings.index_dirs = [...appState.settings.index_dirs, selected];
        appState.hasChanges = true;
      }
    } catch (e) {
      console.error("Failed to pick folder:", e);
    }
  }
</script>

<aside class="sidebar" class:collapsed={appState.sidebarCollapsed}>
  <div class="sidebar-header">
    {#if !appState.sidebarCollapsed}
      <span class="logo">
        <Icon icon="magnifying-glass" size={20} color="var(--primary)" />
        <span>Flash Search</span>
      </span>
    {/if}
    <button 
      class="collapse-btn" 
      onclick={() => appState.sidebarCollapsed = !appState.sidebarCollapsed}
      aria-label={appState.sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
    >
      <Icon icon={appState.sidebarCollapsed ? "chevron-right" : "chevron-left"} size={16} />
    </button>
  </div>

  {#if !appState.sidebarCollapsed}
    <div class="sidebar-content" transition:slide={{ duration: 200 }}>
      
      <!-- Quick Search -->
      <div class="sidebar-section">
        <div class="quick-search">
          <Icon icon="magnifying-glass" size={14} color="var(--text-muted)" />
          <input 
            type="text" 
            placeholder="Quick search..."
            bind:value={appState.query}
            oninput={() => appState.debouncedSearch()}
          />
        </div>
      </div>

      <!-- Index Locations -->
      <div class="sidebar-section">
        <div class="section-header">
          <span class="section-title">
            <Icon icon="folder" size={14} />
            <span>Index Locations</span>
          </span>
          <button class="add-btn" onclick={addIndexFolder} title="Add folder to index">
            <Icon icon="plus" size={12} />
          </button>
        </div>
        
        {#if appState.settings.index_dirs.length > 0}
          <ul class="folder-list">
            {#each appState.settings.index_dirs as dir}
              {@const folderName = dir.split(/[\\/]/).pop() || dir}
              <li class="folder-item" class:active={appState.activeFolder === dir}>
                <button onclick={() => { appState.activeFolder = dir; appState.activeTab = "search"; }}>
                  <Icon icon={expandedFolders.has(dir) ? "folder-open" : "folder"} size={14} color="var(--accent-orange)" />
                  <span class="folder-name" title={dir}>{folderName}</span>
                </button>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="empty-hint">No folders indexed</p>
        {/if}
      </div>

      <!-- Pinned Files -->
      <div class="sidebar-section">
        <div class="section-header">
          <span class="section-title">
            <Icon icon="pin" size={14} />
            <span>Pinned</span>
            {#if appState.pinnedFiles.length > 0}
              <span class="count">{appState.pinnedFiles.length}</span>
            {/if}
          </span>
        </div>
        
        {#if appState.pinnedFiles.length > 0}
          <ul class="file-list">
            {#each appState.pinnedFiles.slice(0, 5) as path}
              {@const filename = path.split(/[\\/]/).pop() || ''}
              <li>
                <button 
                  onclick={() => { appState.showPreview(path); appState.activeTab = "search"; }}
                  title={path}
                >
                  <Icon icon={getFileIcon(filename)} size={14} color={getFileColor(filename)} />
                  <span class="file-name">{filename}</span>
                </button>
              </li>
            {/each}
            {#if appState.pinnedFiles.length > 5}
              <li class="more">
                <button onclick={() => appState.showPinned = true}>
                  +{appState.pinnedFiles.length - 5} more
                </button>
              </li>
            {/if}
          </ul>
        {:else}
          <p class="empty-hint">No pinned files</p>
        {/if}
      </div>

      <!-- Recent Files -->
      <div class="sidebar-section">
        <div class="section-header">
          <span class="section-title">
            <Icon icon="clock" size={14} />
            <span>Recent</span>
          </span>
        </div>
        
        {#if appState.recentFiles.length > 0}
          <ul class="file-list">
            {#each appState.recentFiles.slice(0, 5) as file}
              {@const filename = file.path.split(/[\\/]/).pop() || ''}
              <li>
                <button 
                  onclick={() => { appState.showPreview(file.path); appState.activeTab = "search"; }}
                  title={file.path}
                >
                  <Icon icon={getFileIcon(filename)} size={14} color={getFileColor(filename)} />
                  <span class="file-name">{filename}</span>
                </button>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="empty-hint">No recent files</p>
        {/if}
      </div>

      <!-- Quick Actions -->
      <div class="sidebar-section">
        <div class="section-header">
          <span class="section-title">
            <Icon icon="lightning" size={14} />
            <span>Quick Actions</span>
          </span>
        </div>
        <div class="quick-actions">
          <button class="action-btn" onclick={() => appState.startIndexing()} disabled={appState.isIndexing}>
            <Icon icon="refresh" size={14} />
            <span>{appState.isIndexing ? "Indexing..." : "Rebuild Index"}</span>
          </button>
          <button class="action-btn" onclick={() => appState.activeTab = "settings"}>
            <Icon icon="settings" size={14} />
            <span>Settings</span>
          </button>
        </div>
      </div>

    </div>
  {/if}

  <!-- Collapsed state icons -->
  {#if appState.sidebarCollapsed}
    <div class="collapsed-icons">
      <button 
        class="collapsed-btn" 
        onclick={() => appState.sidebarCollapsed = false}
        title="Flash Search"
      >
        <Icon icon="magnifying-glass" size={20} />
      </button>
      <button 
        class="collapsed-btn" 
        onclick={() => { appState.activeFolder = null; appState.activeTab = "search"; appState.sidebarCollapsed = false; }}
        title="Search"
      >
        <Icon icon="search" size={20} />
      </button>
      <button 
        class="collapsed-btn" 
        onclick={addIndexFolder}
        title="Add folder"
      >
        <Icon icon="folder" size={20} />
      </button>
      <button 
        class="collapsed-btn" 
        onclick={() => appState.activeTab = "settings"}
        title="Settings"
      >
        <Icon icon="settings" size={20} />
      </button>
    </div>
  {/if}
</aside>

<style>
  .sidebar {
    width: 260px;
    min-width: 260px;
    background: var(--bg-card);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    transition: width 0.2s ease, min-width 0.2s ease;
    overflow: hidden;
  }

  .sidebar.collapsed {
    width: 56px;
    min-width: 56px;
  }

  .sidebar-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px;
    border-bottom: 1px solid var(--border);
    min-height: 56px;
  }

  .logo {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 600;
    font-size: 15px;
    color: var(--text);
  }

  .collapse-btn {
    background: none;
    border: none;
    padding: 6px;
    cursor: pointer;
    color: var(--text-secondary);
    border-radius: var(--radius-sm);
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.2s;
  }

  .collapse-btn:hover {
    background: var(--bg);
    color: var(--text);
  }

  .sidebar.collapsed .sidebar-header {
    justify-content: center;
    padding: 16px 8px;
  }

  .sidebar-content {
    flex: 1;
    overflow-y: auto;
    padding: 12px 0;
  }

  .sidebar-section {
    margin-bottom: 16px;
    padding: 0 12px;
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }

  .section-title {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    color: var(--text-muted);
    letter-spacing: 0.5px;
  }

  .section-title .count {
    background: var(--primary);
    color: white;
    font-size: 10px;
    padding: 1px 5px;
    border-radius: 8px;
    margin-left: 4px;
  }

  .add-btn {
    background: none;
    border: none;
    padding: 4px;
    cursor: pointer;
    color: var(--text-muted);
    border-radius: var(--radius-sm);
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.2s;
  }

  .add-btn:hover {
    background: var(--bg);
    color: var(--primary);
  }

  .quick-search {
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 0 10px;
  }

  .quick-search input {
    flex: 1;
    border: none;
    background: transparent;
    padding: 8px 0;
    font-size: 13px;
    color: var(--text);
    outline: none;
  }

  .quick-search input::placeholder {
    color: var(--text-muted);
  }

  .folder-list, .file-list {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .folder-item button, .file-list li button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    background: none;
    border: none;
    cursor: pointer;
    border-radius: var(--radius-sm);
    text-align: left;
    transition: all 0.15s;
    color: var(--text);
    font-size: 13px;
  }

  .folder-item button:hover, .file-list li button:hover {
    background: var(--bg);
  }

  .folder-item.active button {
    background: var(--primary-light);
    color: var(--primary);
  }

  .folder-name, .file-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
  }

  .file-list li.more button {
    color: var(--text-muted);
    font-size: 12px;
  }

  .empty-hint {
    font-size: 12px;
    color: var(--text-muted);
    padding: 8px 10px;
    margin: 0;
  }

  .quick-actions {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .quick-actions .action-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    cursor: pointer;
    color: var(--text-secondary);
    font-size: 12px;
    transition: all 0.15s;
  }

  .quick-actions .action-btn:hover {
    background: var(--bg-elevated);
    color: var(--text);
    border-color: var(--text-secondary);
  }

  .quick-actions .action-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .collapsed-icons {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 12px 0;
    gap: 8px;
  }

  .collapsed-btn {
    width: 40px;
    height: 40px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    cursor: pointer;
    color: var(--text-secondary);
    border-radius: var(--radius-md);
    transition: all 0.15s;
  }

  .collapsed-btn:hover {
    background: var(--bg);
    color: var(--primary);
  }
</style>

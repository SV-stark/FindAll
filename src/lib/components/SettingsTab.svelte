<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { fade, slide } from "svelte/transition";
  import { appState } from '$lib/state.svelte';

  function toggleSection(section: string) {
    appState.toggleSection(section as any);
  }

  async function addIndexDir() {
    try {
      const selected = await invoke<string | null>("select_folder");
      if (selected && !appState.settings.index_dirs.includes(selected)) {
        appState.settings.index_dirs = [...appState.settings.index_dirs, selected];
        appState.hasChanges = true;
      }
    } catch (e) {
      console.error("Failed to pick folder:", e);
    }
  }

  async function addExcludeFolder() {
    try {
      const selected = await invoke<string | null>("select_folder");
      if (selected && !appState.settings.exclude_folders.includes(selected)) {
        appState.settings.exclude_folders = [...appState.settings.exclude_folders, selected];
        appState.hasChanges = true;
      }
    } catch (e) {
      console.error("Failed to pick folder:", e);
    }
  }

  function addExcludePattern(input: HTMLInputElement) {
    const value = input.value;
    if (value && !appState.settings.exclude_patterns.includes(value)) {
      appState.settings.exclude_patterns = [...appState.settings.exclude_patterns, value];
      appState.hasChanges = true;
      input.value = '';
    }
  }
</script>

<div class="settings-container" in:fade={{ duration: 200 }}>
  <div class="settings-header">
    <h1>Settings</h1>
    <div class="header-actions">
      <button class="btn-secondary" onclick={() => appState.resetToDefaults()}>Reset to Defaults</button>
      <button class="btn-primary" onclick={() => appState.saveSettings()} disabled={!appState.hasChanges}>
        {appState.hasChanges ? 'Save Changes' : 'Saved'}
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
      <span class="expand-icon">{appState.expandedSections.indexing ? '▼' : '▶'}</span>
    </button>
    
    {#if appState.expandedSections.indexing}
      <div class="section-content" transition:slide>
        <div class="setting-group">
          <label class="setting-label">Index Locations</label>
          <p class="setting-description">Folders that will be scanned and indexed for searching</p>
          <div class="tag-list">
            {#each appState.settings.index_dirs as dir}
              <div class="tag">
                <span class="tag-text">{dir}</span>
                <button class="tag-remove" onclick={() => {
                  appState.settings.index_dirs = appState.settings.index_dirs.filter(d => d !== dir);
                  appState.hasChanges = true;
                }}>✕</button>
              </div>
            {/each}
            {#if appState.settings.index_dirs.length === 0}
              <span class="empty-hint">No directories added (Home folder indexed by default)</span>
            {/if}
          </div>
          <button class="btn-secondary" onclick={addIndexDir}>
            + Add Folder
          </button>
        </div>

        <div class="setting-group">
          <label class="setting-label">Exclusion Patterns</label>
          <p class="setting-description">Files and folders matching these patterns will be skipped</p>
          <div class="tag-list">
            {#each appState.settings.exclude_patterns as pattern}
              <div class="tag secondary">
                <span class="tag-text">{pattern}</span>
                <button class="tag-remove" onclick={() => {
                  appState.settings.exclude_patterns = appState.settings.exclude_patterns.filter(p => p !== pattern);
                  appState.hasChanges = true;
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
                  addExcludePattern(e.target as HTMLInputElement);
                }
              }}
            />
            <button class="btn-secondary" onclick={(e) => {
              const input = (e.currentTarget.previousElementSibling as HTMLInputElement);
              addExcludePattern(input);
            }}>Add</button>
          </div>
        </div>

        <div class="setting-group">
          <label class="setting-label">Excluded Folders</label>
          <p class="setting-description">Specific folder paths to exclude from indexing</p>
          <div class="tag-list">
            {#each appState.settings.exclude_folders as folder}
              <div class="tag warning">
                <span class="tag-text">{folder}</span>
                <button class="tag-remove" onclick={() => {
                  appState.settings.exclude_folders = appState.settings.exclude_folders.filter(f => f !== folder);
                  appState.hasChanges = true;
                }}>✕</button>
              </div>
            {/each}
            {#if appState.settings.exclude_folders.length === 0}
              <span class="empty-hint">No folders excluded</span>
            {/if}
          </div>
          <button class="btn-secondary" onclick={addExcludeFolder}>
            + Add Excluded Folder
          </button>
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
      <span class="expand-icon">{appState.expandedSections.search ? '▼' : '▶'}</span>
    </button>
    
    {#if appState.expandedSections.search}
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
            value={appState.settings.max_results}
            onchange={(e) => appState.updateSetting('max_results', parseInt((e.target as HTMLInputElement).value))}
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
              checked={appState.settings.search_history_enabled}
              onchange={(e) => appState.updateSetting('search_history_enabled', (e.target as HTMLInputElement).checked)}
            />
            <span class="toggle-slider"></span>
          </label>
        </div>
      </div>
    {/if}
  </div>

  <div class="settings-footer">
    <p>Flash Search v0.2.0 • <a href="#" onclick={() => invoke('open_folder', { path: '.' })}>Open Data Folder</a></p>
  </div>
</div>

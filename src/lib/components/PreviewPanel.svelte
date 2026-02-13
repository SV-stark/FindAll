<script lang="ts">
  import { fade } from "svelte/transition";
  import { appState } from '$lib/state.svelte';
</script>

<div class="preview-panel" transition:fade>
  <div class="preview-header">
    <span class="preview-title">{appState.selectedPath?.split(/[\\/]/).pop()}</span>
    <div class="preview-actions">
      <button class="preview-btn" onclick={() => appState.isQuickLookOpen = true} title="Quick Look (Space)">👁️</button>
      <button class="preview-btn" onclick={() => { appState.selectedPath = null; appState.previewContent = null; appState.selectedIndex = -1; }}>✕</button>
    </div>
  </div>
  <div class="preview-content">
    {#if appState.highlightedPreview}
      <!-- eslint-disable-next-line svelte/no-at-html-tags -->
      <pre>{@html appState.highlightedPreview}</pre>
    {:else}
      <div class="preview-loading">Loading...</div>
    {/if}
  </div>
</div>

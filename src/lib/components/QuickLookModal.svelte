<script lang="ts">
  import { fade } from "svelte/transition";
  import { appState } from '$lib/state.svelte';
  import Icon from './Icon.svelte';

  function close() {
    appState.isQuickLookOpen = false;
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      close();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if appState.isQuickLookOpen && appState.selectedPath}
  <div class="quicklook-overlay" onclick={close} transition:fade>
    <div class="quicklook-modal" onclick={(e) => e.stopPropagation()}>
      <div class="quicklook-header">
        <span class="quicklook-title">{appState.selectedPath.split(/[\\/]/).pop()}</span>
        <button class="close-btn" onclick={close} aria-label="Close quick look">
          <Icon icon="x" size={20} />
        </button>
      </div>
      <div class="quicklook-content">
        <!-- eslint-disable-next-line svelte/no-at-html-tags -->
        <pre>{@html appState.highlightedPreview}</pre>
      </div>
    </div>
  </div>
{/if}

<style>
  .quicklook-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2000;
    backdrop-filter: blur(4px);
  }

  .quicklook-modal {
    background: var(--bg-card);
    border-radius: var(--radius-lg);
    width: 90%;
    max-width: 800px;
    max-height: 90vh;
    display: flex;
    flex-direction: column;
    box-shadow: var(--shadow-lg);
  }

  .quicklook-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border);
  }

  .quicklook-title {
    font-size: 16px;
    font-weight: 500;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .close-btn {
    background: none;
    border: none;
    padding: 8px;
    cursor: pointer;
    color: var(--text-secondary);
    border-radius: var(--radius-sm);
    transition: all 0.2s;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .close-btn:hover {
    background: var(--bg);
    color: var(--text);
  }

  .quicklook-content {
    flex: 1;
    overflow: auto;
    padding: 20px;
  }

  .quicklook-content pre {
    margin: 0;
    white-space: pre-wrap;
    font-family: "Consolas", "Monaco", monospace;
    font-size: 14px;
    line-height: 1.6;
  }
</style>

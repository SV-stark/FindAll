<script lang="ts">
  import { fade } from "svelte/transition";
  import { appState } from '$lib/state.svelte';

  function close() {
    appState.isQuickLookOpen = false;
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      close();
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

{#if appState.isQuickLookOpen && appState.selectedPath}
  <div class="quicklook-overlay" onclick={close} transition:fade>
    <div class="quicklook-modal" onclick={(e) => e.stopPropagation()}>
      <div class="quicklook-header">
        <span class="quicklook-title">{appState.selectedPath.split(/[\\/]/).pop()}</span>
        <button class="close-btn" onclick={close}>✕</button>
      </div>
      <div class="quicklook-content">
        <!-- eslint-disable-next-line svelte/no-at-html-tags -->
        <pre>{@html appState.highlightedPreview}</pre>
      </div>
    </div>
  </div>
{/if}

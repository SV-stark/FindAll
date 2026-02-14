<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import { appState } from '$lib/state.svelte';
  import Icon from './Icon.svelte';

  interface Shortcut {
    keys: string;
    description: string;
    category: string;
  }

  const shortcuts: Shortcut[] = [
    { keys: "↑ / ↓", description: "Navigate through results", category: "Navigation" },
    { keys: "Enter", description: "Open selected file", category: "Navigation" },
    { keys: "Space", description: "Quick look at selected file", category: "Navigation" },
    { keys: "Escape", description: "Clear search / Close modal", category: "Navigation" },
    { keys: "Ctrl + P", description: "Pin/Unpin selected file", category: "Actions" },
    { keys: "Ctrl + C", description: "Copy file path to clipboard", category: "Actions" },
    { keys: "?", description: "Show this help dialog", category: "Help" },
  ];

  function close() {
    appState.showShortcuts = false;
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      close();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if appState.showShortcuts}
  <div class="shortcuts-overlay" transition:fade={{ duration: 150 }} onclick={close} role="dialog" aria-modal="true" aria-label="Keyboard shortcuts">
    <div class="shortcuts-modal" transition:scale={{ duration: 200, start: 0.95 }} onclick={(e) => e.stopPropagation()} role="document">
      <div class="shortcuts-header">
        <h2>
          <Icon icon="keyboard" size={20} />
          <span>Keyboard Shortcuts</span>
        </h2>
        <button class="close-btn" onclick={close} aria-label="Close">
          <Icon icon="x" size={20} />
        </button>
      </div>
      
      <div class="shortcuts-content">
        {#each ["Navigation", "Actions", "Help"] as category}
          <div class="shortcut-category">
            <h3>{category}</h3>
            {#each shortcuts.filter(s => s.category === category) as shortcut}
              <div class="shortcut-item">
                <kbd>{shortcut.keys}</kbd>
                <span>{shortcut.description}</span>
              </div>
            {/each}
          </div>
        {/each}
      </div>
      
      <div class="shortcuts-footer">
        <p>Press <kbd>?</kbd> anytime to show this dialog</p>
      </div>
    </div>
  </div>
{/if}

<style>
  .shortcuts-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2000;
    backdrop-filter: blur(4px);
  }

  .shortcuts-modal {
    background: var(--bg-card);
    border-radius: var(--radius-lg);
    width: 90%;
    max-width: 480px;
    box-shadow: var(--shadow-lg);
    overflow: hidden;
  }

  .shortcuts-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 20px 24px;
    border-bottom: 1px solid var(--border);
  }

  .shortcuts-header h2 {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 0;
    font-size: 18px;
    font-weight: 600;
  }

  .close-btn {
    background: none;
    border: none;
    padding: 8px;
    cursor: pointer;
    color: var(--text-secondary);
    border-radius: var(--radius-sm);
    transition: all 0.2s;
  }

  .close-btn:hover {
    background: var(--bg);
    color: var(--text);
  }

  .shortcuts-content {
    padding: 20px 24px;
  }

  .shortcut-category {
    margin-bottom: 20px;
  }

  .shortcut-category:last-child {
    margin-bottom: 0;
  }

  .shortcut-category h3 {
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    color: var(--text-muted);
    margin: 0 0 12px;
    letter-spacing: 0.5px;
  }

  .shortcut-item {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 10px 0;
    border-bottom: 1px solid var(--border);
  }

  .shortcut-item:last-child {
    border-bottom: none;
  }

  kbd {
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 4px 10px;
    font-size: 12px;
    font-family: inherit;
    font-weight: 500;
    color: var(--text);
    min-width: 80px;
    text-align: center;
  }

  .shortcut-item span {
    color: var(--text-secondary);
    font-size: 14px;
  }

  .shortcuts-footer {
    padding: 16px 24px;
    background: var(--bg);
    text-align: center;
  }

  .shortcuts-footer p {
    margin: 0;
    font-size: 13px;
    color: var(--text-muted);
  }

  .shortcuts-footer kbd {
    background: var(--bg-card);
    border-color: var(--text-secondary);
  }
</style>

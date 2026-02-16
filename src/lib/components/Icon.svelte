<script lang="ts">
  import type { Component } from 'svelte';
  
  export let size: number = 20;
  export let weight: 'regular' | 'bold' = 'regular';
  export let color: string = 'currentColor';
</script>

<!-- Search Icon -->
{#if $$props.icon === 'search'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 24 : 16} stroke-linecap="round" stroke-linejoin="round">
  <circle cx="112" cy="112" r="80"/>
  <line x1="168" y1="168" x2="224" y2="224"/>
</svg>

<!-- Folder Icon -->
{:else if $$props.icon === 'folder'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <path d="M216 72H104a8 8 0 0 1-8-8V40a8 8 0 0 0-8-8H40a8 8 0 0 0-8 8v128a8 8 0 0 0 8 8h160a8 8 0 0 0 8-8V88a8 8 0 0 0-8-8Z"/>
</svg>

<!-- FolderOpen Icon -->
{:else if $$props.icon === 'folder-open'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <path d="M216 72H104a8 8 0 0 1-8-8V40a8 8 0 0 0-8-8H40a8 8 0 0 0-8 8v128a8 8 0 0 0 8 8h160a8 8 0 0 0 8-8V96a8 8 0 0 0-8-8Z"/>
  <path d="M216 128v64a8 8 0 0 1-8 8H48a8 8 0 0 1-6.3-13.7l28.7-38a8 8 0 0 1 12.6 0l28.7 38"/>
</svg>

<!-- File Icon -->
{:else if $$props.icon === 'file'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <path d="M160 40H64a8 8 0 0 0-8 8v144a8 8 0 0 0 8 8h128a8 8 0 0 0 8-8V88L176 40a8 8 0 0 0-16 0Z"/>
  <path d="M176 40v56a8 8 0 0 0 8 8h56"/>
</svg>

<!-- Settings Gear Icon -->
{:else if $$props.icon === 'settings'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <circle cx="128" cy="128" r="24"/>
  <path d="M128 72v-16a8 8 0 0 0-8-8H72a8 8 0 0 0-8 8v16a8 8 0 0 0 8 8h48a8 8 0 0 0 8-8Z"/>
  <path d="M184 104v-16a8 8 0 0 0-8-8h-16a8 8 0 0 0-8 8v16a8 8 0 0 0 8 8h16a8 8 0 0 0 8-8Z"/>
  <path d="M184 184v-16a8 8 0 0 0-8-8h-16a8 8 0 0 0-8 8v16a8 8 0 0 0 8 8h16a8 8 0 0 0 8-8Z"/>
  <path d="M72 104v-16a8 8 0 0 0-8-8H48a8 8 0 0 0-8 8v16a8 8 0 0 0 8 8h16a8 8 0 0 0 8-8Z"/>
  <path d="M128 184v-16a8 8 0 0 0-8-8H72a8 8 0 0 0-8 8v16a8 8 0 0 0 8 8h48a8 8 0 0 0 8-8Z"/>
</svg>

<!-- Chart Bar Icon -->
{:else if $$props.icon === 'chart'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <path d="M216 216H40a8 8 0 0 1-8-8V96a8 8 0 0 1 8-8h8"/>
  <path d="M96 216V136"/>
  <path d="M144 216V96"/>
  <path d="M192 216V56"/>
</svg>

<!-- Pin Icon -->
{:else if $$props.icon === 'pin'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <path d="M192 32l-40 56-56-72H72l56 72-40 56a8 8 0 0 0 0 16l40 56 56-72h40l-56 72 40 56a8 8 0 0 0 16 0l40-56 56 72h-24l-56-72 40-56a8 8 0 0 0 0-16Z"/>
</svg>

<!-- Pin Fill Icon -->
{:else if $$props.icon === 'pin-filled'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill={color}>
  <path d="M232 64l-40 56-56-72H72l56 72-40 56a8 8 0 0 0 0 16l40 56 56-72h40l-56 72 40 56a8 8 0 0 0 16 0l40-56 56 72h-24l-56-72 40-56a8 8 0 0 0 0-16Z"/>
</svg>

<!-- Clock Icon -->
{:else if $$props.icon === 'clock'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <circle cx="128" cy="128" r="96"/>
  <polyline points="128 80 128 128 176 144"/>
</svg>

<!-- Copy Icon -->
{:else if $$props.icon === 'copy'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <rect x="64" y="64" width="144" height="144" rx="8"/>
  <path d="M192 192V64a8 8 0 0 0-8-8H64a8 8 0 0 0-8 8v128a8 8 0 0 0 8 8h128"/>
</svg>

<!-- Folder Simple Icon -->
{:else if $$props.icon === 'folder-simple'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <path d="M216 64H104a8 8 0 0 1-8-8V40a8 8 0 0 0-8-8H40a8 8 0 0 0-8 8v128a8 8 0 0 0 8 8h160a8 8 0 0 0 8-8V88a8 8 0 0 0-8-8Z"/>
  <path d="M96 112h64"/>
</svg>

<!-- Check Icon -->
{:else if $$props.icon === 'check'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 24 : 20} stroke-linecap="round" stroke-linejoin="round">
  <polyline points="80 136 136 192 216 64"/>
</svg>

<!-- X Close Icon -->
{:else if $$props.icon === 'x'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 24 : 20} stroke-linecap="round" stroke-linejoin="round">
  <line x1="56" y1="56" x2="200" y2="200"/>
  <line x1="200" y1="56" x2="56" y2="200"/>
</svg>

<!-- Chevron Down Icon -->
{:else if $$props.icon === 'chevron-down'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <polyline points="80 104 128 152 176 104"/>
</svg>

<!-- Chevron Right Icon -->
{:else if $$props.icon === 'chevron-right'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <polyline points="104 80 152 128 104 176"/>
</svg>

<!-- Plus Icon -->
{:else if $$props.icon === 'plus'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 24 : 20} stroke-linecap="round" stroke-linejoin="round">
  <line x1="128" y1="40" x2="128" y2="216"/>
  <line x1="40" y1="128" x2="216" y2="128"/>
</svg>

<!-- Trash Icon -->
{:else if $$props.icon === 'trash'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <path d="M200 72v144a8 8 0 0 1-8 8H64a8 8 0 0 1-8-8V72"/>
  <path d="M176 72H80"/>
  <path d="M112 112v64"/>
  <path d="M144 112v64"/>
  <path d="M48 72l8 136h144l8-136"/>
</svg>

<!-- Magnifying Glass Icon -->
{:else if $$props.icon === 'magnifying-glass'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <circle cx="112" cy="112" r="72"/>
  <line x1="168" y1="168" x2="224" y2="224"/>
</svg>

<!-- Lightning Icon -->
{:else if $$props.icon === 'lightning'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <path d="M216 112L96 176l32-144-112 64 24-128-96 120 160-88"/>
</svg>

<!-- Text Icon -->
{:else if $$props.icon === 'text'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <path d="M80 64h96"/>
  <path d="M80 128h96"/>
  <path d="M80 192h40"/>
  <path d="M176 192h-24"/>
  <path d="M80 64V48h96v16"/>
</svg>

<!-- Document Icon -->
{:else if $$props.icon === 'document'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <path d="M160 40H64a8 8 0 0 0-8 8v144a8 8 0 0 0 8 8h128a8 8 0 0 0 8-8V88L176 40a8 8 0 0 0-16 0Z"/>
  <path d="M176 40v56a8 8 0 0 0 8 8h56"/>
  <path d="M96 128h64"/>
  <path d="M96 160h64"/>
</svg>

<!-- Export Icon -->
{:else if $$props.icon === 'export'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <path d="M216 160v48a8 8 0 0 1-8 8H48a8 8 0 0 1-8-8v-48"/>
  <path d="M128 168V48"/>
  <path d="M64 104l64-64 64 64"/>
  <path d="M168 48H88"/>
</svg>

<!-- Download Icon -->
{:else if $$props.icon === 'download'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <path d="M216 160v48a8 8 0 0 1-8 8H48a8 8 0 0 1-8-8v-48"/>
  <path d="M128 168V48"/>
  <path d="M64 104l64 64 64-64"/>
</svg>

<!-- Arrow Up Icon -->
{:else if $$props.icon === 'arrow-up'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <line x1="128" y1="208" x2="128" y2="48"/>
  <polyline points="64 104 128 40 192 104"/>
</svg>

<!-- Arrow Down Icon -->
{:else if $$props.icon === 'arrow-down'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <line x1="128" y1="48" x2="128" y2="208"/>
  <polyline points="64 152 128 216 192 152"/>
</svg>

<!-- Question Mark Icon -->
{:else if $$props.icon === 'question'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <circle cx="128" cy="156" r="64"/>
  <path d="M112 108c0-17.7 10.3-33 24-33 5.4 0 10 4.6 10 10 0 8-10 11-10 23"/>
  <path d="M120 172h16"/>
</svg>

<!-- Keyboard Icon -->
{:else if $$props.icon === 'keyboard'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <rect x="16" y="64" width="224" height="144" rx="16"/>
  <path d="M80 112h8"/>
  <path d="M104 112h8"/>
  <path d="M128 112h8"/>
  <path d="M152 112h8"/>
  <path d="M176 112h8"/>
  <path d="M64 144h8"/>
  <path d="M88 144h8"/>
  <path d="M112 144h8"/>
  <path d="M136 144h8"/>
  <path d="M160 144h8"/>
  <path d="M184 144h8"/>
  <path d="M96 176h64"/>
</svg>

<!-- Eye Icon -->
{:else if $$props.icon === 'eye'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <path d="M128 64c-64 0-96 48-96 88s32 88 96 88 96-48 96-88-32-88-96-88Z"/>
  <circle cx="128" cy="152" r="24"/>
</svg>

<!-- Funnel Icon -->
{:else if $$props.icon === 'funnel'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <path d="M216 72H40l64 112 64-112Z"/>
  <path d="M40 72l64 56 64-56"/>
</svg>

<!-- Refresh Icon -->
{:else if $$props.icon === 'refresh'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <path d="M213.7 85.7a8 8 0 0 0-11.4 0l-24 24a8 8 0 0 0 0 11.4l24 24a8 8 0 0 0 11.4-11.4L195.4 136"/>
  <path d="M42.3 170.3a8 8 0 0 0 11.4 0l24-24a8 8 0 0 0 0-11.4l-24-24a8 8 0 0 0-11.4 11.4L60.6 120"/>
  <path d="M213.7 42.3A96 96 0 0 0 128 32"/>
  <path d="M42.3 213.7A96 96 0 0 0 128 224"/>
</svg>

<!-- Home Icon -->
{:else if $$props.icon === 'home'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <path d="M208 128l-80-72V40a8 8 0 0 0-8-8H48a8 8 0 0 0-8 8v80a8 8 0 0 0 8 8l80 72"/>
  <path d="M96 224V136h64v88"/>
</svg>

<!-- Info Icon -->
{:else if $$props.icon === 'info'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <circle cx="128" cy="128" r="96"/>
  <path d="M128 80v80"/>
  <path d="M128 64h8"/>
</svg>

<!-- Warning Icon -->
{:else if $$props.icon === 'warning'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <path d="M128 32l104 184H24L128 32Z"/>
  <path d="M128 176v40"/>
  <path d="M128 96v16"/>
</svg>

<!-- External Link Icon -->
{:else if $$props.icon === 'external-link'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <path d="M216 160v48a8 8 0 0 1-8 8H48a8 8 0 0 1-8-8v-48"/>
  <path d="M176 80h-48"/>
  <path d="M128 80 216 168"/>
</svg>

<!-- Spinner / Loader Icon -->
{:else if $$props.icon === 'spinner'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <circle cx="128" cy="128" r="88" stroke-dasharray="200" stroke-dashoffset="120" stroke-linecap="round"/>
</svg>

<!-- Sort Icon -->
{:else if $$props.icon === 'sort'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <path d="M48 72h160"/>
  <path d="M48 128h112"/>
  <path d="M48 184h72"/>
  <path d="M200 72l16-16 16 16"/>
  <path d="M200 184l16-16 16 16"/>
</svg>

<!-- Sort Ascending Icon -->
{:else if $$props.icon === 'sort-asc'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <path d="M48 208h160"/>
  <path d="M48 144h112"/>
  <path d="M48 80h72"/>
  <path d="M184 80l16 16 16-16"/>
</svg>

<!-- Sort Descending Icon -->
{:else if $$props.icon === 'sort-desc'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <path d="M48 48h160"/>
  <path d="M48 112h112"/>
  <path d="M48 176h72"/>
  <path d="M184 176l-16-16-16 16"/>
</svg>

<!-- Calendar Icon -->
{:else if $$props.icon === 'calendar'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <rect x="32" y="56" width="192" height="176" rx="16"/>
  <path d="M32 104h192"/>
  <path d="M88 56V40"/>
  <path d="M168 56V40"/>
  <path d="M88 152v32"/>
  <path d="M128 152v32"/>
  <path d="M168 152v32"/>
</svg>

<!-- Database Icon -->
{:else if $$props.icon === 'database'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <ellipse cx="128" cy="56" rx="88" ry="24"/>
  <path d="M40 56v144c0 13.3 39.5 24 88 24s88-10.7 88-24V56"/>
  <path d="M40 128c0 13.3 39.5 24 88 24s88-10.7 88-24"/>
</svg>

<!-- Check Circle Icon -->
{:else if $$props.icon === 'check-circle'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <circle cx="128" cy="128" r="96"/>
  <polyline points="96 128 116 148 160 104"/>
</svg>

<!-- Folder Search Icon -->
{:else if $$props.icon === 'folder-search'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <path d="M216 72H104a8 8 0 0 1-8-8V40a8 8 0 0 0-8-8H40a8 8 0 0 0-8 8v128a8 8 0 0 0 8 8h160a8 8 0 0 0 8-8V88a8 8 0 0 0-8-8Z"/>
  <circle cx="140" cy="132" r="36"/>
  <line x1="168" y1="168" x2="216" y2="216"/>
</svg>

<!-- X Circle Icon -->
{:else if $$props.icon === 'x-circle'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <circle cx="128" cy="128" r="96"/>
  <line x1="96" y1="96" x2="160" y2="160"/>
  <line x1="160" y1="96" x2="96" y2="160"/>
</svg>

<!-- Hard Drive Icon -->
{:else if $$props.icon === 'hard-drive'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <rect x="32" y="48" width="192" height="160" rx="16"/>
  <path d="M96 100h8v56h-8z"/>
  <path d="M120 100h8v56h-8z"/>
  <path d="M144 100h8v56h-8z"/>
  <path d="M64 192h128"/>
</svg>

<!-- File Text Icon -->
{:else if $$props.icon === 'file-text'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <path d="M160 40H64a8 8 0 0 0-8 8v144a8 8 0 0 0 8 8h128a8 8 0 0 0 8-8V88L176 40a8 8 0 0 0-16 0Z"/>
  <path d="M176 40v56a8 8 0 0 0 8 8h56"/>
  <path d="M96 136h64"/>
  <path d="M96 160h64"/>
  <path d="M96 184h40"/>
</svg>

<!-- Image Icon -->
{:else if $$props.icon === 'image'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <rect x="32" y="32" width="192" height="192" rx="16"/>
  <circle cx="96" cy="96" r="16"/>
  <path d="m144 192-56-56 56-56"/>
</svg>

<!-- Music Icon -->
{:else if $$props.icon === 'music'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <circle cx="80" cy="136" r="32"/>
  <path d="M192 96v80"/>
  <path d="M144 72v112a8 8 0 0 1-16 0"/>
</svg>

<!-- Video Icon -->
{:else if $$props.icon === 'video'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <rect x="32" y="48" width="176" height="160" rx="16"/>
  <path d="M144 108l40-24v64l-40-24"/>
</svg>

<!-- Code Icon -->
{:else if $$props.icon === 'code'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <polyline points="80 64 32 128 80 192"/>
  <polyline points="176 64 224 128 176 192"/>
</svg>

<!-- Archive Icon -->
{:else if $$props.icon === 'archive'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <rect x="32" y="72" width="192" height="144" rx="8"/>
  <path d="M32 104h192"/>
  <path d="M96 72V56a8 8 0 0 1 8-8h48a8 8 0 0 1 8 8v16"/>
  <circle cx="200" cy="168" r="8"/>
</svg>

<!-- Users Icon -->
{:else if $$props.icon === 'users'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <circle cx="96" cy="80" r="32"/>
  <path d="M32 192c0-26.5 21.5-48 48-48h16c26.5 0 48 21.5 48 48"/>
  <circle cx="176" cy="80" r="32"/>
  <path d="M160 192c0-26.5 21.5-48 48-48h16c26.5 0 48 21.5 48 48"/>
</svg>

<!-- Star Icon -->
{:else if $$props.icon === 'star'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12} stroke-linecap="round" stroke-linejoin="round">
  <polygon points="128 24 156.9 86.1 224 99 176 147 188.9 215 128 184 67.1 215 80 147 32 99 99.1 86.1 128 24"/>
</svg>

<!-- Star Filled Icon -->
{:else if $$props.icon === 'star-filled'}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill={color}>
  <polygon points="128 24 156.9 86.1 224 99 176 147 188.9 215 128 184 67.1 215 80 147 32 99 99.1 86.1 128 24"/>
</svg>

<!-- Default fallback - circle -->
{:else}
<svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 256 256" fill="none" stroke={color} stroke-width={weight === 'bold' ? 16 : 12}>
  <circle cx="128" cy="128" r="96"/>
</svg>
{/if}

<style>
  svg {
    display: inline-block;
    vertical-align: middle;
    flex-shrink: 0;
  }
  
  .spinner {
    animation: spin 1s linear infinite;
  }
  
  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>

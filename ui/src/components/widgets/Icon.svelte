<!-- reason: monolithic SVG sprite map, no logic — exempt from the 350-line cap -->
<script lang="ts">
  // One Svelte component for the entire icon set so colour/size flow through
  // currentColor without 30 tiny files. Matches the design's icon vocabulary.

  type Name =
    | 'check'
    | 'caret'
    | 'grid'
    | 'gear'
    | 'tray'
    | 'move'
    | 'cover'
    | 'reset'
    | 'layout'
    | 'dim'
    | 'plus'
    | 'minus'
    | 'fit'
    | 'save'
    | 'prev'
    | 'next'
    | 'play'
    | 'pause'
    | 'search'
    | 'star'
    | 'star-filled'
    | 'link'
    | 'reveal'
    | 'folder'
    | 'refresh'
    | 'win-min'
    | 'win-max'
    | 'win-close'
    | 'logo'
    | 'logo-lg';

  type Props = { name: Name; size?: number };
  let { name, size = 14 }: Props = $props();
</script>

{#if name === 'check'}
  <svg width={size} height={size} viewBox="0 0 13 13" aria-hidden="true">
    <path
      d="M2.5 6.5l3 3 5-6"
      stroke="currentColor"
      stroke-width="1.8"
      fill="none"
      stroke-linecap="round"
      stroke-linejoin="round"
    ></path>
  </svg>
{:else if name === 'caret'}
  <svg width={size} height={size} viewBox="0 0 10 10" aria-hidden="true">
    <path
      d="M2 4l3 3 3-3"
      stroke="currentColor"
      stroke-width="1.5"
      fill="none"
      stroke-linecap="round"
      stroke-linejoin="round"
    ></path>
  </svg>
{:else if name === 'grid'}
  <svg width={size} height={size} viewBox="0 0 14 14" aria-hidden="true">
    <rect
      x="2"
      y="2"
      width="4"
      height="4"
      rx="0.5"
      fill="none"
      stroke="currentColor"
      stroke-width="1.3"
    ></rect>
    <rect
      x="8"
      y="2"
      width="4"
      height="4"
      rx="0.5"
      fill="none"
      stroke="currentColor"
      stroke-width="1.3"
    ></rect>
    <rect
      x="2"
      y="8"
      width="4"
      height="4"
      rx="0.5"
      fill="none"
      stroke="currentColor"
      stroke-width="1.3"
    ></rect>
    <rect
      x="8"
      y="8"
      width="4"
      height="4"
      rx="0.5"
      fill="none"
      stroke="currentColor"
      stroke-width="1.3"
    ></rect>
  </svg>
{:else if name === 'gear'}
  <svg width={size} height={size} viewBox="0 0 14 14" aria-hidden="true">
    <circle cx="7" cy="7" r="2.2" fill="none" stroke="currentColor" stroke-width="1.3"></circle>
    <path
      d="M7 1.5v1.8M7 10.7v1.8M1.5 7h1.8M10.7 7h1.8M3.1 3.1l1.3 1.3M9.6 9.6l1.3 1.3M3.1 10.9l1.3-1.3M9.6 4.4l1.3-1.3"
      stroke="currentColor"
      stroke-width="1.3"
      stroke-linecap="round"
    ></path>
  </svg>
{:else if name === 'tray'}
  <svg width={size} height={size} viewBox="0 0 14 14" aria-hidden="true">
    <rect
      x="2"
      y="2"
      width="10"
      height="10"
      rx="2"
      fill="none"
      stroke="currentColor"
      stroke-width="1.3"
    ></rect>
    <circle cx="5" cy="9" r="0.7" fill="currentColor"></circle>
    <circle cx="7" cy="9" r="0.7" fill="currentColor"></circle>
    <circle cx="9" cy="9" r="0.7" fill="currentColor"></circle>
  </svg>
{:else if name === 'move'}
  <svg width={size} height={size} viewBox="0 0 14 14" aria-hidden="true">
    <path
      d="M7 2v10M2 7h10M7 2l-1.6 1.6M7 2l1.6 1.6M7 12l-1.6-1.6M7 12l1.6-1.6M2 7l1.6-1.6M2 7l1.6 1.6M12 7l-1.6-1.6M12 7l-1.6 1.6"
      stroke="currentColor"
      stroke-width="1.2"
      fill="none"
      stroke-linecap="round"
    ></path>
  </svg>
{:else if name === 'cover'}
  <svg width={size} height={size} viewBox="0 0 14 14" aria-hidden="true">
    <rect
      x="1.5"
      y="3.5"
      width="11"
      height="7"
      rx="1"
      fill="none"
      stroke="currentColor"
      stroke-width="1.2"
    ></rect>
    <path d="M3 5l8 4M3 9l8-4" stroke="currentColor" stroke-width="1" opacity="0.5"></path>
  </svg>
{:else if name === 'reset'}
  <svg width={size} height={size} viewBox="0 0 14 14" aria-hidden="true">
    <path
      d="M2.5 7a4.5 4.5 0 1 0 1.5-3.4M3 2v2.5h2.5"
      stroke="currentColor"
      stroke-width="1.3"
      fill="none"
      stroke-linecap="round"
      stroke-linejoin="round"
    ></path>
  </svg>
{:else if name === 'layout'}
  <svg width={size} height={size} viewBox="0 0 14 14" aria-hidden="true">
    <rect
      x="1"
      y="3"
      width="3.5"
      height="8"
      rx="0.5"
      fill="none"
      stroke="currentColor"
      stroke-width="1.2"
    ></rect>
    <rect
      x="5.5"
      y="3"
      width="3"
      height="8"
      rx="0.5"
      fill="none"
      stroke="currentColor"
      stroke-width="1.2"
    ></rect>
    <rect
      x="9.5"
      y="3"
      width="3.5"
      height="8"
      rx="0.5"
      fill="none"
      stroke="currentColor"
      stroke-width="1.2"
    ></rect>
  </svg>
{:else if name === 'dim'}
  <svg width={size} height={size} viewBox="0 0 14 14" aria-hidden="true">
    <circle cx="7" cy="7" r="5" fill="none" stroke="currentColor" stroke-width="1.3"></circle>
    <path d="M7 2a5 5 0 0 1 0 10z" fill="currentColor"></path>
  </svg>
{:else if name === 'plus'}
  <svg width={size} height={size} viewBox="0 0 12 12" aria-hidden="true">
    <path d="M6 2v8M2 6h8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"></path>
  </svg>
{:else if name === 'minus'}
  <svg width={size} height={size} viewBox="0 0 12 12" aria-hidden="true">
    <path d="M2 6h8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"></path>
  </svg>
{:else if name === 'fit'}
  <svg width={size} height={size} viewBox="0 0 12 12" aria-hidden="true">
    <path
      d="M2 4V2h2M10 4V2H8M2 8v2h2M10 8v2H8"
      stroke="currentColor"
      stroke-width="1.4"
      fill="none"
      stroke-linecap="round"
    ></path>
  </svg>
{:else if name === 'save'}
  <svg width={size} height={size} viewBox="0 0 12 12" aria-hidden="true">
    <path
      d="M2 2h7l1 1v7H2V2zM4 2v3h4V2M4 7h4v3H4z"
      fill="none"
      stroke="currentColor"
      stroke-width="1.2"
      stroke-linejoin="round"
    ></path>
  </svg>
{:else if name === 'prev'}
  <svg width={size} height={size} viewBox="0 0 12 12" aria-hidden="true">
    <path
      d="M8 2L4 6l4 4"
      stroke="currentColor"
      stroke-width="1.5"
      fill="none"
      stroke-linecap="round"
      stroke-linejoin="round"
    ></path>
  </svg>
{:else if name === 'next'}
  <svg width={size} height={size} viewBox="0 0 12 12" aria-hidden="true">
    <path
      d="M4 2l4 4-4 4"
      stroke="currentColor"
      stroke-width="1.5"
      fill="none"
      stroke-linecap="round"
      stroke-linejoin="round"
    ></path>
  </svg>
{:else if name === 'play'}
  <svg width={size} height={size} viewBox="0 0 12 12" aria-hidden="true">
    <path d="M3 2v8l7-4z" fill="currentColor"></path>
  </svg>
{:else if name === 'pause'}
  <svg width={size} height={size} viewBox="0 0 12 12" aria-hidden="true">
    <rect x="3" y="2" width="2.4" height="8" fill="currentColor"></rect>
    <rect x="6.6" y="2" width="2.4" height="8" fill="currentColor"></rect>
  </svg>
{:else if name === 'search'}
  <svg width={size} height={size} viewBox="0 0 12 12" aria-hidden="true">
    <circle cx="5" cy="5" r="3" fill="none" stroke="currentColor" stroke-width="1.3"></circle>
    <path d="M7.2 7.2L10 10" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"></path>
  </svg>
{:else if name === 'star'}
  <svg width={size} height={size} viewBox="0 0 13 13" aria-hidden="true">
    <path
      d="M6.5 1.5l1.5 3.2 3.4.5-2.5 2.4.6 3.4-3-1.7-3 1.7.6-3.4-2.5-2.4 3.4-.5z"
      fill="none"
      stroke="currentColor"
      stroke-width="1"
      stroke-linejoin="round"
    ></path>
  </svg>
{:else if name === 'star-filled'}
  <svg width={size} height={size} viewBox="0 0 13 13" aria-hidden="true">
    <path
      d="M6.5 1.5l1.5 3.2 3.4.5-2.5 2.4.6 3.4-3-1.7-3 1.7.6-3.4-2.5-2.4 3.4-.5z"
      fill="currentColor"
      stroke="currentColor"
      stroke-width="1"
      stroke-linejoin="round"
    ></path>
  </svg>
{:else if name === 'link'}
  <svg width={size} height={size} viewBox="0 0 12 12" aria-hidden="true">
    <path
      d="M5 3H3a2 2 0 0 0 0 4h2M7 3h2a2 2 0 0 1 0 4H7M4 5h4"
      stroke="currentColor"
      stroke-width="1.3"
      fill="none"
      stroke-linecap="round"
    ></path>
  </svg>
{:else if name === 'reveal'}
  <svg width={size} height={size} viewBox="0 0 12 12" aria-hidden="true">
    <path
      d="M2 3h3l1 1.5h4V9H2z"
      fill="none"
      stroke="currentColor"
      stroke-width="1.2"
      stroke-linejoin="round"
    ></path>
  </svg>
{:else if name === 'folder'}
  <svg width={size} height={size} viewBox="0 0 13 13" aria-hidden="true">
    <path
      d="M1.5 4h3l1 1h6V10h-10z"
      fill="none"
      stroke="currentColor"
      stroke-width="1.2"
      stroke-linejoin="round"
    ></path>
  </svg>
{:else if name === 'refresh'}
  <svg width={size} height={size} viewBox="0 0 12 12" aria-hidden="true">
    <path
      d="M2 6a4 4 0 1 0 1.5-3.1M2 1.5V4h2.5"
      stroke="currentColor"
      stroke-width="1.3"
      fill="none"
      stroke-linecap="round"
      stroke-linejoin="round"
    ></path>
  </svg>
{:else if name === 'win-min'}
  <svg width={size} height={size} viewBox="0 0 10 10" aria-hidden="true">
    <path d="M2 7h6" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"></path>
  </svg>
{:else if name === 'win-max'}
  <svg width={size} height={size} viewBox="0 0 10 10" aria-hidden="true">
    <rect
      x="2"
      y="2"
      width="6"
      height="6"
      fill="none"
      stroke="currentColor"
      stroke-width="1.2"
      rx="0.5"
    ></rect>
  </svg>
{:else if name === 'win-close'}
  <svg width={size} height={size} viewBox="0 0 10 10" aria-hidden="true">
    <path
      d="M2.5 2.5l5 5M7.5 2.5l-5 5"
      stroke="currentColor"
      stroke-width="1.3"
      stroke-linecap="round"
    ></path>
  </svg>
{:else if name === 'logo'}
  <svg width={size} height={size} viewBox="0 0 20 20" aria-hidden="true">
    <rect
      x="1.5"
      y="6"
      width="5"
      height="8"
      rx="0.8"
      fill="none"
      stroke="var(--accent)"
      stroke-width="1.4"
    ></rect>
    <rect x="7.5" y="4" width="5" height="12" rx="0.8" fill="var(--accent)" opacity="0.85"></rect>
    <rect
      x="13.5"
      y="6"
      width="5"
      height="8"
      rx="0.8"
      fill="none"
      stroke="var(--accent)"
      stroke-width="1.4"
    ></rect>
  </svg>
{:else if name === 'logo-lg'}
  <svg width={size} height={size} viewBox="0 0 20 20" aria-hidden="true">
    <rect
      x="1.5"
      y="6"
      width="5"
      height="8"
      rx="0.8"
      fill="none"
      stroke="var(--accent)"
      stroke-width="1.4"
    ></rect>
    <rect x="7.5" y="4" width="5" height="12" rx="0.8" fill="var(--accent)" opacity="0.85"></rect>
    <rect
      x="13.5"
      y="6"
      width="5"
      height="8"
      rx="0.8"
      fill="none"
      stroke="var(--accent)"
      stroke-width="1.4"
    ></rect>
  </svg>
{/if}

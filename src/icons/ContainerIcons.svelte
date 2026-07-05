<script>
  /**
   * ContainerIcons - SVG icons for Docker management module
   */
  let { name = "container", size = 24, class: class_name = "" } = $props();
</script>

{#if name === "container"}
  <!-- Container / Box icon -->
  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" class={class_name} width={size} height={size}>
    <path fill="currentColor" d="M3 3h18v4H3V3zm1 5h16v13H4V8zm2 2v9h12v-9H6zm2 2h8v2H8v-2zm0 3h8v2H8v-2z"/>
  </svg>
{:else if name === "container-running"}
  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" class={class_name} width={size} height={size}>
    <path fill="#3fb950" d="M3 3h18v4H3V3zm1 5h16v13H4V8zm2 2v9h12v-9H6zm3.5 2.5L13 15l-3.5 3.5L8 17l2-2-2-2z"/>
  </svg>
{:else if name === "container-exited"}
  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" class={class_name} width={size} height={size}>
    <path fill="currentColor" opacity="0.5" d="M3 3h18v4H3V3zm1 5h16v13H4V8zm2 2v9h12v-9H6zm4 2h4v2h-4v-2z"/>
  </svg>
{:else if name === "container-paused"}
  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" class={class_name} width={size} height={size}>
    <path fill="#d29922" d="M3 3h18v4H3V3zm1 5h16v13H4V8zm2 2v9h12v-9H6zm4 2h2v5h-2v-5zm4 0h2v5h-2v-5z"/>
  </svg>
{:else if name === "image"}
  <!-- Image / Layers icon -->
  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" class={class_name} width={size} height={size}>
    <path fill="currentColor" d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"/>
  </svg>
{:else if name === "volume"}
  <!-- Volume / Storage icon -->
  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" class={class_name} width={size} height={size}>
    <path fill="currentColor" d="M12 2C6.48 2 2 5.13 2 9c0 2.04 1.23 3.85 3.12 5.02-.07.32-.12.65-.12.98 0 3.87 4.48 7 10 7s10-3.13 10-7c0-.33-.05-.66-.12-.98C20.77 12.85 22 11.04 22 9c0-3.87-4.48-7-10-7zM4 9c0-2.21 3.58-4 8-4s8 1.79 8 4-3.58 4-8 4-8-1.79-8-4zm1 5.76c.69.47 1.5.84 2.38 1.08C8.84 15.59 10.38 16 12 16s3.16-.41 4.62-1.16c.88-.24 1.69-.61 2.38-1.08.48 1.58-2.02 3.24-7 3.24s-7.48-1.66-7-3.24zm0 4c.69.47 1.5.84 2.38 1.08C8.84 19.59 10.38 20 12 20s3.16-.41 4.62-1.16c.88-.24 1.69-.61 2.38-1.08.48 1.58-2.02 3.24-7 3.24s-7.48-1.66-7-3.24z"/>
  </svg>
{:else if name === "network"}
  <!-- Network / Topology icon -->
  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" class={class_name} width={size} height={size}>
    <path fill="currentColor" d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 17.93c-3.94-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93zm6.9-2.54c-.26-.81-1-1.39-1.9-1.39h-1v-3c0-.55-.45-1-1-1H8v-2h2c.55 0 1-.45 1-1V7h2c1.1 0 2-.9 2-2v-.41c2.93 1.19 5 4.06 5 7.41 0 2.08-.8 3.97-2.1 5.39z"/>
  </svg>
{:else if name === "compose"}
  <!-- Compose / Stack icon -->
  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" class={class_name} width={size} height={size}>
    <path fill="currentColor" d="M4 3h6v6H4V3zm10 0h6v6h-6V3zM4 13h6v6H4v-6zm10 0h6v6h-6v-6z"/>
  </svg>
{:else if name === "terminal"}
  <!-- Terminal icon -->
  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" class={class_name} width={size} height={size}>
    <path fill="currentColor" d="M20 4H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2zm0 14H4V6h16v12zM6.5 9l3.5 3.5L6.5 16 5 14.5 7.5 12 5 9.5 6.5 9zm6.5 5h-4v-2h4v2z"/>
  </svg>
{:else if name === "logs"}
  <!-- Logs / Document text icon -->
  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" class={class_name} width={size} height={size}>
    <path fill="currentColor" d="M14 2H6c-1.1 0-2 .9-2 2v16c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V8l-6-6zm4 18H6V4h7v5h5v11zM8 15h8v2H8v-2zm0-4h8v2H8v-2z"/>
  </svg>
{:else if name === "docker-logo"}
  <!-- Docker whale icon (minimal) -->
  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" class={class_name} width={size} height={size}>
    <path fill="currentColor" d="M13.983 11.078h2.119a.186.186 0 0 0 .186-.185V9.006a.186.186 0 0 0-.186-.186h-2.119a.185.185 0 0 0-.185.185v1.887c0 .102.083.186.185.186zm-2.974 0h2.119a.186.186 0 0 0 .186-.185V9.006a.186.186 0 0 0-.186-.186h-2.119a.185.185 0 0 0-.185.185v1.887c0 .102.083.186.185.186zm3.864 1.832h-2.119a.186.186 0 0 0-.185.186v1.886c0 .102.083.185.185.185h2.119a.185.185 0 0 0 .186-.185V13.096a.186.186 0 0 0-.186-.185zm-2.974 0h-2.119a.186.186 0 0 0-.185.186v1.886c0 .102.083.185.185.185h2.119a.185.185 0 0 0 .185-.185V13.096a.186.186 0 0 0-.185-.185zm-2.96 0h-2.12a.186.186 0 0 0-.185.186v1.886c0 .102.083.185.185.185h2.12a.185.185 0 0 0 .185-.185V13.096a.186.186 0 0 0-.185-.185zm3.574-4.076c-.878.406-1.485 1.228-1.485 2.17 0 1.204.985 2.181 2.2 2.181s2.2-.977 2.2-2.18c0-.943-.607-1.765-1.485-2.17z"/>
  </svg>
{:else}
  <!-- Fallback: simple cube -->
  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" class={class_name} width={size} height={size}>
    <path fill="currentColor" d="M21 16.5c0 .38-.21.71-.53.88l-7.9 4.44c-.16.12-.36.18-.57.18-.21 0-.41-.06-.57-.18l-7.9-4.44A.991.991 0 0 1 3 16.5v-9c0-.38.21-.71.53-.88l7.9-4.44c.16-.12.36-.18.57-.18.21 0 .41.06.57.18l7.9 4.44c.32.17.53.5.53.88v9zM12 4.21L6.34 7.5 12 10.79l5.66-3.29L12 4.21zM5 15.91l6 3.38v-6.71L5 9.21v6.7zm14 0v-6.7l-6 3.37v6.71l6-3.38z"/>
  </svg>
{/if}

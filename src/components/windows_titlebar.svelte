<script lang="ts">
  import { CURRENT_WINDOW, dragRegion, focused, maximized } from "$lib/platform/index.svelte";
  import { theme } from "$lib/theme";
  import { version } from "@tauri-apps/plugin-os"
  import type { Snippet } from "svelte";

  const [VERSION_MAJOR, VERSION_MINOR, VERSION_PATCH] = version().split(".");
  const IS_WINDOWS_11 = parseInt(VERSION_PATCH) > 22000; // They didn't bump version_major so 10.0.22000 was the first windows 11 version instead of 11.0.0
  const FONT = IS_WINDOWS_11 ? "Segoe Fluent Icons" : "Segoe MDL2 Assets";

  const CHARACTER_MAPPING = {
    close: "\u{e8bb}",
    minimize: "\u{e921}",
    maximize: "\u{e922}",
    restore: "\u{e923}"
  };

  interface Props {
    contents: Snippet,
  }

  const { contents }: Props = $props();
</script>

{#snippet button(kind: "close" | "maximize" | "restore" | "minimize")}
  <button style="
    font-family: {FONT};
    --icon: {theme(`titlebar-windows-${kind}-icon`)};
    --bg-hover: {theme(`titlebar-windows-${kind}-hover-background`)};
    --icon-hover: {theme(`titlebar-windows-${kind}-hover-icon`)};
    --bg-active: {theme(`titlebar-windows-${kind}-active-background`)};
    --icon-active: {theme(`titlebar-windows-${kind}-active-icon`)};
    --hover-transition-speed: {theme(`titlebar-windows-${kind}-hover-transition-speed`)}
  "
    onclick={() => {
      switch (kind) {
        case "close": CURRENT_WINDOW.close(); break;
        case "restore": CURRENT_WINDOW.unmaximize(); break;
        case "maximize": CURRENT_WINDOW.maximize(); break;
        case "minimize": CURRENT_WINDOW.minimize(); break;
      }
    }}
  >
    <p>{CHARACTER_MAPPING[kind]}</p>
  </button>
{/snippet}

<main use:dragRegion>
  <div use:dragRegion class="contentsWrapper">
    {@render contents()}
  </div>
  <div class="actionItems">
    {@render button("minimize")}
    {#if maximized.value}
      {@render button("restore")}
    {:else}
      {@render button("maximize")}
    {/if}
    {@render button("close")}
  </div>
</main>

<style>
  .contentsWrapper {
    flex-grow: 1;
  }

  button:hover {
    background-color: var(--bg-hover);
    color: var(--icon-hover);
  }

  button:active {
    background-color: var(--bg-active);
    color: var(--icon-active);
    transition: unset;
  }

  button {
    width: 46px;
    display: flex;
    justify-content: center;
    align-items: center;
    background-color: var(--bg);
    color: var(--icon);
    border: 0;
    transition:
      background-color var(--hover-transition-speed) ease-out,
      color var(--hover-transition-speed) ease-out;
  }

  button > p {
    font-size: 10px;
  }

  p { 
    margin: 0;
  }

  main, .actionItems {
    display: flex;
    flex-direction: row;
  }

  .actionItems {
    gap: 1px;
  }

  main {
    justify-content: space-between;
    min-height: 30px;

    -webkit-user-select: none; /* Safari */
    -ms-user-select: none; /* IE 10 and IE 11 */
    user-select: none; /* Standard syntax */
  }
</style>
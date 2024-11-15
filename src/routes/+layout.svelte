<script lang="ts">
  import "../app.css";
  import { DefaultTheme, ThemeOverride, type Theme } from "$lib/theme";
  import WindowsTitlebar from "$component/windows_titlebar.svelte";
  import { CURRENT_WINDOW, decorated, dragRegion, focused } from "$lib/platform/index.svelte";
  import * as OS from "@tauri-apps/plugin-os";
  import ArrowLeftIntoBarIcon from "$icon/arrow_left_into_bar_icon.svelte";
  import ArrowRightIntoBarIcon from "$icon/arrow_right_into_bar_icon.svelte";
  import { REM_SIZE } from "$lib/rem_size.svelte";

  const DEFAULT_THEME = new DefaultTheme;
  const DEFOCUSED_THEME = new ThemeOverride(DEFAULT_THEME, {
    // background: "#111",
    // "titlebar-windows-minimize-icon": "#9d9d9d",
    // "titlebar-windows-minimize-hover-icon": "#9d9d9d",
    // "titlebar-windows-maximize-icon": "#9d9d9d",
    // "titlebar-windows-maximize-hover-icon": "#9d9d9d",
    // "titlebar-windows-restore-icon": "#9d9d9d",
    // "titlebar-windows-restore-hover-icon": "#9d9d9d",
    // "titlebar-windows-close-icon": "#9d9d9d",
    // "titlebar-windows-close-hover-icon": "#9d9d9d",
  })

  let theme: Theme = $derived(focused.value ? DEFAULT_THEME : DEFOCUSED_THEME);

  let style: string = $derived.by(() => {
    let x = ""

    for (let [key, value] of theme.iter_keys()) {
      console.log([key, value]);

      x += `--theme-${key}: ${value};`
    }

    return x;
  });

  const { children } = $props();
  const os = OS.type();
  const decorationsAbove = $derived(os === "macos" || decorated.value);

  const sidebar_base_min_width = $derived((REM_SIZE.value * 5.633333) + 16);

  const DEFAULT_WIDTH = 300;
  
  // svelte-ignore state_referenced_locally we don't want it to update. we're initializing it.
  let sidebar_min_width_override: number | undefined = $state(undefined)
  let sidebar_min_width = $derived(sidebar_min_width_override ? sidebar_min_width_override : sidebar_base_min_width);
  let desired_sidebar_width: number | "minimum" = $state(DEFAULT_WIDTH);
  let previous_user_set_sidebar_width: number | "minimum" = $state(DEFAULT_WIDTH);
  let is_resizing = $state(false);

  function resolve_width(is_resizing: boolean, desired_width: number | "minimum", min_width: number) {
    if (is_resizing)
      return typeof desired_width === "number" ? desired_width : min_width;

    if ((typeof desired_width === "number" && desired_width >= min_width) || desired_width === "minimum")
      return typeof desired_width === "number" ? desired_width : min_width;

    if (desired_width < (min_width / 2))
      return 8
    else
      return min_width;
  }

  let sidebar_width_px = $derived(resolve_width(is_resizing, desired_sidebar_width, sidebar_min_width));
  let is_closed = $derived(sidebar_width_px <= 8);

  function mouseDown(e: MouseEvent) {
    e.preventDefault();

    sidebar_min_width_override = undefined;

    desired_sidebar_width = Math.max(e.clientX, 8)
    is_resizing = true;
  }
  
  function mouseUp(e: MouseEvent) {
    is_resizing = false;

    if (typeof desired_sidebar_width === "string" || desired_sidebar_width < sidebar_min_width) {
      if (typeof desired_sidebar_width === "number" && desired_sidebar_width < (sidebar_min_width / 2)) {
        desired_sidebar_width = 8;
      } else {
        desired_sidebar_width = "minimum";
        previous_user_set_sidebar_width = "minimum";
      }
    } else {
      previous_user_set_sidebar_width = desired_sidebar_width
    }
  }

  function mouseMove(e: MouseEvent) {
    if (!is_resizing)
      return;

    desired_sidebar_width = Math.max(e.clientX, 8);
  }

  let minimized = $derived.by(() => {
    let sidebar_width_marks_minimized = sidebar_width_px <= (REM_SIZE.value * 8.6333333);
    let previous_user_set_sidebar_width_px = previous_user_set_sidebar_width == "minimum" ? sidebar_min_width : previous_user_set_sidebar_width;
    let previous_users_set_sidebar_width_marks_minimized = previous_user_set_sidebar_width_px <= (REM_SIZE.value * 8.6333333);

    if (is_resizing) {
      return sidebar_width_marks_minimized;
    } else {
      return previous_users_set_sidebar_width_marks_minimized;
    }
  })

  $inspect(desired_sidebar_width, sidebar_width_px)
</script>

<svelte:window onmouseup={mouseUp} onmousemove={mouseMove} />

{#snippet TitlebarContents()}
  <div use:dragRegion class="titlebarContents">
    <button class="collapse-button" onclick={() => {
      if (sidebar_width_px < sidebar_min_width) {
        desired_sidebar_width = previous_user_set_sidebar_width;
      } else {
        sidebar_min_width_override = sidebar_width_px;
        desired_sidebar_width = 8;
      }
    } }>
      {#if is_closed}
        <ArrowRightIntoBarIcon size="20px" color="#cccccc" />
      {:else}
        <ArrowLeftIntoBarIcon size="20px" color="#cccccc" />
      {/if}
    </button>
  </div>
{/snippet}

<div id="app" class="app" class:inactive={!focused.value} {style} style:--sidebar_width="{sidebar_width_px}px" style:--sidebar_min_width="{sidebar_min_width}px" style:--is_closed="{is_closed ? 1 : 0}">
  <div class="sidebar_wrap" class:animate={!is_resizing}>
    <div class="sidebar">
    </div>
  </div>

  <div class="right">
    <div class="titlebar">
      {#if !decorationsAbove}
        {#if os === "windows"}
          <WindowsTitlebar contents={TitlebarContents} />
        {/if}
      {:else}
        {@render TitlebarContents()}
      {/if}
    </div>
  
    <div class="contents_wrap">
      <div role="none" class="sidebar_handle" style:--sidebar_width="{sidebar_width_px}px" onmousedown={mouseDown}>
        <div class="sidebar_handle_view"></div>
      </div>

      <div class="contents">
        {@render children()}
      </div>

      <div class="sidebar_gradient_wrap">
        <div class="sidebar_gradient">
          
        </div>
      </div>
    </div>
  </div>
</div>

<style>
  #app:has(.sidebar_handle:hover) .contents {
    border-top-left-radius: 0 !important;
  }

  .sidebar_gradient_wrap {
    position: absolute;
    left: 0;
    width: var(--sidebar_min_width);
    top: -100vh;
    height: 200vh;
    overflow: hidden;
  }

  .sidebar_gradient {
    height: 200vh;
    position: absolute;
    transition: left 0.25s ease-out;
    left: calc(var(--is_closed) * calc(-1 * calc(var(--sidebar_min_width) * 0.2)));
    width: calc(var(--sidebar_min_width) * 2);
    cursor: ew-resize;
    background: linear-gradient(90deg, transparent 0%, var(--theme-background) 10%);
    z-index: 3;
  }

  .sidebar_wrap.animate {
    transition: width 0.2s ease-out;
  }

  .sidebar_handle {
    position: absolute;
    height: 100vh;
    left: -8px;
    width: 16px;
    z-index: 100;
    cursor: ew-resize;
  }

  .sidebar_handle_view {
    position: absolute;
    height: calc(100vh - 30px);
    left: 6px;
    width: 4px;
    border-top-left-radius: 100px;
    z-index: 100;
    transition: background-color 0.1s ease-out;
    transition-delay: 0.1s;
  }

  .sidebar_handle:hover .sidebar_handle_view {
    background-color: #0284c7;
  }

  .collapse-button {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 150%;
    width: 46px;
    z-index: 4;
    background-color: transparent;
    border: none;
    transition: background-color var(--theme-titlebar-windows-minimize-hover-transition-speed) ease-out;
  }

  .collapse-button:hover {
    background-color: var(--theme-titlebar-windows-minimize-hover-background);
  }

  .titlebarContents {
    display: flex;
    height: 100%;
    align-items: center;
  }

  .titlebar {
    z-index: 4;
  }

  .app {
    display: flex;
    flex-direction: row;
    background-color: var(--theme-background);
    width: 100vw;
    height: 100vh;
    overflow: hidden;
  }

  .right {
    display: flex;
    flex-direction: column;
    flex-grow: 1;
  }
  
  .sidebar {
    width: 100%;
    min-width: var(--sidebar_min_width);
    height: 100%;
  }

  .sidebar_wrap {
    width: var(--sidebar_width);
    display: flex;
    flex-direction: column;
    user-select: none;
    -webkit-user-select: none;
  }

  .contents_wrap {
    flex-grow: 1;
    display: flex;
    position: relative;
  }

  .contents {
    display: flex;
    flex-direction: row;
    z-index: 6;
    flex-grow: 1;
    background-color: var(--theme-foreground);
    border-top-left-radius: 8px;
    transition: border-top-left-radius 0s;
    transition-delay: 0.1s;
    overflow: hidden;
    box-shadow: rgba(0, 0, 0, 0.5) 0 0 20px;
  }

  :global(html), :global(body), .app {
    margin: 0;
    min-height: 100vh;
    overflow: hidden;
  }
</style>

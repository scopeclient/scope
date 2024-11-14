import { getCurrentWindow } from "@tauri-apps/api/window"

export function dragRegion(element: HTMLElement) {
  element.dataset.tauriDragRegion = ""
}

export const CURRENT_WINDOW = getCurrentWindow();

CURRENT_WINDOW.onFocusChanged(focus_change => {
  focused.value = focus_change.payload;
})

CURRENT_WINDOW.isFocused().then(f => focused.value = f);
export let focused = $state({ value: false });

CURRENT_WINDOW.isDecorated().then(d => decorated.value = d);
export let decorated = $state({ value: false });

CURRENT_WINDOW.onResized(async _ => {
  maximized.value = await CURRENT_WINDOW.isMaximized();

  console.log("Maximized", maximized.value);
})

CURRENT_WINDOW.isMaximized().then(m => maximized.value = m);
export let maximized = $state({ value: false });
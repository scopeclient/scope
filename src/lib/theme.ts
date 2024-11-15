export function theme(key: string) {
  key = key.replace(/[A-Z]/g, m => "-" + m.toLowerCase());

  return `var(--theme-${key})`
}

export interface Theme {
  iter_keys(): Generator<[string, string]>
}

export class ThemeOverride implements Theme {
  constructor(private base: Theme, private overrides: Record<string, string>) {}

  *iter_keys() {
    console.log(this.base);

    for (let [key, value] of this.base.iter_keys()) {
      if (key in this.overrides) {
        yield [key, this.overrides[key]] as [string, string];
        continue;
      }

      yield [key, value] as [string, string];
    }
  }
}

export class DefaultTheme implements Theme {
  *iter_keys() {
    yield* Object.entries({
      "background": "#1a191c",
      "foreground": "#25272b",

      "titlebar-windows-minimize-icon": "#cccccc",
      "titlebar-windows-minimize-hover-background": "#373737",
      "titlebar-windows-minimize-hover-icon": "#cccccc",
      "titlebar-windows-minimize-hover-transition-speed": "66ms",
      "titlebar-windows-minimize-active-background": "#545454",
      "titlebar-windows-minimize-active-icon": "#cccccc",
      
      "titlebar-windows-maximize-icon": "#cccccc",
      "titlebar-windows-maximize-hover-background": "#373737",
      "titlebar-windows-maximize-hover-icon": "#cccccc",
      "titlebar-windows-maximize-hover-transition-speed": "66ms",
      "titlebar-windows-maximize-active-background": "#545454",
      "titlebar-windows-maximize-active-icon": "#cccccc",
      
      "titlebar-windows-restore-icon": "#cccccc",
      "titlebar-windows-restore-hover-background": "#373737",
      "titlebar-windows-restore-hover-icon": "#cccccc",
      "titlebar-windows-restore-hover-transition-speed": "66ms",
      "titlebar-windows-restore-active-background": "#545454",
      "titlebar-windows-restore-active-icon": "#cccccc",
      
      "titlebar-windows-close-icon": "#cccccc",
      "titlebar-windows-close-hover-background": "#e81123",
      "titlebar-windows-close-hover-icon": "#ffffff",
      "titlebar-windows-close-hover-transition-speed": "66ms",
      "titlebar-windows-close-active-background": "#94141e",
      "titlebar-windows-close-active-icon": "#ffffff",
    });
  }
}
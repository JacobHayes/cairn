// The `localStorage` key of the viewer's theme choice. Two places read it: ui/theme.ts, and the
// inline script in index.html that applies the choice before first paint (vite.config.ts writes
// this value into the page), so the key is written here only.
export const THEME_KEY = "cairn:theme";

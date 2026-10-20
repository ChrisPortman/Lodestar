// Light/dark theme: follows the OS until the person picks one, which this
// browser then remembers. public/theme.js applies the same rule before first
// paint; keep the two in step.
import { ref } from 'vue'

export type Theme = 'light' | 'dark'

export const STORAGE_KEY = 'lodestar-theme'
export const DARK_QUERY = '(prefers-color-scheme: dark)'

function stored(): Theme | null {
  try {
    const value = localStorage.getItem(STORAGE_KEY)
    return value === 'light' || value === 'dark' ? value : null
  } catch {
    return null
  }
}

function resolve(): Theme {
  return stored() ?? (window.matchMedia?.(DARK_QUERY).matches ? 'dark' : 'light')
}

function apply(theme: Theme) {
  document.documentElement.dataset.theme = theme
}

/** The current theme, plus a toggle that remembers the choice. */
export function useTheme() {
  const theme = ref<Theme>(resolve())
  apply(theme.value)

  function toggle() {
    theme.value = theme.value === 'dark' ? 'light' : 'dark'
    apply(theme.value)
    try {
      localStorage.setItem(STORAGE_KEY, theme.value)
    } catch {
      // Storage blocked: the choice lasts for this page only.
    }
  }

  return { theme, toggle }
}

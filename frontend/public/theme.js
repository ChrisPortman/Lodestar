// Runs before the stylesheet paints, so dark mode never flashes light (#54).
// A classic external script, because the CSP forbids inline scripts.
// Keep in step with src/theme.ts.
;(function () {
  var stored = null
  try {
    stored = localStorage.getItem('lodestar-theme')
  } catch {
    // Storage blocked: fall back to the OS preference.
  }
  var dark =
    stored === 'dark' ||
    (stored !== 'light' && window.matchMedia('(prefers-color-scheme: dark)').matches)
  document.documentElement.dataset.theme = dark ? 'dark' : 'light'
})()

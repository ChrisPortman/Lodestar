# Theme tokens, dark-mode toggle, self-hosted fonts and print

Research for #47 (parent #45). Question: how Tailwind v4 semantic colour tokens, a dark-mode toggle that respects `prefers-color-scheme` and persists a manual choice, `@fontsource` fonts and print CSS fit together in the Vue 3 frontend.

## 1. Semantic colour tokens

- `@theme` variables generate utilities, so `--color-surface` gives you `bg-surface`, `text-surface` and the rest. Tailwind also emits them as real CSS variables. Use `:root` for variables that should not get utilities. `@theme` must sit at the top level, not inside a selector or media query. ([Tailwind: Theme variables](https://tailwindcss.com/docs/theme))
- Because `@theme` cannot be nested, the light and dark *values* go on plain selectors (`:root`, `[data-theme=dark]`). `@theme inline` then maps the semantic tokens onto them. With `inline`, "the utility class will use the theme variable *value* instead of referencing the actual theme variable", which avoids resolving the variable at the wrong element. (same source)

Recommended shape for `frontend/src/style.css`:

```css
@import 'tailwindcss';
@custom-variant dark (&:where([data-theme=dark], [data-theme=dark] *));

:root {
  --surface: oklch(0.99 0 0);
  --text: oklch(0.21 0 0);
  --accent: oklch(0.55 0.15 250);
  color-scheme: light;
}
[data-theme='dark'] {
  --surface: oklch(0.2 0 0);
  --text: oklch(0.93 0 0);
  --accent: oklch(0.72 0.13 250);
  color-scheme: dark;
}

@theme inline {
  --color-surface: var(--surface);
  --color-text: var(--text);
  --color-accent: var(--accent);
  --font-sans: 'Inter Variable', ui-sans-serif, system-ui, sans-serif;
}
```

Components then use `bg-surface text-text` and never need `dark:`. Keep `dark:` for the rare one-off.

## 2. Toggle without a flash

- By default `dark:` follows `prefers-color-scheme`. To switch manually, Tailwind says to override the variant with `@custom-variant dark (&:where([data-theme=dark], [data-theme=dark] *));`. ([Tailwind: Dark mode](https://tailwindcss.com/docs/dark-mode))
- Tailwind's recommended three-state pattern (light, dark, system) is shown below. Put it "inline in `head` to avoid FOUC", so it runs before first paint:

```html
<script>
  document.documentElement.dataset.theme =
    localStorage.theme === 'dark' ||
    (!('theme' in localStorage) && matchMedia('(prefers-color-scheme: dark)').matches)
      ? 'dark' : 'light';
</script>
```

  Choosing explicitly sets `localStorage.theme` to `'light'` or `'dark'`. Choosing "system" calls `removeItem('theme')`. (same source)
- Implications for Lodestar:
  - The script must go inline in `frontend/index.html`, not in a Vue module, because modules run after paint.
  - When "system" is selected, the Vue composable should also listen to `matchMedia(...).addEventListener('change', …)`.
  - **CSP:** if the HTTP security headers set a strict `script-src`, this inline script needs a hash (`'sha256-…'`) in the policy. The other option is a tiny same-origin, render-blocking (non-module) `<script src>`. Check against the headers the backend sets.

## 3. Self-hosting fonts with @fontsource

- Install the variable package, `@fontsource-variable/<name>`, and import it once from `main.ts` (`import '@fontsource-variable/inter'`). The CSS family name gets a ` Variable` suffix, e.g. `'Inter Variable'`. Only use one import per font, because "multiple imports of different axes are not supported". `/wght-italic.css` adds italics. ([Fontsource: Variable fonts](https://fontsource.org/docs/getting-started/variable))
- Subsetting: each subset is its own `@font-face` with a `unicode-range`, so "browsers fetch only the character sets used on the page" (same source). For an English UI only the `latin` file downloads, so you do not need manual subsetting. Vite fingerprints the `.woff2` files and emits them as build assets, so the embedded-frontend setup serves them with no extra config. Verified in the 5.3.x tarballs: `font-display: swap`, 7 subset faces for Inter.
- Measured size of the Latin, upright, full-weight-range woff2 (from `npm pack`, latest versions, all OFL-1.1):

| Package | Version | latin wght woff2 |
|---|---|---|
| `@fontsource-variable/inter` | 5.3.0 | 48 KB |
| `@fontsource-variable/ibm-plex-sans` | 5.3.0 | 46 KB |
| `@fontsource-variable/source-sans-3` | 5.3.1 | 29 KB |
| `@fontsource-variable/source-serif-4` | 5.3.1 | 51 KB |

  Adding italics roughly doubles each figure. A sans plus a serif costs about 100 KB, cached once.

## 4. Candidate families

- **Inter** (UI sans, recommended): designed for screens, with tabular figures through `font-variant-numeric: tabular-nums` (Tailwind `tabular-nums`) for aligned numbers in dense tables. Prints cleanly.
- **IBM Plex Sans**: slightly narrower, more character. Pairs with Plex Mono/Serif if a family system is wanted.
- **Source Sans 3**: the lightest download. Compact, and good for dense tables.
- **Source Serif 4** (optional, for Project on a Page): has an optical-size axis and reads well at print sizes. Only worth the extra 51 KB if the printed page should look different from the UI. Otherwise Inter alone is enough.

(These are design judgements, not sourced facts. Sizes and licences come from the npm tarballs.)

## 5. Print

- The default `print-color-adjust: economy` lets browsers drop backgrounds and darken light text for paper. The user's "Print backgrounds" setting overrides the property, and `exact` only *asks* the browser to keep colours. ([MDN: print-color-adjust](https://developer.mozilla.org/en-US/docs/Web/CSS/print-color-adjust))
- So don't depend on backgrounds at all. Force the light palette and white paper in print, and carry meaning in text, borders or patterns, never in fill colour alone:

```css
@media print {
  :root, [data-theme='dark'] {
    --surface: white; --text: black; color-scheme: light;
  }
}
```

  Use Tailwind's built-in `print:` variant to hide navigation and controls (`print:hidden`).
- Greyscale: status colours (e.g. Health Rating) need distinct *lightness*, not just hue. Check the oklch L values differ by about 0.15 or more. Always pair colour with a label or icon. If a coloured chip must print, add `print-color-adjust: exact` to that element only and accept that the user setting can still drop it.
- The existing `@page { size: A4 portrait; margin: 12mm; }` in `style.css` stays as it is.

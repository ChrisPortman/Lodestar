// Item cards (#50): the Projects list and the Actions, Blockers and Risks
// panels show each item as a small card with a 3px state accent on the left.

export type Accent = 'red' | 'amber'

export const CARD = 'rounded-md border border-l-3 border-border bg-surface px-4 py-3'

const ACCENT_CLASS: Record<Accent, string> = {
  red: 'border-l-st-red-solid',
  amber: 'border-l-st-amber-solid',
}

/** The left border for an accent, neutral when there is none. */
export function accentClass(accent: Accent | undefined) {
  return accent ? ACCENT_CLASS[accent] : 'border-l-border'
}

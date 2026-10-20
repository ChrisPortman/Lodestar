// How each status reads on screen and in print (#48, #51): a symbol and a
// word as well as a colour, so it never relies on colour alone.
import type { HealthRating } from './api/projects'
import type { Severity } from './risks'

export type Tone = 'red' | 'amber' | 'green' | 'none'

export interface StatusLook {
  symbol: string
  label: string
  tone: Tone
}

export type RatingChoice = HealthRating['rating'] | 'none'

export const RATING: Record<RatingChoice, StatusLook> = {
  red: { symbol: '▲', label: 'Red', tone: 'red' },
  amber: { symbol: '●', label: 'Amber', tone: 'amber' },
  green: { symbol: '✓', label: 'Green', tone: 'green' },
  none: { symbol: '—', label: 'Not rated', tone: 'none' },
}

export const SEVERITY: Record<Severity, StatusLook> = {
  low: { symbol: '✓', label: 'Low', tone: 'green' },
  medium: { symbol: '●', label: 'Medium', tone: 'amber' },
  high: { symbol: '▲', label: 'High', tone: 'red' },
}

/** A filled chip in the tone's colours. */
export const TONE_CHIP: Record<Tone, string> = {
  red: 'bg-st-red-bg text-st-red-fg',
  amber: 'bg-st-amber-bg text-st-amber-fg',
  green: 'bg-st-green-bg text-st-green-fg',
  none: 'bg-active text-muted',
}

/** Text alone in the tone's colour, e.g. for print. */
export const TONE_TEXT: Record<Tone, string> = {
  red: 'text-st-red-fg',
  amber: 'text-st-amber-fg',
  green: 'text-st-green-fg',
  none: 'text-muted',
}

/** "▲ Red", "— Not rated", ... */
export const tag = (look: StatusLook) => `${look.symbol} ${look.label}`

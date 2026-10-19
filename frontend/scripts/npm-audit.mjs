// `npm audit --audit-level=high` with a committed ignore list (NFR-33).
// audit-ignore.json maps advisory IDs (GHSA-...) to the reason they are accepted.
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'

const ignore = JSON.parse(readFileSync(new URL('../audit-ignore.json', import.meta.url)))
for (const [id, reason] of Object.entries(ignore)) {
  if (typeof reason !== 'string' || reason.trim() === '') {
    console.error(`audit-ignore.json: ${id} has no reason`)
    process.exit(1)
  }
}

let out
try {
  out = execFileSync('npm', ['audit', '--json'], { encoding: 'utf8' })
} catch (e) {
  out = e.stdout // npm audit exits non-zero when it finds anything
}
const { vulnerabilities = {} } = JSON.parse(out)

const failing = new Set()
const used = new Set()
for (const vuln of Object.values(vulnerabilities)) {
  for (const via of vuln.via) {
    if (typeof via !== 'object' || !['high', 'critical'].includes(via.severity)) continue
    const id = via.url.split('/').pop()
    if (id in ignore) used.add(id)
    else failing.add(`${id} (${via.severity}) ${via.name}: ${via.title}`)
  }
}

for (const id of Object.keys(ignore)) {
  if (!used.has(id)) console.warn(`audit-ignore.json: ${id} no longer matches; remove it`)
}
if (failing.size > 0) {
  console.error('High/critical advisories:\n' + [...failing].join('\n'))
  process.exit(1)
}
console.log(`npm audit: no unignored high/critical advisories (${used.size} ignored)`)

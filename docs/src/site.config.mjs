// One place for the facts that appear all over the docs. Markdown pages use the {{tokens}} below
// (see src/plugins/tokens.mjs), so changing the repo link or the version is a one-line edit here.
import metrics from './data/metrics.json' with { type: 'json' }

const repoUrl = 'https://github.com/rmalcomber/dev_browser'

export const site = {
  name: 'The Dev Browser',
  tagline: 'A personal developer browser with workspaces, a built-in launcher and tools',
  // GitHub Pages project site; override with DOCS_BASE=/ for a root deployment (custom domain, Docker).
  site: process.env.DOCS_SITE ?? 'https://rmalcomber.github.io',
  base: process.env.DOCS_BASE ?? '/dev_browser',
  repoUrl,
  releasesUrl: `${repoUrl}/releases`,
  // Keep in step with the project's real version (Cargo.toml, package.json, git tag) when releasing.
  version: '0.1.0-alpha',
  // 'alpha' or 'beta' adds a badge to the release banner; use 'stable' once the project is 1.0.
  stage: 'alpha',
  support: {
    coffee: 'https://buymeacoffee.com/rmalcomber',
    sponsors: 'https://github.com/sponsors/rmalcomber',
  },
}

/** Values for {{repo}}, {{releases}}, {{version}}, {{name}} and {{metric.*}} in Markdown. */
export const tokenValues = {
  repo: site.repoUrl,
  releases: site.releasesUrl,
  version: site.version,
  name: site.name,
  ...Object.fromEntries(Object.entries(metrics).map(([k, v]) => [`metric.${k}`, v])),
}

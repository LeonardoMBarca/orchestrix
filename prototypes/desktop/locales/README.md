# Interface languages

The app starts in **English** and supports **Português (`pt-BR`)** and **Español (`es`)**. The public website remains in English.

Edit the JSON catalogs, then run `npm.cmd run locales`. Commit the generated `catalog.js` with its sources. `npm.cmd run check` verifies completeness, conflicting entries and generated output freshness.

Each trimmed Portuguese UI source is a key with `en` and `es` translations. Portuguese uses the source text. Shared navigation lives in `chrome.json`; other catalogs group app, experience, workspace and connection text. Keep shared translations consistent across files.

Use `t('UI source')` for a known interface label, `html` as a tag on UI templates, and `text` for interpolated system copy. Tagged templates translate only static copy; their values stay intact. Continue escaping untrusted data before inserting it into HTML. Never translate a whole rendered DOM, user input, a repository path, code, a provider identifier or an account name.

Resolve changing UI labels when rendering. Create sample content and new demo messages in the current language; changing the interface does not rewrite existing messages, artifacts or project data. Keep status enums, action IDs and authorization conditions independent of language.

See [the product decision](../../../docs/design/interface-languages.md) and [locale tests](../tests/localization.spec.mjs).

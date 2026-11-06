# SQLBuild UI

The SQLBuild orchestration web UI: a SvelteKit single-page app built to static
assets (`adapter-static`, SPA fallback, no SSR).

This is an early, preliminary version. Pages read from a typed data layer in
`src/lib/api/`; until the server API exists, that layer is backed by an
in-memory mock client with neutral example data.

## Develop

```sh
npm ci
npm run dev
```

## Check and build

```sh
npm run check
npm run build   # writes static assets to build/
```

## Generated files

The run panel's verbs and flags come from the CLI's command table
(`crates/sqlbuild-cli/src/argv/command_table.json`). After changing CLI flags,
regenerate the extract; `check:command-table` fails when it is stale.

```sh
npm run gen:command-table
npm run check:command-table
```

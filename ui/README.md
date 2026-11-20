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

## Copy

UI text follows the style guide in [COPY.md](COPY.md); a unit test checks it
against a baseline that may only shrink.

## Performance

Pages, lists and graphs follow the budgets and rules in [PERF.md](PERF.md); the end-to-end
performance tests enforce them with the CPU slowed 6×.

## Generated files

The run panel's verbs and flags come from the CLI's command table
(`crates/sqlbuild-cli/src/argv/command_table.json`). After changing CLI flags,
regenerate the extract; `check:command-table` fails when it is stale.

```sh
npm run gen:command-table
npm run check:command-table
```

## Static demo

`sqlbuild.com/demo` is this UI with no server behind it: the server client replays
`demo/recording.json.gz`, a recording of DuckShop on `sqb dev`, and refuses every write.
The website workflow builds it:

```sh
SQB_UI_BASE=/demo VITE_SQB_DEMO=1 npm run build
node scripts/demo-pages.mjs build demo/recording.json.gz ../website/dist/demo
```

To record it again, run DuckShop under `sqb dev`, click through the pages with the
browser saving a HAR, then ask the server for every read the pages made:

```sh
node scripts/record-demo.mjs demo/recording.json.gz --server http://127.0.0.1:<port> \
  --replace <project path>=/home/demo/duckshop --replace <location name>=duckshop session.har
```

To record only the plan previews again, through the server's plan summary, rows and diff reads,
keep the rest of the recording and plan each recorded command anew on `sqb dev`:

```sh
node scripts/record-demo-plans.mjs demo/recording.json.gz --server http://127.0.0.1:<port> \
  --replace <project path>=/home/demo/duckshop --ask job:marts_build=job:marts_build_job
```

`--ask` rewrites only the command sent to the server, for a job renamed since the recording.

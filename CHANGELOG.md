# Changelog

## [0.205.1](https://github.com/sqlstack-io/sqlbuild/compare/v0.205.0...v0.205.1) (2026-11-09)


### Performance Improvements

* stop mimalloc asking for transparent huge pages ([#1391](https://github.com/sqlstack-io/sqlbuild/issues/1391)) ([eebb756](https://github.com/sqlstack-io/sqlbuild/commit/eebb75664c654fe4809130f6abc51f27dbcfe087))

## [0.205.0](https://github.com/sqlstack-io/sqlbuild/compare/v0.204.0...v0.205.0) (2026-11-09)


### Features

* **jobs:** the job page shows a job's settings, its group's members in run order, the groups it belongs to and its critical nodes, and can start a full refresh for people with run.rewrite_history ([c0971a2](https://github.com/sqlstack-io/sqlbuild/commit/c0971a2c49b3cdacc1496d9bc8ba3c6bba35b7b9))
* **runs:** the run page shows a failed node's error text and project file, redacted and limited to 2,000 characters, for people with view.evidence on that node. Upgrade the server before its workers: an older server refuses a newer worker's failed-node report ([c0971a2](https://github.com/sqlstack-io/sqlbuild/commit/c0971a2c49b3cdacc1496d9bc8ba3c6bba35b7b9))
* **ui:** backfill a job or group over a cursor window from its launch form ([757a977](https://github.com/sqlstack-io/sqlbuild/commit/757a977a0265a8359f186faa93be4c71f4cf4f0e))
* **ui:** job pages draw their nodes as a lineage graph; a group draws each member job as a box around its nodes, with its last run ([57f4cff](https://github.com/sqlstack-io/sqlbuild/commit/57f4cff5138f34529165450c2eb2649380eb3fed))
* **ui:** location detail and scheduler status from the server's health reads ([757a977](https://github.com/sqlstack-io/sqlbuild/commit/757a977a0265a8359f186faa93be4c71f4cf4f0e))
* **ui:** run header shows the start time, project version and sqb command; a group run's log and statements gather its job runs' ([57f4cff](https://github.com/sqlstack-io/sqlbuild/commit/57f4cff5138f34529165450c2eb2649380eb3fed))
* **ui:** run page shows the first failed node's error inline, orders the timeline by start, and merges events and output into one filterable log. Opening a run with a failed node now records one error.read audit row for people with view.evidence ([57f4cff](https://github.com/sqlstack-io/sqlbuild/commit/57f4cff5138f34529165450c2eb2649380eb3fed))
* **ui:** upcoming schedule times and calendar coverage on the trigger page ([757a977](https://github.com/sqlstack-io/sqlbuild/commit/757a977a0265a8359f186faa93be4c71f4cf4f0e))


### Bug Fixes

* **redaction:** diagnostic and error-text redaction now also covers keys glued to a prefix (such as PGPASSWORD), SQL PASSWORD and IDENTIFIED [WITH plugin] BY values, passwords containing @, and digit passwords in cut connection strings ([c0971a2](https://github.com/sqlstack-io/sqlbuild/commit/c0971a2c49b3cdacc1496d9bc8ba3c6bba35b7b9))
* **runs:** runs record whether they rewrite history (backfills and full refreshes), and the job page no longer shows deferral paths ([c0971a2](https://github.com/sqlstack-io/sqlbuild/commit/c0971a2c49b3cdacc1496d9bc8ba3c6bba35b7b9))
* **server:** the maintenance read returns how many grants were copied to an old-name view, not the GRANT statements ([757a977](https://github.com/sqlstack-io/sqlbuild/commit/757a977a0265a8359f186faa93be4c71f4cf4f0e))
* **server:** the server no longer sends framework error text that names internal types; a plain-text 5xx becomes the generic JSON error ([c0971a2](https://github.com/sqlstack-io/sqlbuild/commit/c0971a2c49b3cdacc1496d9bc8ba3c6bba35b7b9))
* **ui:** a run's origin chip links to its schedule or sensor, or to the run it reruns ([57f4cff](https://github.com/sqlstack-io/sqlbuild/commit/57f4cff5138f34529165450c2eb2649380eb3fed))
* **ui:** group runs list each node once, and a job run's statement SQL loads from its own run ([57f4cff](https://github.com/sqlstack-io/sqlbuild/commit/57f4cff5138f34529165450c2eb2649380eb3fed))
* **ui:** lineage inspector switches nodes without a loading flash, and its header always names the selected node ([57f4cff](https://github.com/sqlstack-io/sqlbuild/commit/57f4cff5138f34529165450c2eb2649380eb3fed))
* **ui:** run statements name their statement kind, and run-level queries read as "run" ([57f4cff](https://github.com/sqlstack-io/sqlbuild/commit/57f4cff5138f34529165450c2eb2649380eb3fed))
* **ui:** top bar popovers such as Run now stay above page graphs ([57f4cff](https://github.com/sqlstack-io/sqlbuild/commit/57f4cff5138f34529165450c2eb2649380eb3fed))
* **worker:** workers stop with a clear message when an older server refuses their report. Upgrade the server before its locations and workers ([c0971a2](https://github.com/sqlstack-io/sqlbuild/commit/c0971a2c49b3cdacc1496d9bc8ba3c6bba35b7b9))

## [0.204.0](https://github.com/sqlstack-io/sqlbuild/compare/v0.203.0...v0.204.0) (2026-11-09)


### Features

* publish column lineage and show it on catalog node pages ([4070f12](https://github.com/sqlstack-io/sqlbuild/commit/4070f12355b3864e8e93de1ab467da2f87fa0798))
* run test, seed, audit and freshness ad-hoc commands on workers ([4070f12](https://github.com/sqlstack-io/sqlbuild/commit/4070f12355b3864e8e93de1ab467da2f87fa0798))
* **ui:** give and remove roles and disable people from the admin pages ([4070f12](https://github.com/sqlstack-io/sqlbuild/commit/4070f12355b3864e8e93de1ab467da2f87fa0798))
* **ui:** show renames, old names and clean-ups on the Maintenance page ([4070f12](https://github.com/sqlstack-io/sqlbuild/commit/4070f12355b3864e8e93de1ab467da2f87fa0798))
* **ui:** show the Overview from live runs, checks, models and deploys ([4070f12](https://github.com/sqlstack-io/sqlbuild/commit/4070f12355b3864e8e93de1ab467da2f87fa0798))
* **ui:** tell a partial viewer why the Overview is empty ([4070f12](https://github.com/sqlstack-io/sqlbuild/commit/4070f12355b3864e8e93de1ab467da2f87fa0798))
* **ui:** test a location's warehouse connection from the Locations page ([4070f12](https://github.com/sqlstack-io/sqlbuild/commit/4070f12355b3864e8e93de1ab467da2f87fa0798))


### Bug Fixes

* run every ad-hoc verb from the worker's verified project ([4070f12](https://github.com/sqlstack-io/sqlbuild/commit/4070f12355b3864e8e93de1ab467da2f87fa0798))
* **ui:** show the approved logos and favicon from the website ([4070f12](https://github.com/sqlstack-io/sqlbuild/commit/4070f12355b3864e8e93de1ab467da2f87fa0798))

## [0.203.0](https://github.com/sqlstack-io/sqlbuild/compare/v0.202.0...v0.203.0) (2026-11-09)


### ⚠ BREAKING CHANGES

* require run.prioritise for job runs and reruns above priority 0

### Features

* **contracts:** prove dynamic pivots from SQLQuack parse trees ([7f41309](https://github.com/sqlstack-io/sqlbuild/commit/7f413098ec481f0d3d73fba5c0ae0384baaeb83f))
* **orchestration:** gzip location snapshot publishes ([7f41309](https://github.com/sqlstack-io/sqlbuild/commit/7f413098ec481f0d3d73fba5c0ae0384baaeb83f))
* parse rule facts, types and syntax trees with SQLQuack ([7f41309](https://github.com/sqlstack-io/sqlbuild/commit/7f413098ec481f0d3d73fba5c0ae0384baaeb83f))
* require run.prioritise for job runs and reruns above priority 0 ([7f41309](https://github.com/sqlstack-io/sqlbuild/commit/7f413098ec481f0d3d73fba5c0ae0384baaeb83f))
* run ad-hoc sqb commands from the web UI ([7f41309](https://github.com/sqlstack-io/sqlbuild/commit/7f413098ec481f0d3d73fba5c0ae0384baaeb83f))
* show run events, statements, SQL and output to people ([7f41309](https://github.com/sqlstack-io/sqlbuild/commit/7f413098ec481f0d3d73fba5c0ae0384baaeb83f))
* **ui:** wire the platform and admin pages to the control API ([7f41309](https://github.com/sqlstack-io/sqlbuild/commit/7f413098ec481f0d3d73fba5c0ae0384baaeb83f))


### Bug Fixes

* render SQL Server functions, schemas, merges and sources the engine accepts, and list warehouse Recall relations portably ([7f41309](https://github.com/sqlstack-io/sqlbuild/commit/7f413098ec481f0d3d73fba5c0ae0384baaeb83f))
* **ui:** stop a burst of control API requests on every live refresh ([7f41309](https://github.com/sqlstack-io/sqlbuild/commit/7f413098ec481f0d3d73fba5c0ae0384baaeb83f))

## [0.202.0](https://github.com/sqlstack-io/sqlbuild/compare/v0.201.0...v0.202.0) (2026-11-08)


### Features

* **ui:** show catalog node detail and finish the lineage pages ([#1377](https://github.com/sqlstack-io/sqlbuild/issues/1377)) ([0624bde](https://github.com/sqlstack-io/sqlbuild/commit/0624bded052898af855378a8f74afac04d60be9f))

## [0.201.0](https://github.com/sqlstack-io/sqlbuild/compare/v0.200.0...v0.201.0) (2026-11-08)


### Features

* **ui:** wire the lineage and changes pages to the live control API ([#1373](https://github.com/sqlstack-io/sqlbuild/issues/1373)) ([20c406a](https://github.com/sqlstack-io/sqlbuild/commit/20c406a4d09bc8d310b9a1537a618f77c12d9594))

## [0.200.0](https://github.com/sqlstack-io/sqlbuild/compare/v0.199.0...v0.200.0) (2026-11-08)


### Features

* **rules:** read custom-rule SQL syntax trees from SQLQuack parsers ([#1358](https://github.com/sqlstack-io/sqlbuild/issues/1358)) ([decd291](https://github.com/sqlstack-io/sqlbuild/commit/decd29162f9a47dd59c3690dd807d111e9937511))

## [0.199.0](https://github.com/sqlstack-io/sqlbuild/compare/v0.198.0...v0.199.0) (2026-11-08)


### Features

* **analysis:** route each dialect's SQL analysis to Polyglot or a native module ([#1354](https://github.com/sqlstack-io/sqlbuild/issues/1354)) ([97ab62c](https://github.com/sqlstack-io/sqlbuild/commit/97ab62c82a511e0fb79ff7f8626f3829a1845b80))


### Bug Fixes

* **ui:** wrap list toolbars instead of clipping their filters and counts ([#1349](https://github.com/sqlstack-io/sqlbuild/issues/1349)) ([702224e](https://github.com/sqlstack-io/sqlbuild/commit/702224e261e03f106e2a028acddedd3486e0939b))

## [0.198.0](https://github.com/sqlstack-io/sqlbuild/compare/v0.197.0...v0.198.0) (2026-11-07)


### ⚠ BREAKING CHANGES

* the first run after upgrading can plan some models as query_changed even though their SQL did not change. On DuckDB, Snowflake and SQLite this affects models that call an unquoted user-defined function, and on SQLite also models calling built-ins such as datetime, date or strftime in lower case; PostgreSQL models are unaffected. Plan that run: affected incremental and microbatch models whose replay_on_change is bounded or full replay or rebuild (even with full_refresh false), sqb build --changed reruns them, and models on the default forward policy only warn.

### Features

* compute query fingerprints from SQLQuack tokens ([#1348](https://github.com/sqlstack-io/sqlbuild/issues/1348)) ([1ee7c8f](https://github.com/sqlstack-io/sqlbuild/commit/1ee7c8f61bbc54fdd97b6b447b6577bba3fbcb6d))

## [0.197.0](https://github.com/sqlstack-io/sqlbuild/compare/v0.196.1...v0.197.0) (2026-11-07)


### Features

* **ui:** list the live catalog from the control API ([#1344](https://github.com/sqlstack-io/sqlbuild/issues/1344)) ([13b233f](https://github.com/sqlstack-io/sqlbuild/commit/13b233f61e35aa1982fc9dac6f53a8841485a00f))


### Bug Fixes

* **adapters:** escape cursor bounds and UDF packages as dialect string literals ([#1346](https://github.com/sqlstack-io/sqlbuild/issues/1346)) ([d3fc0e0](https://github.com/sqlstack-io/sqlbuild/commit/d3fc0e04908434bf758da2abc1ff864a695c1024))

## [0.196.1](https://github.com/sqlstack-io/sqlbuild/compare/v0.196.0...v0.196.1) (2026-11-07)


### Bug Fixes

* **bigquery:** render GoogleSQL column type changes and escapes ([#1341](https://github.com/sqlstack-io/sqlbuild/issues/1341)) ([18f2ac3](https://github.com/sqlstack-io/sqlbuild/commit/18f2ac3ed0edd93b599794640371227c6c324bfd))

## [0.196.0](https://github.com/sqlstack-io/sqlbuild/compare/v0.195.0...v0.196.0) (2026-11-07)


### Features

* **ui:** wire the checks pages to the live control API ([#1338](https://github.com/sqlstack-io/sqlbuild/issues/1338)) ([d7ebacb](https://github.com/sqlstack-io/sqlbuild/commit/d7ebacbccfa6b7ab4b9145471fc88ffecff0042a))

## [0.195.0](https://github.com/sqlstack-io/sqlbuild/compare/v0.194.0...v0.195.0) (2026-11-07)


### Features

* **ui:** wire the jobs and triggers pages to the live control API ([#1336](https://github.com/sqlstack-io/sqlbuild/issues/1336)) ([8b6a91c](https://github.com/sqlstack-io/sqlbuild/commit/8b6a91c474d0d064e2b0d30aaee744df29ef8c55))

## [0.194.0](https://github.com/sqlstack-io/sqlbuild/compare/v0.193.0...v0.194.0) (2026-11-07)


### Features

* **orchestration:** let people read run nodes, triggers and ticks ([#1332](https://github.com/sqlstack-io/sqlbuild/issues/1332)) ([fb13201](https://github.com/sqlstack-io/sqlbuild/commit/fb132010481633df2c8c5776f658380d5f265043))
* **ui:** wire the runs pages to the live control API ([#1334](https://github.com/sqlstack-io/sqlbuild/issues/1334)) ([68a1551](https://github.com/sqlstack-io/sqlbuild/commit/68a1551ce61e1c7010bfac472823e36aadfe1bc2))

## [0.193.0](https://github.com/sqlstack-io/sqlbuild/compare/v0.192.0...v0.193.0) (2026-11-07)


### Features

* **cli:** make sqlbuild a small forwarder to sqb ([#1333](https://github.com/sqlstack-io/sqlbuild/issues/1333)) ([821385d](https://github.com/sqlstack-io/sqlbuild/commit/821385d8af5bbc39ecf23f87f3493bc260882ab6))
* run the orchestration UI shell on the live control API ([#1328](https://github.com/sqlstack-io/sqlbuild/issues/1328)) ([cf07c40](https://github.com/sqlstack-io/sqlbuild/commit/cf07c4016ea4b8595dcc6d2d9e5dcfbb21021403))
* serve the web UI from sqb server and ship it in the wheels ([#1329](https://github.com/sqlstack-io/sqlbuild/issues/1329)) ([1345322](https://github.com/sqlstack-io/sqlbuild/commit/1345322e34957bb1fb7735fc44fc40e7fc86be5d))


### Bug Fixes

* let Recall's final delivery rows use the remaining shutdown budget ([#1325](https://github.com/sqlstack-io/sqlbuild/issues/1325)) ([04e98cb](https://github.com/sqlstack-io/sqlbuild/commit/04e98cbb4b2c3b2ac50024efd8ba025de9cefa71))
* publish audits that share a name as separate snapshot nodes ([#1326](https://github.com/sqlstack-io/sqlbuild/issues/1326)) ([8f0728e](https://github.com/sqlstack-io/sqlbuild/commit/8f0728ec45af3ff9c79cb8761d721e1ee02e6844))
* **security:** name no nodes in authorization denials ([#1322](https://github.com/sqlstack-io/sqlbuild/issues/1322)) ([8d2d2da](https://github.com/sqlstack-io/sqlbuild/commit/8d2d2da23f9b900a233c123fd2b2feae02aa937f))

## [0.192.0](https://github.com/sqlstack-io/sqlbuild/compare/v0.191.0...v0.192.0) (2026-11-06)


### Features

* add microbatch coverage endpoint ([#1319](https://github.com/sqlstack-io/sqlbuild/issues/1319)) ([d9abd83](https://github.com/sqlstack-io/sqlbuild/commit/d9abd8318654bcc3793e52c41f4e97b4e0a9ea73))

## [0.191.0](https://github.com/sqlstack-io/sqlbuild/compare/v0.190.0...v0.191.0) (2026-11-06)


### Features

* add maintenance history endpoint ([#1318](https://github.com/sqlstack-io/sqlbuild/issues/1318)) ([ec9a3b4](https://github.com/sqlstack-io/sqlbuild/commit/ec9a3b4651f97292454779110d301c6e8f8207e7))

## [0.190.0](https://github.com/sqlstack-io/sqlbuild/compare/v0.189.0...v0.190.0) (2026-11-06)


### Features

* read check evidence through the control API ([#1317](https://github.com/sqlstack-io/sqlbuild/issues/1317)) ([bf4a2a2](https://github.com/sqlstack-io/sqlbuild/commit/bf4a2a277e628b634da294855885c7d14784c266))

## [0.189.0](https://github.com/sqlstack-io/sqlbuild/compare/v0.188.0...v0.189.0) (2026-11-06)


### ⚠ BREAKING CHANGES

* control stores created by 0.188.x are not upgraded. The server refuses them with an error saying to recreate the control store; create a new one when upgrading.

### Features

* read checks through the control API ([#1315](https://github.com/sqlstack-io/sqlbuild/issues/1315)) ([865f8f7](https://github.com/sqlstack-io/sqlbuild/commit/865f8f79f5f9e10969420649ed5df41e48eb98f5))

## [0.188.0](https://github.com/sqlstack-io/sqlbuild/compare/v0.187.3...v0.188.0) (2026-11-05)


### Features

* add scheduler backend ([#1282](https://github.com/sqlstack-io/sqlbuild/issues/1282)) ([5f451f8](https://github.com/sqlstack-io/sqlbuild/commit/5f451f88acc929a5e9d7cb101f8ad7c4f65bfdc6))

## [0.187.3](https://github.com/sqlstack-io/sqlbuild/compare/v0.187.2...v0.187.3) (2026-11-02)


### Bug Fixes

* Snowflake time and timestamp precision no longer fails source contracts ([#1294](https://github.com/sqlstack-io/sqlbuild/issues/1294)) ([9c41a61](https://github.com/sqlstack-io/sqlbuild/commit/9c41a61825c3dd5db34f91447143821684bc718c))

## [0.187.2](https://github.com/sqlstack-io/sqlbuild/compare/v0.187.1...v0.187.2) (2026-11-02)


### Performance Improvements

* **rules:** pin SQLClean 0.4.3 for cheaper unused-CTE facts ([#1296](https://github.com/sqlstack-io/sqlbuild/issues/1296)) ([1ba0a3d](https://github.com/sqlstack-io/sqlbuild/commit/1ba0a3df837cdf34c4ef81f9a163127fe62841a9))

## [0.187.1](https://github.com/sqlstack-io/sqlbuild/compare/v0.187.0...v0.187.1) (2026-11-02)


### Bug Fixes

* **duckshop:** declare the columns staging models output and downstream models read ([#1297](https://github.com/sqlstack-io/sqlbuild/issues/1297)) ([43448ef](https://github.com/sqlstack-io/sqlbuild/commit/43448ef00b7150c5c4c60be60a0d1927c2c71ab5))

## [0.187.0](https://github.com/sqlstack-io/sqlbuild/compare/v0.186.2...v0.187.0) (2026-11-02)


### Features

* **rules:** run SQL Rules through the SQLClean engine ([#1283](https://github.com/sqlstack-io/sqlbuild/issues/1283)) ([f7c1e76](https://github.com/sqlstack-io/sqlbuild/commit/f7c1e7662333908bd834eb874df0b313da2cb3e8))

## [0.186.2](https://github.com/sqlstack-io/sqlbuild/compare/v0.186.1...v0.186.2) (2026-10-30)


### Bug Fixes

* retain native planner data through graph traversal ([#1280](https://github.com/sqlstack-io/sqlbuild/issues/1280)) ([90b3a9b](https://github.com/sqlstack-io/sqlbuild/commit/90b3a9b1ca6a895b1f511188448f2114daab472d))

## [0.186.1](https://github.com/sqlstack-io/sqlbuild/compare/v0.186.0...v0.186.1) (2026-10-29)


### Bug Fixes

* match configured snapshot keys and cursors to quoted columns ([#1273](https://github.com/sqlstack-io/sqlbuild/issues/1273)) ([2116208](https://github.com/sqlstack-io/sqlbuild/commit/2116208e354c27cf946c0252f494d0404ab1cf76))

## [0.186.0](https://github.com/sqlstack-io/sqlbuild/compare/v0.185.0...v0.186.0) (2026-10-29)


### Features

* support quoted mixed-case columns and read SQL with adapter lexical rules ([#1270](https://github.com/sqlstack-io/sqlbuild/issues/1270)) ([c3a2295](https://github.com/sqlstack-io/sqlbuild/commit/c3a2295864bb52432093ec9cedb7cfcc775d75fd))

## [0.185.0](https://github.com/chio-labs/sqlbuild/compare/v0.184.1...v0.185.0) (2026-10-28)


### Features

* **benchmarks:** add a DuckShop edge-case stress area ([dd0084a](https://github.com/chio-labs/sqlbuild/commit/dd0084a9554739d0bd3de355d8f6624da2c9457d))

## [0.184.1](https://github.com/chio-labs/sqlbuild/compare/v0.184.0...v0.184.1) (2026-10-28)


### Performance Improvements

* **snowflake:** cut per-model statement overhead, and write a Rules-failed compile to target/compiled-failed ([8d75270](https://github.com/chio-labs/sqlbuild/commit/8d75270ff6582977c958d2334b741c8ea1f757bd))

## [0.184.0](https://github.com/chio-labs/sqlbuild/compare/v0.183.0...v0.184.0) (2026-10-28)


### ⚠ BREAKING CHANGES

* `--format csv` and `--format table` are removed from `sqb query` and `sqb recall query`. Use the default output, `--format long`, or `--json`.

### Features

* harden Recall, unify query output and fix macro slot qualification ([db2f274](https://github.com/chio-labs/sqlbuild/commit/db2f27421a6714828a24e9f2f0a17a93e98c534b))

## [0.183.0](https://github.com/chio-labs/sqlbuild/compare/v0.182.0...v0.183.0) (2026-10-26)


### ⚠ BREAKING CHANGES

* SQLBuild requires DuckDB 2.0. MotherDuck users should stay on 0.182.0 until MotherDuck supports DuckDB 2.0. Database files SQLBuild creates cannot be opened by DuckDB 1.x.

### Features

* move to DuckDB 2.0 ([#1260](https://github.com/chio-labs/sqlbuild/issues/1260)) ([38f885f](https://github.com/chio-labs/sqlbuild/commit/38f885f207b031f8c20a6c4d38646b9f8e890903))

## [0.182.0](https://github.com/chio-labs/sqlbuild/compare/v0.181.0...v0.182.0) (2026-10-26)


### Features

* check enum literals at compile time and fix rule helper and slot checks ([#1258](https://github.com/chio-labs/sqlbuild/issues/1258)) ([229570f](https://github.com/chio-labs/sqlbuild/commit/229570f8afab139927bded0ecd4e9f7a8a2af0f1))

## [0.181.0](https://github.com/chio-labs/sqlbuild/compare/v0.180.0...v0.181.0) (2026-10-26)


### Features

* name failed checks in Dagster failures and make Recall runs self-describing ([#1256](https://github.com/chio-labs/sqlbuild/issues/1256)) ([07fb51a](https://github.com/chio-labs/sqlbuild/commit/07fb51af06fb93255d661d838a20adadb4673cca))

## [0.180.0](https://github.com/chio-labs/sqlbuild/compare/v0.179.0...v0.180.0) (2026-10-26)


### ⚠ BREAKING CHANGES

* **sql-macros:** COLUMN OF SQL macro arguments are removed; use relation slots.

### Features

* **sql-macros:** relation slots with COLUMNS groups, replacing COLUMN OF ([#1254](https://github.com/chio-labs/sqlbuild/issues/1254)) ([911882f](https://github.com/chio-labs/sqlbuild/commit/911882f236dc8979d233f9fc995dfcc0b57f0e39))

## [0.179.0](https://github.com/chio-labs/sqlbuild/compare/v0.178.0...v0.179.0) (2026-10-25)


### ⚠ BREAKING CHANGES

* resolve relative DuckDB and SQLite attach paths from the project directory
* reject Snowflake constructs Snowflake does not support
* on Snowflake, CONNECT_BY_ISLEAF and CONNECT_BY_ISCYCLE now raise E201, and a bare VALUES list as a CTE body, including in SQL-test fixtures, now fails with P001.

### Features

* add a Postgres option to turn off temporary test functions ([b91e2f3](https://github.com/chio-labs/sqlbuild/commit/b91e2f3b615943d7219e970903dae60e1b462823))


### Bug Fixes

* compare scenario VARIANT and JSON columns by JSON value in local replay ([b91e2f3](https://github.com/chio-labs/sqlbuild/commit/b91e2f3b615943d7219e970903dae60e1b462823))
* count Python checks in the build completion line, find mocked references on both sides of parenthesised joins, and support sqb mv with Python references ([b91e2f3](https://github.com/chio-labs/sqlbuild/commit/b91e2f3b615943d7219e970903dae60e1b462823))
* keep make skills out of HOME ([b91e2f3](https://github.com/chio-labs/sqlbuild/commit/b91e2f3b615943d7219e970903dae60e1b462823))
* map rule positions back through macro expansion ([b91e2f3](https://github.com/chio-labs/sqlbuild/commit/b91e2f3b615943d7219e970903dae60e1b462823))
* match Snowflake star outputs against lowercase contract columns ([b91e2f3](https://github.com/chio-labs/sqlbuild/commit/b91e2f3b615943d7219e970903dae60e1b462823))
* Polyglot star expansion with EXCLUDE, REPLACE and RENAME, column alias lists, Snowflake typed lambdas and subscript paths, nested set operation grouping, ClickHouse DATE_TRUNC and BigQuery RANGE types ([b91e2f3](https://github.com/chio-labs/sqlbuild/commit/b91e2f3b615943d7219e970903dae60e1b462823))
* register DuckDB UDFs on every test connection and attach each database once ([b91e2f3](https://github.com/chio-labs/sqlbuild/commit/b91e2f3b615943d7219e970903dae60e1b462823))
* reject --json on sqb dbt run and sqb dbt build ([b91e2f3](https://github.com/chio-labs/sqlbuild/commit/b91e2f3b615943d7219e970903dae60e1b462823))
* reject Snowflake constructs Snowflake does not support ([b91e2f3](https://github.com/chio-labs/sqlbuild/commit/b91e2f3b615943d7219e970903dae60e1b462823))
* resolve relative DuckDB and SQLite attach paths from the project directory ([b91e2f3](https://github.com/chio-labs/sqlbuild/commit/b91e2f3b615943d7219e970903dae60e1b462823))
* SQL Server ISO cursor bounds, Databricks schema checks without a catalog, and ClickHouse line comments ([b91e2f3](https://github.com/chio-labs/sqlbuild/commit/b91e2f3b615943d7219e970903dae60e1b462823))
* SQLite text timestamp cursors, temporal cast warnings, and dlt and ingestr loading ([b91e2f3](https://github.com/chio-labs/sqlbuild/commit/b91e2f3b615943d7219e970903dae60e1b462823))

## [0.178.0](https://github.com/chio-labs/sqlbuild/compare/v0.177.0...v0.178.0) (2026-10-25)


### ⚠ BREAKING CHANGES

* on Snowflake without QUOTED_IDENTIFIERS_IGNORE_CASE, a quoted lowercase reference such as "bundle_sku" to a listed source column now reports B002 during planning, as Snowflake would reject it against an upper-case column. The listing cannot tell a column really created with a quoted lowercase name apart, so such columns now bind as unquoted names.

### Bug Fixes

* Snowflake, scenario, rule and type correctness batch ([#1250](https://github.com/chio-labs/sqlbuild/issues/1250)) ([d2b4ce2](https://github.com/chio-labs/sqlbuild/commit/d2b4ce287353c24ffbdc6aaf4ed72d17359dffc2))

## [0.177.0](https://github.com/chio-labs/sqlbuild/compare/v0.176.0...v0.177.0) (2026-10-24)


### Features

* **benchmarks:** add the DuckShop 10K generator ([#1248](https://github.com/chio-labs/sqlbuild/issues/1248)) ([3dc8d7f](https://github.com/chio-labs/sqlbuild/commit/3dc8d7f6ac8caac6c601d2ca4f8df548a004f35d))
* type SQL macro calls everywhere, with optional returns, TYPE OF, ANY families and __cte ([#1247](https://github.com/chio-labs/sqlbuild/issues/1247)) ([4e07586](https://github.com/chio-labs/sqlbuild/commit/4e07586c3c0ac31f2f9a65f6a911d6a5870c65d4))

## [0.176.0](https://github.com/chio-labs/sqlbuild/compare/v0.175.0...v0.176.0) (2026-10-24)


### Features

* **benchmarks:** add DuckShop, a dense Snowflake example project ([#1245](https://github.com/chio-labs/sqlbuild/issues/1245)) ([e68476e](https://github.com/chio-labs/sqlbuild/commit/e68476e646c0e56446f03038c77fdd68deca4b3a))

## [0.175.0](https://github.com/chio-labs/sqlbuild/compare/v0.174.0...v0.175.0) (2026-10-24)


### ⚠ BREAKING CHANGES

* fold header descriptions and accept triple-quoted strings ([#1242](https://github.com/chio-labs/sqlbuild/issues/1242))

### Features

* fold header descriptions and accept triple-quoted strings ([#1242](https://github.com/chio-labs/sqlbuild/issues/1242)) ([4fcdd05](https://github.com/chio-labs/sqlbuild/commit/4fcdd05ea05eb2ec8d725e071f7139dd8203ed8e))

## [0.174.0](https://github.com/chio-labs/sqlbuild/compare/v0.173.0...v0.174.0) (2026-10-23)


### Features

* ANY macro arguments, macro return types in contract checks and per-kind macro folder counts ([#1240](https://github.com/chio-labs/sqlbuild/issues/1240)) ([8b9f207](https://github.com/chio-labs/sqlbuild/commit/8b9f2077a8439fb3deb9cf7eb069b2043c023e6a))

## [0.173.0](https://github.com/chio-labs/sqlbuild/compare/v0.172.0...v0.173.0) (2026-10-23)


### Features

* add sqb review to build, test and compare a branch in a temporary schema ([#1238](https://github.com/chio-labs/sqlbuild/issues/1238)) ([6cfb178](https://github.com/chio-labs/sqlbuild/commit/6cfb1782f568de6860b3e2bf2c52caa58163a694))

## [0.172.0](https://github.com/chio-labs/sqlbuild/compare/v0.171.0...v0.172.0) (2026-10-23)


### Features

* show which janitor candidates are in use, and measure and cut custom rule overhead ([#1232](https://github.com/chio-labs/sqlbuild/issues/1232)) ([90970f2](https://github.com/chio-labs/sqlbuild/commit/90970f23c289f4bd5de8d4988bddedd809b2f17e))


### Bug Fixes

* lint SQL macro bodies as compiled and pass SQL macro expansions to Python macros intact ([#1236](https://github.com/chio-labs/sqlbuild/issues/1236)) ([2357ce2](https://github.com/chio-labs/sqlbuild/commit/2357ce2c6550b102cf35bdf3aaa2b1e7944eca8b))

## [0.171.0](https://github.com/chio-labs/sqlbuild/compare/v0.170.0...v0.171.0) (2026-10-23)


### ⚠ BREAKING CHANGES

* Python macros now live in macros/python/ (and the python/ folder of scoped macro roots) and are called as @py.name(...). A macro file directly under a macros/ folder fails with D013, and a bare @name(...) call fails with P014; move files into macros/python/ and replace @name( with @py.name( in project SQL.

### Features

* add typed SQL macros and require [@py](https://github.com/py)./[@sql](https://github.com/sql). macro calls ([#1224](https://github.com/chio-labs/sqlbuild/issues/1224)) ([21a802b](https://github.com/chio-labs/sqlbuild/commit/21a802bfbb22b6cb23eab6be932da2de48beb45f))

## [0.170.0](https://github.com/chio-labs/sqlbuild/compare/v0.169.0...v0.170.0) (2026-10-23)


### Features

* batch renames, Python import rewrites and repairs of existing rule failures in sqb rename and sqb mv ([#1229](https://github.com/chio-labs/sqlbuild/issues/1229)) ([39fef4b](https://github.com/chio-labs/sqlbuild/commit/39fef4b7d80c4e78d987716dd8748918131fae11))

## [0.169.0](https://github.com/chio-labs/sqlbuild/compare/v0.168.0...v0.169.0) (2026-10-23)


### Features

* run only affected tests and scenarios with --since ([#1228](https://github.com/chio-labs/sqlbuild/issues/1228)) ([720539c](https://github.com/chio-labs/sqlbuild/commit/720539c74d2759aec746b4badaebf7783c0a68ac))
* show janitor progress, inspect and archive faster ([#1226](https://github.com/chio-labs/sqlbuild/issues/1226)) ([bc1245c](https://github.com/chio-labs/sqlbuild/commit/bc1245ce7bbdda1ad69a8956d43596267b743abc))

## [0.168.0](https://github.com/chio-labs/sqlbuild/compare/v0.167.2...v0.168.0) (2026-10-22)


### Features

* create the Recall database automatically and lead the Recall docs with the intended workflow ([#1222](https://github.com/chio-labs/sqlbuild/issues/1222)) ([65a5ee0](https://github.com/chio-labs/sqlbuild/commit/65a5ee01e4cb27b197fc4b40b3e0cd7a17dbba66))

## [0.167.2](https://github.com/chio-labs/sqlbuild/compare/v0.167.1...v0.167.2) (2026-10-22)


### Bug Fixes

* let read-only users query Recall on ClickHouse and read each table once ([#1218](https://github.com/chio-labs/sqlbuild/issues/1218)) ([32aa75a](https://github.com/chio-labs/sqlbuild/commit/32aa75a1eb62078ce4c6afc6900a05955cc19a28))

## [0.167.1](https://github.com/chio-labs/sqlbuild/compare/v0.167.0...v0.167.1) (2026-10-22)


### Bug Fixes

* deferral, refactor, microbatch progress, scenario, diff and clone fixes ([#1212](https://github.com/chio-labs/sqlbuild/issues/1212)) ([78b3adc](https://github.com/chio-labs/sqlbuild/commit/78b3adc62b8375ea082bf2ed0d97c5a5fdd7e67f))
* evaluate every rule over the whole project on selected runs ([#1216](https://github.com/chio-labs/sqlbuild/issues/1216)) ([5f46d68](https://github.com/chio-labs/sqlbuild/commit/5f46d68fb9442b3a9c0c007d2ee4aa297f74c792))
* match interrupted connections by the connection, not a reusable id ([#1214](https://github.com/chio-labs/sqlbuild/issues/1214)) ([e1a2850](https://github.com/chio-labs/sqlbuild/commit/e1a285091c1338719dc4da0732a0d98a8dc5cc14))

## [0.167.0](https://github.com/chio-labs/sqlbuild/compare/v0.166.0...v0.167.0) (2026-10-22)


### ⚠ BREAKING CHANGES

* the Recall `connection` key is removed. A configuration that still sets it fails with an error naming `write_connection` and `read_connection`; set both to the old connection to keep the previous behaviour.

### Features

* split Recall read and write connections, faster recall queries, richer failure records ([#1210](https://github.com/chio-labs/sqlbuild/issues/1210)) ([5c70edd](https://github.com/chio-labs/sqlbuild/commit/5c70edd90d6496f3ea8c8a9f96625ba353330578))


### Bug Fixes

* error a unit test when its tested output query fails ([#1208](https://github.com/chio-labs/sqlbuild/issues/1208)) ([e250b75](https://github.com/chio-labs/sqlbuild/commit/e250b755de97d7828ff770bd2304cce6343ffbd7))

## [0.166.0](https://github.com/chio-labs/sqlbuild/compare/v0.165.0...v0.166.0) (2026-10-21)


### Features

* move several models in one sqb mv and match carried declaration layouts ([#1206](https://github.com/chio-labs/sqlbuild/issues/1206)) ([601ccfc](https://github.com/chio-labs/sqlbuild/commit/601ccfc87bb5cb6a64b06dd528aac4a76e28f58a))

## [0.165.0](https://github.com/chio-labs/sqlbuild/compare/v0.164.5...v0.165.0) (2026-10-21)


### ⚠ BREAKING CHANGES

* projects whose audit attachments pass arguments the audit never reads must remove those arguments.

### Bug Fixes

* batch of refactor, sink, audit, CLI and rules fixes ([#1202](https://github.com/chio-labs/sqlbuild/issues/1202)) ([1ecc1d0](https://github.com/chio-labs/sqlbuild/commit/1ecc1d07498943ff8e148e5df0b166ca53b59050))


### Performance Improvements

* **compile:** delta-compile dynamic columns, audit factories and model rules ([#1201](https://github.com/chio-labs/sqlbuild/issues/1201)) ([b5bdb76](https://github.com/chio-labs/sqlbuild/commit/b5bdb7685c2ce268c85387c1ee8cf796eb1f5cc7))

## [0.164.5](https://github.com/chio-labs/sqlbuild/compare/v0.164.4...v0.164.5) (2026-10-21)


### Bug Fixes

* **ingestr:** pin the DuckDB ADBC driver to the installed duckdb version ([#1203](https://github.com/chio-labs/sqlbuild/issues/1203)) ([e1ca0bc](https://github.com/chio-labs/sqlbuild/commit/e1ca0bcce8548969a3cf1f728f054c00e5a659fd))

## [0.164.4](https://github.com/chio-labs/sqlbuild/compare/v0.164.3...v0.164.4) (2026-10-21)


### Bug Fixes

* evaluate project-wide rules over the whole project on selected runs ([#1198](https://github.com/chio-labs/sqlbuild/issues/1198)) ([5556991](https://github.com/chio-labs/sqlbuild/commit/555699187fe21e2579a898b5f9ef80db1e546071))

## [0.164.3](https://github.com/chio-labs/sqlbuild/compare/v0.164.2...v0.164.3) (2026-10-21)


### Performance Improvements

* **format:** remove repeated work from the native SQL formatter ([#1188](https://github.com/chio-labs/sqlbuild/issues/1188)) ([8928936](https://github.com/chio-labs/sqlbuild/commit/8928936359692fca5f59a6b99168cb91c3440dd3))

## [0.164.2](https://github.com/chio-labs/sqlbuild/compare/v0.164.1...v0.164.2) (2026-10-20)


### Performance Improvements

* **format:** format files in parallel and skip unchanged files ([#1183](https://github.com/chio-labs/sqlbuild/issues/1183)) ([1f39065](https://github.com/chio-labs/sqlbuild/commit/1f390652f13d5499f4eee68edb209be605d6a55a))

## [0.164.1](https://github.com/chio-labs/sqlbuild/compare/v0.164.0...v0.164.1) (2026-10-20)


### Bug Fixes

* speed up clone preparation and keep its console output clean ([#1180](https://github.com/chio-labs/sqlbuild/issues/1180)) ([f21d774](https://github.com/chio-labs/sqlbuild/commit/f21d7749138be0b01b705d4ed76170c851933f44))


### Performance Improvements

* build the sqb binary and the extension from one set of compiled dependencies ([#1179](https://github.com/chio-labs/sqlbuild/issues/1179)) ([b4620f4](https://github.com/chio-labs/sqlbuild/commit/b4620f41088363ea5142ee3b40ee2ebfa907fe31))

## [0.164.0](https://github.com/chio-labs/sqlbuild/compare/v0.163.0...v0.164.0) (2026-10-20)


### ⚠ BREAKING CHANGES

* inline `-- sqb: ignore` comments are errors and `[rules] allow_exceptions` is rejected. Move each reviewed deviation into a [[rules.rule_exceptions]] entry and delete the setting.

### Features

* make configuration the only Rule exception mechanism, speed up sqb query and fix exit and cost failures ([#1175](https://github.com/chio-labs/sqlbuild/issues/1175)) ([d8b34c8](https://github.com/chio-labs/sqlbuild/commit/d8b34c89c275b3b056c7d35a84beb5eb94168556))

## [0.163.0](https://github.com/chio-labs/sqlbuild/compare/v0.162.1...v0.163.0) (2026-10-20)


### Features

* answer simple commands in sqb, upgrade fensu and accept SharedMergeTree ([#1170](https://github.com/chio-labs/sqlbuild/issues/1170)) ([bb1c502](https://github.com/chio-labs/sqlbuild/commit/bb1c502837fa4d491f21df665fddedab998d766d))

## [0.162.1](https://github.com/chio-labs/sqlbuild/compare/v0.162.0...v0.162.1) (2026-10-19)


### Bug Fixes

* stop compute log capture deadlocking a command at exit ([#1165](https://github.com/chio-labs/sqlbuild/issues/1165)) ([21727f6](https://github.com/chio-labs/sqlbuild/commit/21727f6320690e04548d08fece08fa22a0e2138b))

## [0.162.0](https://github.com/chio-labs/sqlbuild/compare/v0.161.0...v0.162.0) (2026-10-19)


### Features

* store Recall history in ClickHouse and add replacement sinks ([#1163](https://github.com/chio-labs/sqlbuild/issues/1163)) ([41e183e](https://github.com/chio-labs/sqlbuild/commit/41e183e8203c5685e0ff668e42fadb3e046402af))

## [0.161.0](https://github.com/chio-labs/sqlbuild/compare/v0.160.2...v0.161.0) (2026-10-19)


### Features

* record local command history with Recall ([#1155](https://github.com/chio-labs/sqlbuild/issues/1155)) ([4c0d23f](https://github.com/chio-labs/sqlbuild/commit/4c0d23f89c6c4559c25d303994c7cfd5de6de4a2))


### Bug Fixes

* stop false errors on stars, comments, set operations, snapshots and table-view switches ([#1158](https://github.com/chio-labs/sqlbuild/issues/1158)) ([b21b0ff](https://github.com/chio-labs/sqlbuild/commit/b21b0ff035d97b70415d49b687f63f1be22c2345))

## [0.160.2](https://github.com/chio-labs/sqlbuild/compare/v0.160.1...v0.160.2) (2026-10-19)


### Performance Improvements

* compile only the changed models after an edit ([#1157](https://github.com/chio-labs/sqlbuild/issues/1157)) ([0808c72](https://github.com/chio-labs/sqlbuild/commit/0808c724c9c5c3800071d5c72dd08dbc4d23a9e9))

## [0.160.1](https://github.com/chio-labs/sqlbuild/compare/v0.160.0...v0.160.1) (2026-10-19)


### Bug Fixes

* keep SQLite loader and source timestamps as text and add a waffle shop E2E ([#1154](https://github.com/chio-labs/sqlbuild/issues/1154)) ([f5b655e](https://github.com/chio-labs/sqlbuild/commit/f5b655ee6d95afbea3fa63adefcd47cc04b0e5d7))

## [0.160.0](https://github.com/chio-labs/sqlbuild/compare/v0.159.0...v0.160.0) (2026-10-19)


### Features

* add a ClickHouse adapter and fix CLI rename, errors and test functions ([#1152](https://github.com/chio-labs/sqlbuild/issues/1152)) ([7d3b43a](https://github.com/chio-labs/sqlbuild/commit/7d3b43a760c15aa6a5b4d82e8dbab4e267c59a8c))

## [0.159.0](https://github.com/chio-labs/sqlbuild/compare/v0.158.2...v0.159.0) (2026-10-19)


### Features

* add a SQLite adapter ([#1150](https://github.com/chio-labs/sqlbuild/issues/1150)) ([e86f3a0](https://github.com/chio-labs/sqlbuild/commit/e86f3a00c82a9814b5fc694c00519f97699ac30b))

## [0.158.2](https://github.com/chio-labs/sqlbuild/compare/v0.158.1...v0.158.2) (2026-10-19)


### Bug Fixes

* scenario, SQL test and dialect fixes for fixtures, checks and truth tests ([#1147](https://github.com/chio-labs/sqlbuild/issues/1147)) ([a411860](https://github.com/chio-labs/sqlbuild/commit/a411860fcadf4bd23d509df3f63ba8990ca77029))

## [0.158.1](https://github.com/chio-labs/sqlbuild/compare/v0.158.0...v0.158.1) (2026-10-18)


### Performance Improvements

* cut edit-loop compile overhead in the reuse store, hashing and walks ([#1144](https://github.com/chio-labs/sqlbuild/issues/1144)) ([5f52299](https://github.com/chio-labs/sqlbuild/commit/5f522999845b86a3b9af91a98493d8c1e7de3c3c))

## [0.158.0](https://github.com/chio-labs/sqlbuild/compare/v0.157.4...v0.158.0) (2026-10-18)


### Features

* bound SQL unit test and model execution cost and attach audit metadata ([#1141](https://github.com/chio-labs/sqlbuild/issues/1141)) ([dfdadd8](https://github.com/chio-labs/sqlbuild/commit/dfdadd80df071bde50235150b30eb2bb4124afbe))

## [0.157.4](https://github.com/chio-labs/sqlbuild/compare/v0.157.3...v0.157.4) (2026-10-18)


### Performance Improvements

* read native records directly and shorten the compile tail ([#1139](https://github.com/chio-labs/sqlbuild/issues/1139)) ([98a2133](https://github.com/chio-labs/sqlbuild/commit/98a213385340d0021e1a0dd6c738b3cd765fbc1a))

## [0.157.3](https://github.com/chio-labs/sqlbuild/compare/v0.157.2...v0.157.3) (2026-10-18)


### Performance Improvements

* remove redundant graph-phase scans and parallelise source and resource SQL preparation ([#1137](https://github.com/chio-labs/sqlbuild/issues/1137)) ([4505bbb](https://github.com/chio-labs/sqlbuild/commit/4505bbbba50d5984d8208b3fc2a14bd99f0a52f7))

## [0.157.2](https://github.com/chio-labs/sqlbuild/compare/v0.157.1...v0.157.2) (2026-10-18)


### Performance Improvements

* write the custom-rule host payload natively and decode facts lazily ([#1135](https://github.com/chio-labs/sqlbuild/issues/1135)) ([c7817ab](https://github.com/chio-labs/sqlbuild/commit/c7817ab82a151a6d9e177059feae218e99b7b14a))

## [0.157.1](https://github.com/chio-labs/sqlbuild/compare/v0.157.0...v0.157.1) (2026-10-17)


### Performance Improvements

* cut serial compile work and duplicated parsing on dense projects ([#1130](https://github.com/chio-labs/sqlbuild/issues/1130)) ([409e328](https://github.com/chio-labs/sqlbuild/commit/409e3286f06725fae9bd678c33c566000de4845e))

## [0.157.0](https://github.com/chio-labs/sqlbuild/compare/v0.156.1...v0.157.0) (2026-10-17)


### Code Refactoring

* guard native bindings and read native views, properties and dicts directly ([#1126](https://github.com/chio-labs/sqlbuild/issues/1126)) ([1729fce](https://github.com/chio-labs/sqlbuild/commit/1729fce860671d91a4036eaa87beac035603c0e9))

## [0.156.1](https://github.com/chio-labs/sqlbuild/compare/v0.156.0...v0.156.1) (2026-10-12)


### Performance Improvements

* skip redundant SQL normalisation and CTE scans during compile ([#1088](https://github.com/chio-labs/sqlbuild/issues/1088)) ([3ffcae2](https://github.com/chio-labs/sqlbuild/commit/3ffcae2dbe83a5ec44c62e7b15f9edc9608cdcf6))

## [0.156.0](https://github.com/chio-labs/sqlbuild/compare/v0.155.0...v0.156.0) (2026-10-12)


### Features

* complete native compiler ownership and CLI replay ([#1082](https://github.com/chio-labs/sqlbuild/issues/1082)) ([076d64f](https://github.com/chio-labs/sqlbuild/commit/076d64fd3bfec6ec057bfd3ce573b64eda6ad3a0))


### Bug Fixes

* install companion CLI wheel in release checks ([#1084](https://github.com/chio-labs/sqlbuild/issues/1084)) ([d9d3477](https://github.com/chio-labs/sqlbuild/commit/d9d34774136f0a77bc30965d7e2e68457b538431))
* simplify native TOML config reading and restore error columns ([#1085](https://github.com/chio-labs/sqlbuild/issues/1085)) ([00a533d](https://github.com/chio-labs/sqlbuild/commit/00a533d222c9c6eed0661813bb3619a0068eec45))

## [0.155.0](https://github.com/chio-labs/sqlbuild/compare/v0.154.0...v0.155.0) (2026-10-10)


### Features

* **native:** make native model analysis the default with a native analysis cache ([#1079](https://github.com/chio-labs/sqlbuild/issues/1079)) ([2a73431](https://github.com/chio-labs/sqlbuild/commit/2a7343197e10433f3cafc2e350bc421d9100c3f3))

## [0.154.0](https://github.com/chio-labs/sqlbuild/compare/v0.153.0...v0.154.0) (2026-10-10)


### Features

* **native:** close out M3 shipped-stage ports and delete their Python fallbacks ([#1077](https://github.com/chio-labs/sqlbuild/issues/1077)) ([2cec8a4](https://github.com/chio-labs/sqlbuild/commit/2cec8a423db47698ccf1b96b95b707cd57f75b4c))

## [0.153.0](https://github.com/chio-labs/sqlbuild/compare/v0.152.0...v0.153.0) (2026-10-10)


### Features

* **native:** land the reviewed M4 preview ports and speed fixes together ([#1075](https://github.com/chio-labs/sqlbuild/issues/1075)) ([85a10f0](https://github.com/chio-labs/sqlbuild/commit/85a10f09e67f35d0ab2f4570519ef562368bad10))

## [0.152.0](https://github.com/chio-labs/sqlbuild/compare/v0.151.0...v0.152.0) (2026-10-09)


### Features

* **native:** assemble project resource facts natively behind the preview engine ([#1066](https://github.com/chio-labs/sqlbuild/issues/1066)) ([daf0620](https://github.com/chio-labs/sqlbuild/commit/daf062043373b9b844a681c69d091ee2624f468f))

## [0.151.0](https://github.com/chio-labs/sqlbuild/compare/v0.150.0...v0.151.0) (2026-10-09)


### Features

* **native:** plan SQL tests natively behind the preview engine ([#1056](https://github.com/chio-labs/sqlbuild/issues/1056)) ([509d559](https://github.com/chio-labs/sqlbuild/commit/509d5598d1266021dcfb90dfcfb4c155951c655d))

## [0.150.0](https://github.com/chio-labs/sqlbuild/compare/v0.149.0...v0.150.0) (2026-10-09)


### Features

* **native:** enrich analysis inputs natively behind the preview engine ([#1055](https://github.com/chio-labs/sqlbuild/issues/1055)) ([762eac9](https://github.com/chio-labs/sqlbuild/commit/762eac9e424d436a43b6d2238cbfe0115d9739c1))
* **native:** run metadata checks natively behind the preview engine ([#1058](https://github.com/chio-labs/sqlbuild/issues/1058)) ([9a8e410](https://github.com/chio-labs/sqlbuild/commit/9a8e4109599d9a4a42ab02aba1d412bb3e8db5dc))
* **native:** validate contracts natively behind the preview engine ([#1054](https://github.com/chio-labs/sqlbuild/issues/1054)) ([b930183](https://github.com/chio-labs/sqlbuild/commit/b93018345c9acc4d13000fd86ae5d1963d46bd4c))

## [0.149.0](https://github.com/chio-labs/sqlbuild/compare/v0.148.0...v0.149.0) (2026-10-09)


### Features

* **native:** run semantic checks natively behind the preview engine ([#1050](https://github.com/chio-labs/sqlbuild/issues/1050)) ([62cc5b8](https://github.com/chio-labs/sqlbuild/commit/62cc5b870b023d77aa9d7e07d712df7983329e37))

## [0.148.0](https://github.com/chio-labs/sqlbuild/compare/v0.147.0...v0.148.0) (2026-10-09)


### Features

* **native:** analyse models natively behind the preview engine ([#1045](https://github.com/chio-labs/sqlbuild/issues/1045)) ([4d07a41](https://github.com/chio-labs/sqlbuild/commit/4d07a416da943401f3ef6cfcf6ddb4ecc067c5c3))
* **native:** report macro errors natively without re-running Python ([#1051](https://github.com/chio-labs/sqlbuild/issues/1051)) ([0ce6787](https://github.com/chio-labs/sqlbuild/commit/0ce6787a9eb2a601c0a164cdaf7b4534100ec1ba))
* **scenarios:** reject quoted and materialized scenario CTE names like SQL tests ([#1052](https://github.com/chio-labs/sqlbuild/issues/1052)) ([33fd340](https://github.com/chio-labs/sqlbuild/commit/33fd340bdc416ce459e81e7de036d5aa5154c20f))


### Bug Fixes

* **rename:** suggest the one-step sqb mv command when a rename crosses layers ([#1048](https://github.com/chio-labs/sqlbuild/issues/1048)) ([4f5e8c6](https://github.com/chio-labs/sqlbuild/commit/4f5e8c6a32fd8e74308c3c7114a585dd8b5a88a9))

## [0.147.0](https://github.com/chio-labs/sqlbuild/compare/v0.146.0...v0.147.0) (2026-10-09)


### Features

* **native:** make native rendering the default compiler path ([#1043](https://github.com/chio-labs/sqlbuild/issues/1043)) ([95491e1](https://github.com/chio-labs/sqlbuild/commit/95491e1ed70afe8c834da7841297f7d9663699fc))
* **native:** report SQL test and scenario errors natively ([#1046](https://github.com/chio-labs/sqlbuild/issues/1046)) ([0e87c96](https://github.com/chio-labs/sqlbuild/commit/0e87c96b2e940e025bf2bb771d48ee7f317eadb1))

## [0.146.0](https://github.com/chio-labs/sqlbuild/compare/v0.145.0...v0.146.0) (2026-10-08)


### Features

* **native:** report reference extraction errors natively ([#1037](https://github.com/chio-labs/sqlbuild/issues/1037)) ([8dc2d1e](https://github.com/chio-labs/sqlbuild/commit/8dc2d1e203e3fd3a7cc411d9b4d583435ab911dc))

## [0.145.0](https://github.com/chio-labs/sqlbuild/compare/v0.144.0...v0.145.0) (2026-10-08)


### Features

* **native:** report model config errors natively ([#1042](https://github.com/chio-labs/sqlbuild/issues/1042)) ([19ae79e](https://github.com/chio-labs/sqlbuild/commit/19ae79ebf10aaa96bb381e2a9baa5ea05bb65b8c))

## [0.144.0](https://github.com/chio-labs/sqlbuild/compare/v0.143.1...v0.144.0) (2026-10-08)


### Features

* **native:** build model config and validate models natively behind the preview engine ([#1034](https://github.com/chio-labs/sqlbuild/issues/1034)) ([24e47aa](https://github.com/chio-labs/sqlbuild/commit/24e47aa5399b702a92e2fbc13aac70cf39f8d15a))
* **native:** report attachment errors natively ([#1038](https://github.com/chio-labs/sqlbuild/issues/1038)) ([ee19894](https://github.com/chio-labs/sqlbuild/commit/ee19894bb5328fcb3310e21eff3962934189fb48))
* **native:** report declaration and scope errors natively ([#1033](https://github.com/chio-labs/sqlbuild/issues/1033)) ([241fb56](https://github.com/chio-labs/sqlbuild/commit/241fb56ef8454c7e9c5739cbb3169d4b5e2a0fd1))

## [0.143.1](https://github.com/chio-labs/sqlbuild/compare/v0.143.0...v0.143.1) (2026-10-08)


### Bug Fixes

* **test:** reject mocks reached through reference calls that read referencing helpers ([#1035](https://github.com/chio-labs/sqlbuild/issues/1035)) ([4ba74a0](https://github.com/chio-labs/sqlbuild/commit/4ba74a0ddb72549295f3480537bd229d254107b5))

## [0.143.0](https://github.com/chio-labs/sqlbuild/compare/v0.142.0...v0.143.0) (2026-10-08)


### Features

* **native:** build fast column lineage natively behind the preview engine ([#1029](https://github.com/chio-labs/sqlbuild/issues/1029)) ([e074bd1](https://github.com/chio-labs/sqlbuild/commit/e074bd14e6562132dba7633c3da6485809baa7d9))
* **native:** normalise and compare types natively behind the preview engine ([#1028](https://github.com/chio-labs/sqlbuild/issues/1028)) ([37df06a](https://github.com/chio-labs/sqlbuild/commit/37df06a16390c0b2eba2106d6e521cb765fd1729))

## [0.142.0](https://github.com/chio-labs/sqlbuild/compare/v0.141.0...v0.142.0) (2026-10-08)


### Features

* **native:** extract raw and expanded SQL tests with one native extractor ([#1026](https://github.com/chio-labs/sqlbuild/issues/1026)) ([82b33b0](https://github.com/chio-labs/sqlbuild/commit/82b33b040ff4abb874b56a1fb7fa433610822028))

## [0.141.0](https://github.com/chio-labs/sqlbuild/compare/v0.140.1...v0.141.0) (2026-10-08)


### Features

* **native:** register the analysis stages behind the preview engine ([#1024](https://github.com/chio-labs/sqlbuild/issues/1024)) ([18ed683](https://github.com/chio-labs/sqlbuild/commit/18ed6830b2faa9d41d420d3407530b94132add47))

## [0.140.1](https://github.com/chio-labs/sqlbuild/compare/v0.140.0...v0.140.1) (2026-10-08)


### Bug Fixes

* **test:** resolve mocked and expected-row references consistently in SQL tests ([#1023](https://github.com/chio-labs/sqlbuild/issues/1023)) ([7e908ac](https://github.com/chio-labs/sqlbuild/commit/7e908ac10ffa5f051670b68d1cebc457bca3512a))

## [0.140.0](https://github.com/chio-labs/sqlbuild/compare/v0.139.0...v0.140.0) (2026-10-08)


### Features

* **native:** attach SQL tests, scenarios and functions natively behind the preview engine ([#1022](https://github.com/chio-labs/sqlbuild/issues/1022)) ([4f9a9b7](https://github.com/chio-labs/sqlbuild/commit/4f9a9b7b0d995da796c4f19f846259394a5ddebd))
* **native:** batch and cache macro calls behind the preview engine ([#1013](https://github.com/chio-labs/sqlbuild/issues/1013)) ([8bc5325](https://github.com/chio-labs/sqlbuild/commit/8bc532556ca211de689d8a70c84dc1520058c20e))
* **native:** build compile attachments natively behind the preview engine ([#1018](https://github.com/chio-labs/sqlbuild/issues/1018)) ([79a5b45](https://github.com/chio-labs/sqlbuild/commit/79a5b45407f9f5eadc6e061781be50f0958952b9))
* **native:** extract model references natively behind the preview engine ([#1008](https://github.com/chio-labs/sqlbuild/issues/1008)) ([3fb32ee](https://github.com/chio-labs/sqlbuild/commit/3fb32eec9cdbbe98fbe198bdb421ae8112f4a19c))
* **native:** make native discovery the default compiler engine ([#1003](https://github.com/chio-labs/sqlbuild/issues/1003)) ([d0c6760](https://github.com/chio-labs/sqlbuild/commit/d0c6760b97e286fff945bf41c4ea51dc42dcacaa))
* **native:** parse declaration files natively behind the preview engine ([#1014](https://github.com/chio-labs/sqlbuild/issues/1014)) ([2cb1514](https://github.com/chio-labs/sqlbuild/commit/2cb151480da21249c6a715054eb655a74a729909))
* **native:** resolve declaration contexts and references natively in the preview model loop ([#1012](https://github.com/chio-labs/sqlbuild/issues/1012)) ([b5a2453](https://github.com/chio-labs/sqlbuild/commit/b5a24532295dd05229135aead535db7206e2c873))
* **native:** resolve declaration scopes natively behind a native-preview engine tier ([#1006](https://github.com/chio-labs/sqlbuild/issues/1006)) ([799db35](https://github.com/chio-labs/sqlbuild/commit/799db3580ce36a8b75ba123a0bf603ce7f4ddc52))
* **native:** resolve model configuration natively behind the preview engine ([#1009](https://github.com/chio-labs/sqlbuild/issues/1009)) ([e166844](https://github.com/chio-labs/sqlbuild/commit/e1668441f98c721027118125a00d848926aefeac))


### Bug Fixes

* **compiler:** keep shared-analysis lineage and stage captures deterministic ([#1005](https://github.com/chio-labs/sqlbuild/issues/1005)) ([1175b83](https://github.com/chio-labs/sqlbuild/commit/1175b8367384b11f0ef618dea05aec8e15af12a1))
* **compiler:** reject reference calls that compile cannot replace ([#1011](https://github.com/chio-labs/sqlbuild/issues/1011)) ([57c6da6](https://github.com/chio-labs/sqlbuild/commit/57c6da632edba32d0459e63be0d1facf226cf0f9))
* **compiler:** treat dollar-quoted strings as quoted text in variable substitution ([#1020](https://github.com/chio-labs/sqlbuild/issues/1020)) ([c9b0985](https://github.com/chio-labs/sqlbuild/commit/c9b09854d08ea5b67debe3305dcd9404df7c8f75))
* **native:** report deeply nested header values instead of crashing ([#1015](https://github.com/chio-labs/sqlbuild/issues/1015)) ([eca2cec](https://github.com/chio-labs/sqlbuild/commit/eca2cecc5dc7966d02923cb088b75f5edc36ed02))
* **test:** rewrite references in helper CTEs and order them after their dependencies ([#1019](https://github.com/chio-labs/sqlbuild/issues/1019)) ([620bca0](https://github.com/chio-labs/sqlbuild/commit/620bca02b0a39fd990595d61cba8b23dc8376191))

## [0.139.0](https://github.com/chio-labs/sqlbuild/compare/v0.138.0...v0.139.0) (2026-10-07)


### Features

* make TEST headers, test modes and trailing SELECT 1 optional ([#978](https://github.com/chio-labs/sqlbuild/issues/978)) ([e1e7e62](https://github.com/chio-labs/sqlbuild/commit/e1e7e624819d0d3b3a69b63ace3c9e85e1fceeb4))
* **native:** add core types, exact JSON emitter and config parsing ([#992](https://github.com/chio-labs/sqlbuild/issues/992)) ([6794064](https://github.com/chio-labs/sqlbuild/commit/679406499e67a3abec08d923eed45a2f7604a6cd))
* **native:** discover declarations, SQL tests and YAML files natively ([#999](https://github.com/chio-labs/sqlbuild/issues/999)) ([c0b546f](https://github.com/chio-labs/sqlbuild/commit/c0b546f3dc7ab4cec004da64910f3aede3d6df71))
* **native:** discover models natively behind the compiler engine switch ([#995](https://github.com/chio-labs/sqlbuild/issues/995)) ([d4c7157](https://github.com/chio-labs/sqlbuild/commit/d4c71572807e8b6d006b88f07fd929ea6d720693))


### Bug Fixes

* default targets to the only connection and infer source freshness settings ([#973](https://github.com/chio-labs/sqlbuild/issues/973)) ([a811140](https://github.com/chio-labs/sqlbuild/commit/a811140d5f87a99c475cd3def13768e38d76125e))
* require a TEST() header in SQL test files again ([#1000](https://github.com/chio-labs/sqlbuild/issues/1000)) ([9742975](https://github.com/chio-labs/sqlbuild/commit/97429757124295bc92a0d1ff691ebd6e0a8e28b1))

## [0.138.0](https://github.com/chio-labs/sqlbuild/compare/v0.137.0...v0.138.0) (2026-10-06)


### Features

* **compiler:** add compiler engine switch and differential harness ([#987](https://github.com/chio-labs/sqlbuild/issues/987)) ([94683ca](https://github.com/chio-labs/sqlbuild/commit/94683ca41c1319dff0897988cd0480e49a30bfc2))

## [0.137.0](https://github.com/chio-labs/sqlbuild/compare/v0.136.2...v0.137.0) (2026-10-06)


### Features

* **test:** select unit tests by name and require unique test names ([#989](https://github.com/chio-labs/sqlbuild/issues/989)) ([896c452](https://github.com/chio-labs/sqlbuild/commit/896c4524542d259fd2fac7eb0ea9b5ed80c805af))

## [0.136.2](https://github.com/chio-labs/sqlbuild/compare/v0.136.1...v0.136.2) (2026-10-06)


### Bug Fixes

* **test:** isolate unit-test SQL per model and reserve helper names ([#986](https://github.com/chio-labs/sqlbuild/issues/986)) ([0e38e08](https://github.com/chio-labs/sqlbuild/commit/0e38e087caf64c1f08de14ce18ada37428239487))
* **test:** isolate unit-test SQL per model and reserve helper names ([#986](https://github.com/chio-labs/sqlbuild/issues/986)) ([36b5fc8](https://github.com/chio-labs/sqlbuild/commit/36b5fc83ff1cdfe579431a3e9a1add2fefd6936d))

## [0.136.1](https://github.com/chio-labs/sqlbuild/compare/v0.136.0...v0.136.1) (2026-10-06)


### Bug Fixes

* **scenario:** honour promotion mode and run scenarios concurrently ([#984](https://github.com/chio-labs/sqlbuild/issues/984)) ([b625100](https://github.com/chio-labs/sqlbuild/commit/b62510038de0fa1cce22d300212b7a865bb59cfd))

## [0.136.0](https://github.com/chio-labs/sqlbuild/compare/v0.135.0...v0.136.0) (2026-10-06)


### Features

* shorter CLI forms for selection, global flags, JSON output, scope targets and cursors ([#980](https://github.com/chio-labs/sqlbuild/issues/980)) ([e9a7aa1](https://github.com/chio-labs/sqlbuild/commit/e9a7aa1fea71ddc8f5e94e6001b5fb5c06f6a307))


### Performance Improvements

* **rules:** cut custom-rule host overhead ([#981](https://github.com/chio-labs/sqlbuild/issues/981)) ([7000d36](https://github.com/chio-labs/sqlbuild/commit/7000d364f06b667c8c4a741fd5d252d54b9b9b85))

## [0.135.0](https://github.com/chio-labs/sqlbuild/compare/v0.134.9...v0.135.0) (2026-10-06)


### Features

* default sqb diff to the active target and a size-guarded full comparison ([#976](https://github.com/chio-labs/sqlbuild/issues/976)) ([92cb04e](https://github.com/chio-labs/sqlbuild/commit/92cb04eee890eb6777f354b1e8af2659ff63bd47))

## [0.134.9](https://github.com/chio-labs/sqlbuild/compare/v0.134.8...v0.134.9) (2026-10-06)


### Bug Fixes

* make sqb lineage use the shared project selector parser ([#972](https://github.com/chio-labs/sqlbuild/issues/972)) ([306eceb](https://github.com/chio-labs/sqlbuild/commit/306ecebe853cb0804052cecd6e58539ce506fc50))

## [0.134.8](https://github.com/chio-labs/sqlbuild/compare/v0.134.7...v0.134.8) (2026-10-06)


### Bug Fixes

* **incremental:** reject cursor models with no inputs and allow empty cursor inputs ([#965](https://github.com/chio-labs/sqlbuild/issues/965)) ([a4b86c6](https://github.com/chio-labs/sqlbuild/commit/a4b86c6e17bfa1819739f3a71d92bc07d7c203e8))
* make model: and column: prefixes optional in sqb rename and sqb mv ([#970](https://github.com/chio-labs/sqlbuild/issues/970)) ([0b11ca6](https://github.com/chio-labs/sqlbuild/commit/0b11ca63abadede9b0db4296cf45f3647fd909f3))
* show statement and step progress rows only with --debug ([#967](https://github.com/chio-labs/sqlbuild/issues/967)) ([7e2fdd8](https://github.com/chio-labs/sqlbuild/commit/7e2fdd8f872d394ba8e5896c14e637170b29aea3))
* **snowflake:** create state tables permanent with maximum time travel ([#964](https://github.com/chio-labs/sqlbuild/issues/964)) ([1293e5d](https://github.com/chio-labs/sqlbuild/commit/1293e5df0874b0d02d1dd1e5fb9685fa0795f6de))


### Performance Improvements

* **compile:** reuse discovery, analysis and writes after model-only edits ([#971](https://github.com/chio-labs/sqlbuild/issues/971)) ([372e0c1](https://github.com/chio-labs/sqlbuild/commit/372e0c1da8fefcfb383f8c441cc7cd6e5d8f2399))

## [0.134.7](https://github.com/chio-labs/sqlbuild/compare/v0.134.6...v0.134.7) (2026-10-06)


### Bug Fixes

* no traceback after failed builds, and aligned test statuses ([#962](https://github.com/chio-labs/sqlbuild/issues/962)) ([12ce749](https://github.com/chio-labs/sqlbuild/commit/12ce749bb485c0255f21b4449a21254164605020))
* singular file counts in sqb mv and sqb rename, plain cursor bounds wording in sqb plan ([#961](https://github.com/chio-labs/sqlbuild/issues/961)) ([ea7c27b](https://github.com/chio-labs/sqlbuild/commit/ea7c27b36048130b224938c241483c40cb93ef79))

## [0.134.6](https://github.com/chio-labs/sqlbuild/compare/v0.134.5...v0.134.6) (2026-10-06)


### Performance Improvements

* **compile:** analyze models as soon as their producers finish ([#953](https://github.com/chio-labs/sqlbuild/issues/953)) ([84b50fb](https://github.com/chio-labs/sqlbuild/commit/84b50fb7fa43b0cfb20bcda60ad6b1980f1483f9))
* **compile:** batch analysis preparation normalization ([#957](https://github.com/chio-labs/sqlbuild/issues/957)) ([9a98b2f](https://github.com/chio-labs/sqlbuild/commit/9a98b2f5aa66fdd9e99397c0e83e2f2384776364))
* **compile:** reuse unaffected model renders after model-only edits ([#955](https://github.com/chio-labs/sqlbuild/issues/955)) ([d576523](https://github.com/chio-labs/sqlbuild/commit/d576523acb0620e5b278db1ae70d775d3cdf15ec))

## [0.134.5](https://github.com/chio-labs/sqlbuild/compare/v0.134.4...v0.134.5) (2026-10-05)


### Performance Improvements

* **compile:** reuse the previous compile when no input changed ([#951](https://github.com/chio-labs/sqlbuild/issues/951)) ([0b86fac](https://github.com/chio-labs/sqlbuild/commit/0b86facfcfc28ec4709a095faaad91ed7bf4064a))

## [0.134.4](https://github.com/chio-labs/sqlbuild/compare/v0.134.3...v0.134.4) (2026-10-05)


### Performance Improvements

* **runtime:** chunk captured non-ASCII output on UTF-8 byte boundaries ([#949](https://github.com/chio-labs/sqlbuild/issues/949)) ([e697cd9](https://github.com/chio-labs/sqlbuild/commit/e697cd9ebb7b5fc23e6e87860389ade9c4451edf))

## [0.134.3](https://github.com/chio-labs/sqlbuild/compare/v0.134.2...v0.134.3) (2026-10-05)


### Performance Improvements

* **cli:** load command code only after argument parsing ([#947](https://github.com/chio-labs/sqlbuild/issues/947)) ([8fced25](https://github.com/chio-labs/sqlbuild/commit/8fced258cc141fa475c9af8821f7faa488d2e3d5))

## [0.134.2](https://github.com/chio-labs/sqlbuild/compare/v0.134.1...v0.134.2) (2026-10-04)


### Performance Improvements

* **compile:** keep command models out of the compile import path ([#945](https://github.com/chio-labs/sqlbuild/issues/945)) ([86ab18a](https://github.com/chio-labs/sqlbuild/commit/86ab18ae697e6a781e4f964cf0c81cf56096b1be))
* **rules:** evaluate built-in rules incrementally per model ([#939](https://github.com/chio-labs/sqlbuild/issues/939)) ([6ddd00b](https://github.com/chio-labs/sqlbuild/commit/6ddd00bd2200e20a191895d6796ca0d0afc7a618))

## [0.134.1](https://github.com/chio-labs/sqlbuild/compare/v0.134.0...v0.134.1) (2026-10-04)


### Bug Fixes

* **config:** use the query warehouse for standalone test, scenario and janitor ([#942](https://github.com/chio-labs/sqlbuild/issues/942)) ([311d954](https://github.com/chio-labs/sqlbuild/commit/311d95460ace05463b3f4961192c82e84ee3b13c))

## [0.134.0](https://github.com/chio-labs/sqlbuild/compare/v0.133.6...v0.134.0) (2026-10-04)


### Features

* **config:** select default warehouses per command group ([#941](https://github.com/chio-labs/sqlbuild/issues/941)) ([e0d0706](https://github.com/chio-labs/sqlbuild/commit/e0d0706566bdd1d27ef0038e91435d0441075acc))
* **rules:** repeated JSON parse rule and allow_exceptions setting ([#936](https://github.com/chio-labs/sqlbuild/issues/936)) ([f16a20a](https://github.com/chio-labs/sqlbuild/commit/f16a20af414f74a97f779fb058ca246a0ea0afcf))


### Performance Improvements

* **compile:** lower rules-phase peak memory ([#937](https://github.com/chio-labs/sqlbuild/issues/937)) ([6d35706](https://github.com/chio-labs/sqlbuild/commit/6d35706909caf095517b7cd0de504c7189f94b1a))
* **rules:** check custom-rule module state with one shared snapshot per call ([#940](https://github.com/chio-labs/sqlbuild/issues/940)) ([013e687](https://github.com/chio-labs/sqlbuild/commit/013e687cb6a84f1f592d54d7c7e43ae397de5098))

## [0.133.6](https://github.com/chio-labs/sqlbuild/compare/v0.133.5...v0.133.6) (2026-10-04)


### Performance Improvements

* **compile:** keep native analysis parallel at 10k models ([#934](https://github.com/chio-labs/sqlbuild/issues/934)) ([ad29d02](https://github.com/chio-labs/sqlbuild/commit/ad29d020c748afb98f276dad4911547dfcb7c4ce))

## [0.133.5](https://github.com/chio-labs/sqlbuild/compare/v0.133.4...v0.133.5) (2026-10-04)


### Performance Improvements

* **compile:** read SQL-test relation markers from the parse tree ([#932](https://github.com/chio-labs/sqlbuild/issues/932)) ([d80af81](https://github.com/chio-labs/sqlbuild/commit/d80af8168155f958f133b68a1363c8c0ca8581e6))

## [0.133.4](https://github.com/chio-labs/sqlbuild/compare/v0.133.3...v0.133.4) (2026-10-04)


### Performance Improvements

* **compile:** faster discovery, scope visibility and warm writes ([#930](https://github.com/chio-labs/sqlbuild/issues/930)) ([3a12e4e](https://github.com/chio-labs/sqlbuild/commit/3a12e4e75fc207a210c2fd1d1aacd57587c69e74))

## [0.133.3](https://github.com/chio-labs/sqlbuild/compare/v0.133.2...v0.133.3) (2026-10-04)


### Performance Improvements

* **rules:** run custom rules across host processes ([#928](https://github.com/chio-labs/sqlbuild/issues/928)) ([ee6ea4f](https://github.com/chio-labs/sqlbuild/commit/ee6ea4f26572938e08b1d0b0f366f78fc4fdf3f8))

## [0.133.2](https://github.com/chio-labs/sqlbuild/compare/v0.133.1...v0.133.2) (2026-10-03)


### Performance Improvements

* **compile:** plan SQL tests in the background in every mode ([#926](https://github.com/chio-labs/sqlbuild/issues/926)) ([9f94ebd](https://github.com/chio-labs/sqlbuild/commit/9f94ebdcc1081d3f2534319dc032c7e14560a806))

## [0.133.1](https://github.com/chio-labs/sqlbuild/compare/v0.133.0...v0.133.1) (2026-10-03)


### Bug Fixes

* **compile:** linear missing-description locations and comment-tolerant expected fixtures ([#924](https://github.com/chio-labs/sqlbuild/issues/924)) ([2d8fe1d](https://github.com/chio-labs/sqlbuild/commit/2d8fe1dbc5215e4f26ba701641ac2ccf41fcc2ed))

## [0.133.0](https://github.com/chio-labs/sqlbuild/compare/v0.132.1...v0.133.0) (2026-10-03)


### ⚠ BREAKING CHANGES

* **compile:** projects with undescribed resources no longer compile. SQBRTEST105 was replaced by P010 and must be removed from [rules] select and ignore selectors, which otherwise fail with C000.

### Features

* **compile:** require a description on every named resource ([#917](https://github.com/chio-labs/sqlbuild/issues/917)) ([2fafd63](https://github.com/chio-labs/sqlbuild/commit/2fafd63c7d985b329c059a323a3f16f2629fb7c5))


### Performance Improvements

* **compile:** faster cold compiles ([#922](https://github.com/chio-labs/sqlbuild/issues/922)) ([9311dda](https://github.com/chio-labs/sqlbuild/commit/9311dda90b2a69b190bf23827087e98b84bb52d8))
* **compile:** stop the unused early SQL lint and map lint lines once per file ([#919](https://github.com/chio-labs/sqlbuild/issues/919)) ([9fae2ec](https://github.com/chio-labs/sqlbuild/commit/9fae2ec9c36431f5bf16e7f86cd8a37af894af1b))
* pin polyglot with faster column qualification ([#923](https://github.com/chio-labs/sqlbuild/issues/923)) ([cbe710d](https://github.com/chio-labs/sqlbuild/commit/cbe710d2a6092e3ebe3048c83fed8683355dc16c))

## [0.132.1](https://github.com/chio-labs/sqlbuild/compare/v0.132.0...v0.132.1) (2026-10-02)


### Bug Fixes

* keep set operations after subquery operands at the outer query ([#914](https://github.com/chio-labs/sqlbuild/issues/914)) ([d8d9507](https://github.com/chio-labs/sqlbuild/commit/d8d950779515cd1de3f836bea32bfad736f582da))


### Performance Improvements

* **compile:** reuse rule results on warm and one-edit compiles ([#915](https://github.com/chio-labs/sqlbuild/issues/915)) ([01a2bbd](https://github.com/chio-labs/sqlbuild/commit/01a2bbd7e6dcbc905a776c7111c6eca506bf5368))

## [0.132.0](https://github.com/chio-labs/sqlbuild/compare/v0.131.0...v0.132.0) (2026-10-02)


### ⚠ BREAKING CHANGES

* **compiler:** unknown on_schema_change or replay_on_change values and unknown dlt_sources keys now fail compilation.

### Bug Fixes

* **compiler:** reject unknown schema-change values and follow dialect quoting in compile scanners ([#912](https://github.com/chio-labs/sqlbuild/issues/912)) ([db5cd56](https://github.com/chio-labs/sqlbuild/commit/db5cd56ab3c173c37e860b0a33db6fe468c489ec))

## [0.131.0](https://github.com/chio-labs/sqlbuild/compare/v0.130.1...v0.131.0) (2026-10-02)


### ⚠ BREAKING CHANGES

* **rules:** check audit, test and hook SQL and enforce SQL quality rules ([#910](https://github.com/chio-labs/sqlbuild/issues/910))

### Features

* **rules:** check audit, test and hook SQL and enforce SQL quality rules ([#910](https://github.com/chio-labs/sqlbuild/issues/910)) ([723a25a](https://github.com/chio-labs/sqlbuild/commit/723a25a7fcc8690e9ffb0d8812ea4b31c58e0216))

## [0.130.1](https://github.com/chio-labs/sqlbuild/compare/v0.130.0...v0.130.1) (2026-10-01)


### Bug Fixes

* **planner:** observe source freshness per source and fail plans on missing source tables ([#908](https://github.com/chio-labs/sqlbuild/issues/908)) ([d79e9b7](https://github.com/chio-labs/sqlbuild/commit/d79e9b736e8ad0686309b9b6d50905d050d6b833))

## [0.130.0](https://github.com/chio-labs/sqlbuild/compare/v0.129.1...v0.130.0) (2026-10-01)


### ⚠ BREAKING CHANGES

* **compiler:** unknown keys in MODEL() headers, project defaults, path defaults and sources files now fail compilation.

### Bug Fixes

* **compiler:** reject unknown config keys and keep scenario SQL as authored ([#906](https://github.com/chio-labs/sqlbuild/issues/906)) ([e160823](https://github.com/chio-labs/sqlbuild/commit/e1608238dccefdef0fa9141ed3ae65a7cc6ecb64))

## [0.129.1](https://github.com/chio-labs/sqlbuild/compare/v0.129.0...v0.129.1) (2026-10-01)


### Performance Improvements

* **planner:** read fingerprint state once and keep plan generation linear ([#904](https://github.com/chio-labs/sqlbuild/issues/904)) ([3b5f131](https://github.com/chio-labs/sqlbuild/commit/3b5f131af759c34ff457ea8a6f296f2cd80d0503))

## [0.129.0](https://github.com/chio-labs/sqlbuild/compare/v0.128.0...v0.129.0) (2026-10-01)


### ⚠ BREAKING CHANGES

* **format:** layout-only token formatter with canonical SQL fingerprints ([#902](https://github.com/chio-labs/sqlbuild/issues/902))

### Features

* **format:** layout-only token formatter with canonical SQL fingerprints ([#902](https://github.com/chio-labs/sqlbuild/issues/902)) ([9d81b0f](https://github.com/chio-labs/sqlbuild/commit/9d81b0f55ac80fc31bae6efa3cb4d337a64845fa))

## [0.128.0](https://github.com/chio-labs/sqlbuild/compare/v0.127.2...v0.128.0) (2026-10-01)


### Features

* **cli:** add janitor --target and inspection-only --as preview ([#898](https://github.com/chio-labs/sqlbuild/issues/898)) ([9e7cff5](https://github.com/chio-labs/sqlbuild/commit/9e7cff5a43531b3d36b18f87a67cd908b4122d4c))

## [0.127.2](https://github.com/chio-labs/sqlbuild/compare/v0.127.1...v0.127.2) (2026-10-01)


### Performance Improvements

* **snowflake:** plan from SHOW metadata and run inspection reads concurrently ([#896](https://github.com/chio-labs/sqlbuild/issues/896)) ([f7c8faf](https://github.com/chio-labs/sqlbuild/commit/f7c8fafb451491406b4b328922885f03bbfdde59))

## [0.127.1](https://github.com/chio-labs/sqlbuild/compare/v0.127.0...v0.127.1) (2026-10-01)


### Bug Fixes

* **test:** run model SQL in SQL tests exactly as build sends it ([#893](https://github.com/chio-labs/sqlbuild/issues/893)) ([b6cbd5f](https://github.com/chio-labs/sqlbuild/commit/b6cbd5f537c1d0613b9e6bd40d792b369425bd0e))

## [0.127.0](https://github.com/chio-labs/sqlbuild/compare/v0.126.6...v0.127.0) (2026-09-30)


### Features

* **cli:** add sqb rename and sqb mv refactoring commands ([#891](https://github.com/chio-labs/sqlbuild/issues/891)) ([38029bf](https://github.com/chio-labs/sqlbuild/commit/38029bf6e9cb07453dff0530f338582081325b9d))

## [0.126.6](https://github.com/chio-labs/sqlbuild/compare/v0.126.5...v0.126.6) (2026-09-30)


### Bug Fixes

* **cli:** return native allocator memory within 10 ms by default ([#890](https://github.com/chio-labs/sqlbuild/issues/890)) ([e60652b](https://github.com/chio-labs/sqlbuild/commit/e60652b3b90fab6ff7aba0d172b71a1977d90440))


### Performance Improvements

* **compile:** close the fresh compile gap with ordered native scheduling and mimalloc ([#889](https://github.com/chio-labs/sqlbuild/issues/889)) ([eb9af35](https://github.com/chio-labs/sqlbuild/commit/eb9af359669bd0b7cd9415dea43c86805b5bae65))
* **compile:** reuse per-file facts on warm compiles ([#886](https://github.com/chio-labs/sqlbuild/issues/886)) ([47831f8](https://github.com/chio-labs/sqlbuild/commit/47831f87465d52737ea49a1c22143e3d3ff056f5))

## [0.126.5](https://github.com/chio-labs/sqlbuild/compare/v0.126.4...v0.126.5) (2026-09-29)


### Performance Improvements

* **rules:** reuse compile front-end work in lint and SQL rules ([#884](https://github.com/chio-labs/sqlbuild/issues/884)) ([ba67825](https://github.com/chio-labs/sqlbuild/commit/ba67825db0d2020b9a6e2dc5572d3b6ae613632c))

## [0.126.4](https://github.com/chio-labs/sqlbuild/compare/v0.126.3...v0.126.4) (2026-09-29)


### Performance Improvements

* **compile:** cut cache-write overhead and fresh compile preparation cost ([#882](https://github.com/chio-labs/sqlbuild/issues/882)) ([aea4484](https://github.com/chio-labs/sqlbuild/commit/aea44842d030695e6e2e67977666fc63bc02ccd4))

## [0.126.3](https://github.com/chio-labs/sqlbuild/compare/v0.126.2...v0.126.3) (2026-09-29)


### Bug Fixes

* keep stub-named CTEs from capturing references and the layered benchmark compiling ([#880](https://github.com/chio-labs/sqlbuild/issues/880)) ([5adf6a4](https://github.com/chio-labs/sqlbuild/commit/5adf6a4e202b7a045ceea893de9ec1f88c09fd5d))

## [0.126.2](https://github.com/chio-labs/sqlbuild/compare/v0.126.1...v0.126.2) (2026-09-29)


### Performance Improvements

* **lineage:** analyse traced columns against only referenced relations ([#877](https://github.com/chio-labs/sqlbuild/issues/877)) ([9655a4e](https://github.com/chio-labs/sqlbuild/commit/9655a4e7d7e0f34db423e937cbd4cec4bbed0557))

## [0.126.1](https://github.com/chio-labs/sqlbuild/compare/v0.126.0...v0.126.1) (2026-09-29)


### Performance Improvements

* **plan:** cut plan and build planning time on large projects ([#875](https://github.com/chio-labs/sqlbuild/issues/875)) ([d6be114](https://github.com/chio-labs/sqlbuild/commit/d6be114589f75bc2d907a47938cd20aa157b8d2c))

## [0.126.0](https://github.com/chio-labs/sqlbuild/compare/v0.125.1...v0.126.0) (2026-09-28)


### Features

* **migrations:** keep old model names working with expiring compatibility views ([#871](https://github.com/chio-labs/sqlbuild/issues/871)) ([b67bea0](https://github.com/chio-labs/sqlbuild/commit/b67bea0914cb17772ed2002ca4b6cc14954c5c8f))

## [0.125.1](https://github.com/chio-labs/sqlbuild/compare/v0.125.0...v0.125.1) (2026-09-28)


### Bug Fixes

* **lineage:** expand each shared node once when rendering lineage trees ([#870](https://github.com/chio-labs/sqlbuild/issues/870)) ([78b4626](https://github.com/chio-labs/sqlbuild/commit/78b46264d9bf2e31d7294ef34694c1eb9b25f4ee))

## [0.125.0](https://github.com/chio-labs/sqlbuild/compare/v0.124.2...v0.125.0) (2026-09-28)


### Features

* enforce custom rule hermeticity at runtime ([#868](https://github.com/chio-labs/sqlbuild/issues/868)) ([5fdb062](https://github.com/chio-labs/sqlbuild/commit/5fdb062ab8b94aa5cf9d4b9ab1278e222ebb0381))

## [0.124.2](https://github.com/chio-labs/sqlbuild/compare/v0.124.1...v0.124.2) (2026-09-28)


### Bug Fixes

* run selected Python nodes against upstreams outside the run ([#864](https://github.com/chio-labs/sqlbuild/issues/864)) ([0d1a8b4](https://github.com/chio-labs/sqlbuild/commit/0d1a8b41b96e12a73d50d74397c5edb85b4fb0ae))

## [0.124.1](https://github.com/chio-labs/sqlbuild/compare/v0.124.0...v0.124.1) (2026-09-28)


### Bug Fixes

* small fixes for audits, selection, loads, microbatch cursors, headers and clones ([#862](https://github.com/chio-labs/sqlbuild/issues/862)) ([685e846](https://github.com/chio-labs/sqlbuild/commit/685e846f87822582931f69496512adaaadf12902))

## [0.124.0](https://github.com/chio-labs/sqlbuild/compare/v0.123.0...v0.124.0) (2026-09-28)


### Features

* **migrations:** preserve column data on renames in incremental models ([#860](https://github.com/chio-labs/sqlbuild/issues/860)) ([730d640](https://github.com/chio-labs/sqlbuild/commit/730d6401cc2d6ac952f8251c6700c12dc6bcc86a))

## [0.123.0](https://github.com/chio-labs/sqlbuild/compare/v0.122.0...v0.123.0) (2026-09-28)


### ⚠ BREAKING CHANGES

* macros that emit __ref(), __source() or __seed() in their output now fail compile; pass the reference in as a typed argument.

### Features

* require explicit, typed references in macros, hooks and Python nodes ([#858](https://github.com/chio-labs/sqlbuild/issues/858)) ([3b34bce](https://github.com/chio-labs/sqlbuild/commit/3b34bce3ff6c0e38b865cddd696147f687d5a5e3))

## [0.122.0](https://github.com/chio-labs/sqlbuild/compare/v0.121.0...v0.122.0) (2026-09-27)


### ⚠ BREAKING CHANGES

* audit files outside audits/generic/ and audits/singular/ no longer compile; single-model, source-only, seed-only, and resource-free singular audits fail with P004; top-level audits, schemas, and hooks used only under one folder must move to that folder's _sqlbuild/ roles; unknown seed YAML keys are rejected.

### Features

* scope audits, schemas and hooks, add seed audits and gate targets on attached audits ([#855](https://github.com/chio-labs/sqlbuild/issues/855)) ([8966898](https://github.com/chio-labs/sqlbuild/commit/8966898386ba56e650b734b4df88418f0c2be545))

## [0.121.0](https://github.com/chio-labs/sqlbuild/compare/v0.120.0...v0.121.0) (2026-09-27)


### ⚠ BREAKING CHANGES

* Projects run only in direct mode. Virtual commands, flags, configuration keys, state backends, and prepare_version hooks are removed. Concurrent microbatch event IDs changed; drop existing _sqlbuild_microbatches tables before the next concurrent run. Sequential runs never use this table.

### Features

* remove virtual data environments and validate models at compile time ([#852](https://github.com/chio-labs/sqlbuild/issues/852)) ([0964666](https://github.com/chio-labs/sqlbuild/commit/0964666bbcb66d2b4c55a958bf74633dd08fce59))

## [0.120.0](https://github.com/chio-labs/sqlbuild/compare/v0.119.5...v0.120.0) (2026-09-27)


### ⚠ BREAKING CHANGES

* Python nodes and their helper modules must live under python/ (for example python/tasks/orders.py, imported as python.tasks.orders). Python under tasks/, assets/, checks/, loaders/, factories/, libs/, dagster/, rivers_pipeline/, or a root definitions.py now fails compilation with D016. Python path selectors use python/... instead of tasks/, assets/, checks/, or loaders/.

### Features

* merge Python node folders into python/ ([#850](https://github.com/chio-labs/sqlbuild/issues/850)) ([cbaa787](https://github.com/chio-labs/sqlbuild/commit/cbaa78714a0bd9e8508d8d75d97c998c91c87bfc))

## [0.119.5](https://github.com/chio-labs/sqlbuild/compare/v0.119.4...v0.119.5) (2026-09-27)


### Bug Fixes

* close correctness gaps in Rules, SQL scanners and sqb query ([#848](https://github.com/chio-labs/sqlbuild/issues/848)) ([0fe3fa5](https://github.com/chio-labs/sqlbuild/commit/0fe3fa5cbf209292fafb952f4830ba5fecde4b6a))

## [0.119.4](https://github.com/chio-labs/sqlbuild/compare/v0.119.3...v0.119.4) (2026-09-26)


### Bug Fixes

* keep authored SQL tokens when formatting ([#846](https://github.com/chio-labs/sqlbuild/issues/846)) ([0eb4bb7](https://github.com/chio-labs/sqlbuild/commit/0eb4bb7f0c8759f95d5354936f5e443722f95de1))

## [0.119.3](https://github.com/chio-labs/sqlbuild/compare/v0.119.2...v0.119.3) (2026-09-26)


### Bug Fixes

* qualify Snowflake function discovery by database ([#842](https://github.com/chio-labs/sqlbuild/issues/842)) ([264f84c](https://github.com/chio-labs/sqlbuild/commit/264f84ccabffd68f1b5c66e87cf9db08f375aad4))

## [0.119.2](https://github.com/chio-labs/sqlbuild/compare/v0.119.1...v0.119.2) (2026-09-26)


### Bug Fixes

* colour plan migrations and scope output and stop progress lines merging ([#837](https://github.com/chio-labs/sqlbuild/issues/837)) ([87ac2ef](https://github.com/chio-labs/sqlbuild/commit/87ac2ef1181287c683abdfc74c47f9a75dcaeabd))

## [0.119.1](https://github.com/chio-labs/sqlbuild/compare/v0.119.0...v0.119.1) (2026-09-25)


### Bug Fixes

* preserve empty-fixture lint semantics after Unicode text ([#829](https://github.com/chio-labs/sqlbuild/issues/829)) ([3260499](https://github.com/chio-labs/sqlbuild/commit/3260499bbff2e3d347009419dd9ba4c0cd1f9622))


### Documentation

* add link preview images and show the hero animation on mobile ([#827](https://github.com/chio-labs/sqlbuild/issues/827)) ([0cb0df8](https://github.com/chio-labs/sqlbuild/commit/0cb0df871723eadf8bdc500e8509f345eeb40882))
* add the sqlbuild.com website with the docs at /docs ([#824](https://github.com/chio-labs/sqlbuild/issues/824)) ([68f79f8](https://github.com/chio-labs/sqlbuild/commit/68f79f8f565dc0c5a5278e7591583a6efca5b013))
* centre the link preview cards so square crops stay readable ([#828](https://github.com/chio-labs/sqlbuild/issues/828)) ([71e49ef](https://github.com/chio-labs/sqlbuild/commit/71e49ef90ad2a8c3294aefb9975e85eaaf845a68))

## [0.119.0](https://github.com/chio-labs/sqlbuild/compare/v0.118.3...v0.119.0) (2026-09-25)


### Features

* report unsafe formatting and add join-predicate and final-CTE rules ([#822](https://github.com/chio-labs/sqlbuild/issues/822)) ([cdc6a61](https://github.com/chio-labs/sqlbuild/commit/cdc6a61db862733db305ce43d313e9db72b884a3))

## [0.118.3](https://github.com/chio-labs/sqlbuild/compare/v0.118.2...v0.118.3) (2026-09-25)


### Bug Fixes

* use the current declaration layout in the playground ([#820](https://github.com/chio-labs/sqlbuild/issues/820)) ([6701700](https://github.com/chio-labs/sqlbuild/commit/67017002edcf21c03e97071083105d9ea677d070))

## [0.118.2](https://github.com/chio-labs/sqlbuild/compare/v0.118.1...v0.118.2) (2026-09-25)


### Bug Fixes

* recreate relations missing from the warehouse snapshot ([#818](https://github.com/chio-labs/sqlbuild/issues/818)) ([19b42fd](https://github.com/chio-labs/sqlbuild/commit/19b42fd058392d64692fce77060e59ee2cf2f5cd))

## [0.118.1](https://github.com/chio-labs/sqlbuild/compare/v0.118.0...v0.118.1) (2026-09-25)


### Bug Fixes

* align SQL scanners and remove obsolete implementations ([#815](https://github.com/chio-labs/sqlbuild/issues/815)) ([920a933](https://github.com/chio-labs/sqlbuild/commit/920a933ba7c2c6d4f612320d964e0a57504a61a1))

## [0.118.0](https://github.com/chio-labs/sqlbuild/compare/v0.117.1...v0.118.0) (2026-09-25)


### Features

* **migrations:** keep incremental and snapshot history when models are renamed ([#813](https://github.com/chio-labs/sqlbuild/issues/813)) ([427f4da](https://github.com/chio-labs/sqlbuild/commit/427f4dad2138bf6af49d604315807a7f8b82909a))

## [0.117.1](https://github.com/chio-labs/sqlbuild/compare/v0.117.0...v0.117.1) (2026-09-24)


### Bug Fixes

* consolidate shared behavior and correct snapshot histories ([#811](https://github.com/chio-labs/sqlbuild/issues/811)) ([a1c7168](https://github.com/chio-labs/sqlbuild/commit/a1c7168ded5063ea90062f787cb088bedf8c6b9c))

## [0.117.0](https://github.com/chio-labs/sqlbuild/compare/v0.116.0...v0.117.0) (2026-09-24)


### Features

* **janitor:** archive stale relations before deletion in direct mode ([#809](https://github.com/chio-labs/sqlbuild/issues/809)) ([693e580](https://github.com/chio-labs/sqlbuild/commit/693e58054601db59c0a2e4059a32a37dc354219d))

## [0.116.0](https://github.com/chio-labs/sqlbuild/compare/v0.115.0...v0.116.0) (2026-09-24)


### Features

* **dupscore:** ignore forced adapter overrides and compare call targets ([#807](https://github.com/chio-labs/sqlbuild/issues/807)) ([5f0dbb5](https://github.com/chio-labs/sqlbuild/commit/5f0dbb5599c0d9b8327fb38dcbc3075d6b01cd26))


### Documentation

* add missing 0.113.5 changelog entry for the PIVOT lineage fix ([#805](https://github.com/chio-labs/sqlbuild/issues/805)) ([adb9248](https://github.com/chio-labs/sqlbuild/commit/adb9248105df0137663df40387ca4f73de4b51e3))

## [0.115.0](https://github.com/chio-labs/sqlbuild/compare/v0.114.2...v0.115.0) (2026-09-24)


### Features

* reject empty-input-only SQL tests and fix SQL test helper scope ([#803](https://github.com/chio-labs/sqlbuild/issues/803)) ([b1410f6](https://github.com/chio-labs/sqlbuild/commit/b1410f6aa7cfbf8cc5dc14132e8b912e54194d45))

## [0.114.2](https://github.com/chio-labs/sqlbuild/compare/v0.114.1...v0.114.2) (2026-09-24)


### Bug Fixes

* preserve full-replay history and improve SQL test planning ([#801](https://github.com/chio-labs/sqlbuild/issues/801)) ([d616759](https://github.com/chio-labs/sqlbuild/commit/d616759e95b40d75418f17f3f1614ca29752db12))

## [0.114.1](https://github.com/chio-labs/sqlbuild/compare/v0.114.0...v0.114.1) (2026-09-23)


### Performance Improvements

* focused agent skill and batched Snowflake retention inspection ([#798](https://github.com/chio-labs/sqlbuild/issues/798)) ([2a5a54e](https://github.com/chio-labs/sqlbuild/commit/2a5a54e54179d57cd72886ca2e9a3a1fb0f03542))

## [0.114.0](https://github.com/chio-labs/sqlbuild/compare/v0.113.9...v0.114.0) (2026-09-23)


### Features

* deny time travel retention decreases by default and support per-materialization target retention ([#795](https://github.com/chio-labs/sqlbuild/issues/795)) ([2eea4d9](https://github.com/chio-labs/sqlbuild/commit/2eea4d92d2446d6188599e8e7ae4a79953acdbc7))


### Performance Improvements

* plan SQL tests during warehouse inspection and skip them without tests ([#796](https://github.com/chio-labs/sqlbuild/issues/796)) ([6f1a086](https://github.com/chio-labs/sqlbuild/commit/6f1a0861fbf41c9fd317a21c0d9512532db8dc63))

## [0.113.9](https://github.com/chio-labs/sqlbuild/compare/v0.113.8...v0.113.9) (2026-09-23)


### Bug Fixes

* skip target retention defaults for views ([#793](https://github.com/chio-labs/sqlbuild/issues/793)) ([3d1dd48](https://github.com/chio-labs/sqlbuild/commit/3d1dd48d0388302977dfb994bfa639c42660ab7b))

## [0.113.8](https://github.com/chio-labs/sqlbuild/compare/v0.113.7...v0.113.8) (2026-09-23)


### Bug Fixes

* **format:** make sqb format safe and idempotent ([#791](https://github.com/chio-labs/sqlbuild/issues/791)) ([041a264](https://github.com/chio-labs/sqlbuild/commit/041a2646a2b787084d5f89412b9ade639e90cb88))

## [0.113.7](https://github.com/chio-labs/sqlbuild/compare/v0.113.6...v0.113.7) (2026-09-23)


### Bug Fixes

* faster Snowflake table-type conversion and accurate schema diffs ([#789](https://github.com/chio-labs/sqlbuild/issues/789)) ([a7a7e08](https://github.com/chio-labs/sqlbuild/commit/a7a7e082fa44cc9ba9eeb7905b2939aa35256796))

## [0.113.6](https://github.com/chio-labs/sqlbuild/compare/v0.113.5...v0.113.6) (2026-09-23)


### Performance Improvements

* reuse compiler declaration scope in warm SQL rules ([#787](https://github.com/chio-labs/sqlbuild/issues/787)) ([3304b9d](https://github.com/chio-labs/sqlbuild/commit/3304b9db14be34c7890c748d996ab696ee174ba4))

## [0.113.5](https://github.com/chio-labs/sqlbuild/compare/v0.113.4...v0.113.5) (2026-09-23)


### Bug Fixes

* upgrade Polyglot to 0.12.3 for PIVOT and UNPIVOT star lineage ([#784](https://github.com/chio-labs/sqlbuild/issues/784)) ([f8d4481](https://github.com/chio-labs/sqlbuild/commit/f8d448145002137651609f1b32d602a66287cf06))


### Performance Improvements

* scale declaration scope resolution with many scoped macros ([#782](https://github.com/chio-labs/sqlbuild/issues/782)) ([eb21a7a](https://github.com/chio-labs/sqlbuild/commit/eb21a7a29b553d3ac75ccaeffb9c13a12ea8789f))

## [0.113.4](https://github.com/chio-labs/sqlbuild/compare/v0.113.3...v0.113.4) (2026-09-22)


### Performance Improvements

* speed up warm compiles and upgrade Polyglot to 0.12.2 ([#779](https://github.com/chio-labs/sqlbuild/issues/779)) ([ca25236](https://github.com/chio-labs/sqlbuild/commit/ca25236bb7f8862343c0f38ae0ba8c2277859065))

## [0.113.3](https://github.com/chio-labs/sqlbuild/compare/v0.113.2...v0.113.3) (2026-09-22)


### Performance Improvements

* integrate conservative cold compiler optimizations ([#776](https://github.com/chio-labs/sqlbuild/issues/776)) ([0356503](https://github.com/chio-labs/sqlbuild/commit/0356503996f22c6b2b7e5cb4e3f65e9691d4588b))

## [0.113.2](https://github.com/chio-labs/sqlbuild/compare/v0.113.1...v0.113.2) (2026-09-22)


### Bug Fixes

* **rules:** enforce semantic model layers ([#774](https://github.com/chio-labs/sqlbuild/issues/774)) ([6225ee3](https://github.com/chio-labs/sqlbuild/commit/6225ee36d50129457c6aabebd933820a3494f122))

## [0.113.1](https://github.com/chio-labs/sqlbuild/compare/v0.113.0...v0.113.1) (2026-09-22)


### Performance Improvements

* **compiler:** bound cold analysis and tighten regression budgets ([#772](https://github.com/chio-labs/sqlbuild/issues/772)) ([9f248e6](https://github.com/chio-labs/sqlbuild/commit/9f248e672b52cd7f0abb1647d7c1b68ca1385ff1))

## [0.113.0](https://github.com/chio-labs/sqlbuild/compare/v0.112.2...v0.113.0) (2026-09-21)


### Features

* **observability:** expose microbatch lifecycle bounds ([#770](https://github.com/chio-labs/sqlbuild/issues/770)) ([1470abb](https://github.com/chio-labs/sqlbuild/commit/1470abb46b297184e7fcdf92686a1d75822c8405))

## [0.112.2](https://github.com/chio-labs/sqlbuild/compare/v0.112.1...v0.112.2) (2026-09-21)


### Performance Improvements

* accelerate no-cache cold compiles ([#768](https://github.com/chio-labs/sqlbuild/issues/768)) ([794b9dd](https://github.com/chio-labs/sqlbuild/commit/794b9dd1d973a3fc86db45a8eea76696aa21a3ab))

## [0.112.1](https://github.com/chio-labs/sqlbuild/compare/v0.112.0...v0.112.1) (2026-09-21)


### Performance Improvements

* accelerate warm incremental compiles ([#766](https://github.com/chio-labs/sqlbuild/issues/766)) ([82e353d](https://github.com/chio-labs/sqlbuild/commit/82e353d4fc539faf5d9f9f6f3688621ef58633ee))

## [0.112.0](https://github.com/chio-labs/sqlbuild/compare/v0.111.1...v0.112.0) (2026-09-21)


### Features

* **format:** accelerate selected test formatting ([#764](https://github.com/chio-labs/sqlbuild/issues/764)) ([1782e00](https://github.com/chio-labs/sqlbuild/commit/1782e00ce21d4b12850f66fb266b4534dd5765e2))

## [0.111.1](https://github.com/chio-labs/sqlbuild/compare/v0.111.0...v0.111.1) (2026-09-21)


### Performance Improvements

* **compiler:** reduce compile memory and latency ([#758](https://github.com/chio-labs/sqlbuild/issues/758)) ([359ef00](https://github.com/chio-labs/sqlbuild/commit/359ef002d6ec295785e4a19f48f90e5f14058151))


### Documentation

* clarify auto-merge ownership ([#762](https://github.com/chio-labs/sqlbuild/issues/762)) ([1891652](https://github.com/chio-labs/sqlbuild/commit/1891652e80e9ecbc64c74d9a5bf1daffcb2cfee7))

## [0.111.0](https://github.com/chio-labs/sqlbuild/compare/v0.110.7...v0.111.0) (2026-09-21)


### Features

* improve Dagster checks and test fixtures ([#759](https://github.com/chio-labs/sqlbuild/issues/759)) ([a5923fa](https://github.com/chio-labs/sqlbuild/commit/a5923fa7948ae53802e762f1895dbb77ab48e60f))

## [0.110.7](https://github.com/chio-labs/sqlbuild/compare/v0.110.6...v0.110.7) (2026-09-20)


### Bug Fixes

* batch clone fingerprint finalization ([#756](https://github.com/chio-labs/sqlbuild/issues/756)) ([20eab8f](https://github.com/chio-labs/sqlbuild/commit/20eab8f16c78f49a55187e0904c1732c844eee13))

## [0.110.6](https://github.com/chio-labs/sqlbuild/compare/v0.110.5...v0.110.6) (2026-09-20)


### Bug Fixes

* type null-only SQL test fixtures ([#754](https://github.com/chio-labs/sqlbuild/issues/754)) ([7ca2da3](https://github.com/chio-labs/sqlbuild/commit/7ca2da372e9d04fdcffab6a574705c6a3834aa2e))

## [0.110.5](https://github.com/chio-labs/sqlbuild/compare/v0.110.4...v0.110.5) (2026-09-20)


### Bug Fixes

* align configured rules with selected analysis ([#752](https://github.com/chio-labs/sqlbuild/issues/752)) ([956e993](https://github.com/chio-labs/sqlbuild/commit/956e993bfbd4189174ce32af0fbd7e4d74718fc6))

## [0.110.4](https://github.com/chio-labs/sqlbuild/compare/v0.110.3...v0.110.4) (2026-09-20)


### Bug Fixes

* **planner:** avoid duplicate planning work ([#750](https://github.com/chio-labs/sqlbuild/issues/750)) ([0219c74](https://github.com/chio-labs/sqlbuild/commit/0219c744bdf89bc63bd4a7f879a8dc2a2ce403c0))

## [0.110.3](https://github.com/chio-labs/sqlbuild/compare/v0.110.2...v0.110.3) (2026-09-20)


### Bug Fixes

* reduce large-project planning work ([#748](https://github.com/chio-labs/sqlbuild/issues/748)) ([c553f54](https://github.com/chio-labs/sqlbuild/commit/c553f54f36f71f076043191998cbebe391719acd))

## [0.110.2](https://github.com/chio-labs/sqlbuild/compare/v0.110.1...v0.110.2) (2026-09-19)


### Performance Improvements

* reduce semantic compile analysis cost ([#746](https://github.com/chio-labs/sqlbuild/issues/746)) ([107ab8f](https://github.com/chio-labs/sqlbuild/commit/107ab8fadefff7b68d76c2892f37692ec7e859cf))

## [0.110.1](https://github.com/chio-labs/sqlbuild/compare/v0.110.0...v0.110.1) (2026-09-19)


### Bug Fixes

* harden SQL analysis for complex projects ([#744](https://github.com/chio-labs/sqlbuild/issues/744)) ([b373c93](https://github.com/chio-labs/sqlbuild/commit/b373c938fa39f0a164ae423accd0d618b1dd7025))

## [0.110.0](https://github.com/chio-labs/sqlbuild/compare/v0.109.0...v0.110.0) (2026-09-18)


### Features

* add target execution limits ([#742](https://github.com/chio-labs/sqlbuild/issues/742)) ([02f0e18](https://github.com/chio-labs/sqlbuild/commit/02f0e181390e127891cb495227a00d1f69bb9fc8))

## [0.109.0](https://github.com/chio-labs/sqlbuild/compare/v0.108.1...v0.109.0) (2026-09-18)


### Features

* support table function fixtures in model tests ([#740](https://github.com/chio-labs/sqlbuild/issues/740)) ([d09b53a](https://github.com/chio-labs/sqlbuild/commit/d09b53a110f423b88d034559cd791516b06d59f0))

## [0.108.1](https://github.com/chio-labs/sqlbuild/compare/v0.108.0...v0.108.1) (2026-09-18)


### Bug Fixes

* distinguish Snowflake START WITH from CTEs ([#738](https://github.com/chio-labs/sqlbuild/issues/738)) ([b39bd91](https://github.com/chio-labs/sqlbuild/commit/b39bd916d2189dc85e8817c0892d31893472ef3a))

## [0.108.0](https://github.com/chio-labs/sqlbuild/compare/v0.107.0...v0.108.0) (2026-09-18)


### Features

* govern dynamic pivot column families ([#736](https://github.com/chio-labs/sqlbuild/issues/736)) ([0cdcb45](https://github.com/chio-labs/sqlbuild/commit/0cdcb456b8348f8ff32b6d921ab9ceae7a52a564))

## [0.107.0](https://github.com/chio-labs/sqlbuild/compare/v0.106.1...v0.107.0) (2026-09-18)


### Features

* polish raw query diff output ([#734](https://github.com/chio-labs/sqlbuild/issues/734)) ([04fc86f](https://github.com/chio-labs/sqlbuild/commit/04fc86fb4df760cbe18990362e15d4aa48b74242))

## [0.106.1](https://github.com/chio-labs/sqlbuild/compare/v0.106.0...v0.106.1) (2026-09-18)


### Bug Fixes

* evaluate authored numeric decisions structurally ([#732](https://github.com/chio-labs/sqlbuild/issues/732)) ([6753680](https://github.com/chio-labs/sqlbuild/commit/675368088271854ef5087012cba4742f99e530c8))

## [0.106.0](https://github.com/chio-labs/sqlbuild/compare/v0.105.0...v0.106.0) (2026-09-18)


### Features

* configure authoritative contract name types ([#730](https://github.com/chio-labs/sqlbuild/issues/730)) ([6887bbc](https://github.com/chio-labs/sqlbuild/commit/6887bbc940196bc84619b6e273141904f2ba9069))

## [0.105.0](https://github.com/chio-labs/sqlbuild/compare/v0.104.1...v0.105.0) (2026-09-18)


### Features

* add raw query diff inputs ([#728](https://github.com/chio-labs/sqlbuild/issues/728)) ([af7308c](https://github.com/chio-labs/sqlbuild/commit/af7308cd5e4dbb030209ed16ca6fa9ab0b85fc68))

## [0.104.1](https://github.com/chio-labs/sqlbuild/compare/v0.104.0...v0.104.1) (2026-09-18)


### Bug Fixes

* accept proven dependency wildcards ([#726](https://github.com/chio-labs/sqlbuild/issues/726)) ([cb5f01e](https://github.com/chio-labs/sqlbuild/commit/cb5f01e727edac5ccb1c69efb9a80dfbb783a178))

## [0.104.0](https://github.com/chio-labs/sqlbuild/compare/v0.103.7...v0.104.0) (2026-09-18)


### Features

* apply contract defaults to sources ([#724](https://github.com/chio-labs/sqlbuild/issues/724)) ([5ad3e97](https://github.com/chio-labs/sqlbuild/commit/5ad3e9736aadf401ec47033a5af71a723ffc4d73))

## [0.103.7](https://github.com/chio-labs/sqlbuild/compare/v0.103.6...v0.103.7) (2026-09-18)


### Bug Fixes

* inspect contracts through active connection ([#722](https://github.com/chio-labs/sqlbuild/issues/722)) ([b7a1260](https://github.com/chio-labs/sqlbuild/commit/b7a1260fd7e613224e441ce38111c49d9a846706))

## [0.103.6](https://github.com/chio-labs/sqlbuild/compare/v0.103.5...v0.103.6) (2026-09-18)


### Bug Fixes

* normalize contract adoption paths ([#720](https://github.com/chio-labs/sqlbuild/issues/720)) ([2c4e160](https://github.com/chio-labs/sqlbuild/commit/2c4e160d0a97154ad2fee15b106db9cb56537c6f))

## [0.103.5](https://github.com/chio-labs/sqlbuild/compare/v0.103.4...v0.103.5) (2026-09-18)


### Bug Fixes

* separate audit checks from schema nullability ([#718](https://github.com/chio-labs/sqlbuild/issues/718)) ([5616d37](https://github.com/chio-labs/sqlbuild/commit/5616d375adbb6de841bbb306ae1d3bb731979609))

## [0.103.4](https://github.com/chio-labs/sqlbuild/compare/v0.103.3...v0.103.4) (2026-09-18)


### Bug Fixes

* resolve Snowflake output aliases ([#716](https://github.com/chio-labs/sqlbuild/issues/716)) ([df68807](https://github.com/chio-labs/sqlbuild/commit/df688074a4cd11dcaedad76eb65e89d12d73a3bc))
* resolve Snowflake values columns ([#714](https://github.com/chio-labs/sqlbuild/issues/714)) ([02a3f98](https://github.com/chio-labs/sqlbuild/commit/02a3f9895e42def50d719a46cec9ca2e10b184a8))

## [0.103.3](https://github.com/chio-labs/sqlbuild/compare/v0.103.2...v0.103.3) (2026-09-18)


### Bug Fixes

* infer SQL predicate output types ([#713](https://github.com/chio-labs/sqlbuild/issues/713)) ([072b7f1](https://github.com/chio-labs/sqlbuild/commit/072b7f1e868c6ac43d1fcfa6325bf17e951ce1d4))

## [0.103.2](https://github.com/chio-labs/sqlbuild/compare/v0.103.1...v0.103.2) (2026-09-17)


### Bug Fixes

* refine nullability after SQL filters ([#711](https://github.com/chio-labs/sqlbuild/issues/711)) ([55c5a90](https://github.com/chio-labs/sqlbuild/commit/55c5a9042be479deecb07cf487d9de3021ea2362))

## [0.103.1](https://github.com/chio-labs/sqlbuild/compare/v0.103.0...v0.103.1) (2026-09-17)


### Bug Fixes

* use upstream Polyglot parser ([#709](https://github.com/chio-labs/sqlbuild/issues/709)) ([5fc1178](https://github.com/chio-labs/sqlbuild/commit/5fc11781a78f5a2396dc3fa74d7776da39448465))

## [0.103.0](https://github.com/chio-labs/sqlbuild/compare/v0.102.5...v0.103.0) (2026-09-17)


### Features

* require types on enforced contract columns ([#707](https://github.com/chio-labs/sqlbuild/issues/707)) ([1a0b340](https://github.com/chio-labs/sqlbuild/commit/1a0b34040451d3e027d0ed662937ea725352c397))

## [0.102.5](https://github.com/chio-labs/sqlbuild/compare/v0.102.4...v0.102.5) (2026-09-17)


### Bug Fixes

* ignore cast type parameters in literal rules ([#705](https://github.com/chio-labs/sqlbuild/issues/705)) ([2d2b8d5](https://github.com/chio-labs/sqlbuild/commit/2d2b8d5784f5fd14e3c70531f2484bfe66321367))

## [0.102.4](https://github.com/chio-labs/sqlbuild/compare/v0.102.3...v0.102.4) (2026-09-17)


### Bug Fixes

* map final CTE contract outputs by name ([#703](https://github.com/chio-labs/sqlbuild/issues/703)) ([83a3f53](https://github.com/chio-labs/sqlbuild/commit/83a3f5359ce1882c7d7ec459f8e0052e32a7e350))

## [0.102.3](https://github.com/chio-labs/sqlbuild/compare/v0.102.2...v0.102.3) (2026-09-17)


### Bug Fixes

* preserve cast type spellings in formatter ([#701](https://github.com/chio-labs/sqlbuild/issues/701)) ([5b0d301](https://github.com/chio-labs/sqlbuild/commit/5b0d301562c7545f216a9f6a1eee8f3238d54cd6))

## [0.102.2](https://github.com/chio-labs/sqlbuild/compare/v0.102.1...v0.102.2) (2026-09-17)


### Bug Fixes

* trust exact contract output casts ([#699](https://github.com/chio-labs/sqlbuild/issues/699)) ([3483ab3](https://github.com/chio-labs/sqlbuild/commit/3483ab38f07bf10acb98fc0d7fa873f0d83fb6f9))

## [0.102.1](https://github.com/chio-labs/sqlbuild/compare/v0.102.0...v0.102.1) (2026-09-17)


### Bug Fixes

* resolve fixture columns through CTEs ([#697](https://github.com/chio-labs/sqlbuild/issues/697)) ([09feed9](https://github.com/chio-labs/sqlbuild/commit/09feed94ff48b5b75b5c9acd5a3cd8e3af173936))

## [0.102.0](https://github.com/chio-labs/sqlbuild/compare/v0.101.16...v0.102.0) (2026-09-17)


### Features

* enforce explicit contract output types ([#695](https://github.com/chio-labs/sqlbuild/issues/695)) ([50f0fa3](https://github.com/chio-labs/sqlbuild/commit/50f0fa3709a96fb73cba208e09eb6ff015560dbd))

## [0.101.16](https://github.com/chio-labs/sqlbuild/compare/v0.101.15...v0.101.16) (2026-09-17)


### Bug Fixes

* preserve inferred types through CTEs ([#693](https://github.com/chio-labs/sqlbuild/issues/693)) ([222be68](https://github.com/chio-labs/sqlbuild/commit/222be68b0c75ee5585b74c71f7a9db67d428bb46))

## [0.101.15](https://github.com/chio-labs/sqlbuild/compare/v0.101.14...v0.101.15) (2026-09-17)


### Bug Fixes

* distinguish model column audit checks ([#691](https://github.com/chio-labs/sqlbuild/issues/691)) ([3b174d4](https://github.com/chio-labs/sqlbuild/commit/3b174d45389e587362fc7d1f881d209353a45b70))

## [0.101.14](https://github.com/chio-labs/sqlbuild/compare/v0.101.13...v0.101.14) (2026-09-16)


### Bug Fixes

* validate open relation fixtures ([#689](https://github.com/chio-labs/sqlbuild/issues/689)) ([a9563ac](https://github.com/chio-labs/sqlbuild/commit/a9563ac47e501ca062a5fbf3e2eaa29eae97e812))

## [0.101.13](https://github.com/chio-labs/sqlbuild/compare/v0.101.12...v0.101.13) (2026-09-16)


### Bug Fixes

* identify authored numeric literals ([#687](https://github.com/chio-labs/sqlbuild/issues/687)) ([237343f](https://github.com/chio-labs/sqlbuild/commit/237343f255b7a39c9133c4033bb70771b5657cf1))

## [0.101.12](https://github.com/chio-labs/sqlbuild/compare/v0.101.11...v0.101.12) (2026-09-16)


### Bug Fixes

* clarify project layer remediation ([#685](https://github.com/chio-labs/sqlbuild/issues/685)) ([1ee489c](https://github.com/chio-labs/sqlbuild/commit/1ee489cc962a48b4a48b444613f30f1b856dcd3b))

## [0.101.11](https://github.com/chio-labs/sqlbuild/compare/v0.101.10...v0.101.11) (2026-09-16)


### Bug Fixes

* allow irreducible output shapes ([#683](https://github.com/chio-labs/sqlbuild/issues/683)) ([172012a](https://github.com/chio-labs/sqlbuild/commit/172012a9703d70e4293a2ad409a177ef92f87161))

## [0.101.10](https://github.com/chio-labs/sqlbuild/compare/v0.101.9...v0.101.10) (2026-09-16)


### Bug Fixes

* preserve Snowflake cursor timestamp semantics ([#681](https://github.com/chio-labs/sqlbuild/issues/681)) ([8f9680a](https://github.com/chio-labs/sqlbuild/commit/8f9680ad622cefb7897b1a9c8ecdef4c5e6fc015))

## [0.101.9](https://github.com/chio-labs/sqlbuild/compare/v0.101.8...v0.101.9) (2026-09-16)


### Bug Fixes

* recover CTE types for typed models ([#679](https://github.com/chio-labs/sqlbuild/issues/679)) ([0d7aac1](https://github.com/chio-labs/sqlbuild/commit/0d7aac1b6ae1fd503eb9892e17534fbdfd3578f7))

## [0.101.8](https://github.com/chio-labs/sqlbuild/compare/v0.101.7...v0.101.8) (2026-09-16)


### Bug Fixes

* allow stars over dynamic pivots ([#677](https://github.com/chio-labs/sqlbuild/issues/677)) ([045d088](https://github.com/chio-labs/sqlbuild/commit/045d088cf9f223febcc7d7eccdae039cd370bf0a))

## [0.101.7](https://github.com/chio-labs/sqlbuild/compare/v0.101.6...v0.101.7) (2026-09-16)


### Bug Fixes

* preserve varchar cast lengths in inference ([#675](https://github.com/chio-labs/sqlbuild/issues/675)) ([a4bbf02](https://github.com/chio-labs/sqlbuild/commit/a4bbf0298d7b765258ef606d0f0ae7065eff9355))

## [0.101.6](https://github.com/chio-labs/sqlbuild/compare/v0.101.5...v0.101.6) (2026-09-16)


### Bug Fixes

* preserve explicit null ordering during format ([#673](https://github.com/chio-labs/sqlbuild/issues/673)) ([2c5d692](https://github.com/chio-labs/sqlbuild/commit/2c5d692827e61993150f9bd3b8f66b89826c86ca))

## [0.101.5](https://github.com/chio-labs/sqlbuild/compare/v0.101.4...v0.101.5) (2026-09-16)


### Bug Fixes

* ignore commented dependency calls in model rules ([#671](https://github.com/chio-labs/sqlbuild/issues/671)) ([2bfea0f](https://github.com/chio-labs/sqlbuild/commit/2bfea0f1acc809939fc6042742b0233303cfecf6))

## [0.101.4](https://github.com/chio-labs/sqlbuild/compare/v0.101.3...v0.101.4) (2026-09-16)


### Bug Fixes

* accept keyword-named terminal columns ([#669](https://github.com/chio-labs/sqlbuild/issues/669)) ([827d19c](https://github.com/chio-labs/sqlbuild/commit/827d19c0e2186950ddbbf1ebc5cfa343e101c525))

## [0.101.3](https://github.com/chio-labs/sqlbuild/compare/v0.101.2...v0.101.3) (2026-09-15)


### Bug Fixes

* preserve cast types through final CTEs ([#667](https://github.com/chio-labs/sqlbuild/issues/667)) ([0e45708](https://github.com/chio-labs/sqlbuild/commit/0e4570876306c20b1bf0edf7700c172b35095e07))

## [0.101.2](https://github.com/chio-labs/sqlbuild/compare/v0.101.1...v0.101.2) (2026-09-15)


### Bug Fixes

* accept spaced timezone cursor suffixes ([#664](https://github.com/chio-labs/sqlbuild/issues/664)) ([21d3ab4](https://github.com/chio-labs/sqlbuild/commit/21d3ab47c9b8178a86fddfdd8335fe6eb0ea27a1))

## [0.101.1](https://github.com/chio-labs/sqlbuild/compare/v0.101.0...v0.101.1) (2026-09-14)


### Bug Fixes

* ignore derived relations in scalar queries ([#662](https://github.com/chio-labs/sqlbuild/issues/662)) ([45ae3a4](https://github.com/chio-labs/sqlbuild/commit/45ae3a4920440c087fc35b458fa3a6b489ec00b8))

## [0.101.0](https://github.com/chio-labs/sqlbuild/compare/v0.100.8...v0.101.0) (2026-09-14)


### Features

* require named query relations ([#660](https://github.com/chio-labs/sqlbuild/issues/660)) ([327a4fa](https://github.com/chio-labs/sqlbuild/commit/327a4fa8df234a24ab68de6aa2378c2aa4ab77b4))

## [0.100.8](https://github.com/chio-labs/sqlbuild/compare/v0.100.7...v0.100.8) (2026-09-14)


### Bug Fixes

* allow stars in dependency imports ([#658](https://github.com/chio-labs/sqlbuild/issues/658)) ([924ba63](https://github.com/chio-labs/sqlbuild/commit/924ba63efaaddbb78b85a491f8dcb45d240eeaa8))

## [0.100.7](https://github.com/chio-labs/sqlbuild/compare/v0.100.6...v0.100.7) (2026-09-14)


### Bug Fixes

* respect SQL resource semantics ([#656](https://github.com/chio-labs/sqlbuild/issues/656)) ([4f7e327](https://github.com/chio-labs/sqlbuild/commit/4f7e3274c37e0ba0e131e1841f163a6346680481))

## [0.100.6](https://github.com/chio-labs/sqlbuild/compare/v0.100.5...v0.100.6) (2026-09-14)


### Bug Fixes

* allow ceremonial final CTE reads ([#654](https://github.com/chio-labs/sqlbuild/issues/654)) ([d620a74](https://github.com/chio-labs/sqlbuild/commit/d620a74bc960c8e4ebb0e0417a29272b45015e18))

## [0.100.5](https://github.com/chio-labs/sqlbuild/compare/v0.100.4...v0.100.5) (2026-09-13)


### Bug Fixes

* infer commented projection names ([#652](https://github.com/chio-labs/sqlbuild/issues/652)) ([08f04df](https://github.com/chio-labs/sqlbuild/commit/08f04dfc82190763363a0ec05a03db03eb3a2175))

## [0.100.4](https://github.com/chio-labs/sqlbuild/compare/v0.100.3...v0.100.4) (2026-09-13)


### Bug Fixes

* preserve framework CTE references ([#650](https://github.com/chio-labs/sqlbuild/issues/650)) ([37b8ed1](https://github.com/chio-labs/sqlbuild/commit/37b8ed100c9f0cc055dd629978b9578d1b56f061))

## [0.100.3](https://github.com/chio-labs/sqlbuild/compare/v0.100.2...v0.100.3) (2026-09-13)


### Bug Fixes

* isolate nested SQL rule contexts ([#648](https://github.com/chio-labs/sqlbuild/issues/648)) ([dad03f4](https://github.com/chio-labs/sqlbuild/commit/dad03f47c796012b4573d0cc2aedd23f58d0748f))

## [0.100.2](https://github.com/chio-labs/sqlbuild/compare/v0.100.1...v0.100.2) (2026-09-13)


### Bug Fixes

* preserve description escapes ([#646](https://github.com/chio-labs/sqlbuild/issues/646)) ([74595b8](https://github.com/chio-labs/sqlbuild/commit/74595b8d4c7c3d46305ee100f53fe79325d9c9cc))

## [0.100.1](https://github.com/chio-labs/sqlbuild/compare/v0.100.0...v0.100.1) (2026-09-13)


### Bug Fixes

* align scoped macro tests ([#644](https://github.com/chio-labs/sqlbuild/issues/644)) ([b2792c5](https://github.com/chio-labs/sqlbuild/commit/b2792c5d66d568f35bf4cb2378589eb2233e1462))

## [0.100.0](https://github.com/chio-labs/sqlbuild/compare/v0.99.0...v0.100.0) (2026-09-13)


### Features

* group scoped declarations ([#642](https://github.com/chio-labs/sqlbuild/issues/642)) ([ae586be](https://github.com/chio-labs/sqlbuild/commit/ae586bebba9dd6545dcba117b8284df6b0b01ac7))

## [0.99.0](https://github.com/chio-labs/sqlbuild/compare/v0.98.5...v0.99.0) (2026-09-13)


### Features

* wrap model descriptions ([#640](https://github.com/chio-labs/sqlbuild/issues/640)) ([2de9f15](https://github.com/chio-labs/sqlbuild/commit/2de9f1588a1a00b6d84bc851df442b50e77aff22))

## [0.98.5](https://github.com/chio-labs/sqlbuild/compare/v0.98.4...v0.98.5) (2026-09-12)


### Bug Fixes

* restrict redundant self alias fixes ([#638](https://github.com/chio-labs/sqlbuild/issues/638)) ([da2f968](https://github.com/chio-labs/sqlbuild/commit/da2f968042336232064ee480908c5e77b9b2f5ad))

## [0.98.4](https://github.com/chio-labs/sqlbuild/compare/v0.98.3...v0.98.4) (2026-09-12)


### Bug Fixes

* preserve scalar subquery scope ([#636](https://github.com/chio-labs/sqlbuild/issues/636)) ([988dce5](https://github.com/chio-labs/sqlbuild/commit/988dce5bc8eb7d090e9b09eef0a026680ca5a159))

## [0.98.3](https://github.com/chio-labs/sqlbuild/compare/v0.98.2...v0.98.3) (2026-09-12)


### Bug Fixes

* count filtered join references ([#634](https://github.com/chio-labs/sqlbuild/issues/634)) ([7e6ba49](https://github.com/chio-labs/sqlbuild/commit/7e6ba49d13b6a36792fd6451964a8512984db4b8))

## [0.98.2](https://github.com/chio-labs/sqlbuild/compare/v0.98.1...v0.98.2) (2026-09-12)


### Bug Fixes

* withhold unsafe multi-branch case fixes ([#632](https://github.com/chio-labs/sqlbuild/issues/632)) ([87127f2](https://github.com/chio-labs/sqlbuild/commit/87127f2b7d396392074892404e58fcbdbd705c21))

## [0.98.1](https://github.com/chio-labs/sqlbuild/compare/v0.98.0...v0.98.1) (2026-09-12)


### Bug Fixes

* allow typed nulls without nullability metadata ([#630](https://github.com/chio-labs/sqlbuild/issues/630)) ([1354637](https://github.com/chio-labs/sqlbuild/commit/1354637ba60a4d1795887f53c912d24f7c1a1a25))

## [0.98.0](https://github.com/chio-labs/sqlbuild/compare/v0.97.2...v0.98.0) (2026-09-12)


### Features

* support schema-aware partial fixtures ([#628](https://github.com/chio-labs/sqlbuild/issues/628)) ([5384378](https://github.com/chio-labs/sqlbuild/commit/53843785b3e430980ef37ccb8998208948874260))

## [0.97.2](https://github.com/chio-labs/sqlbuild/compare/v0.97.1...v0.97.2) (2026-09-12)


### Performance Improvements

* cache structural lineage graphs ([#626](https://github.com/chio-labs/sqlbuild/issues/626)) ([18bd5cd](https://github.com/chio-labs/sqlbuild/commit/18bd5cd07f2fa0c6d7d798831bbe7aa43357df90))

## [0.97.1](https://github.com/chio-labs/sqlbuild/compare/v0.97.0...v0.97.1) (2026-09-11)


### Bug Fixes

* complete nested scope validation ([#624](https://github.com/chio-labs/sqlbuild/issues/624)) ([f41c00c](https://github.com/chio-labs/sqlbuild/commit/f41c00c9348dd948db30fdf603d60584aa430d02))

## [0.97.0](https://github.com/chio-labs/sqlbuild/compare/v0.96.1...v0.97.0) (2026-09-11)


### Features

* support selector name patterns ([#622](https://github.com/chio-labs/sqlbuild/issues/622)) ([01e19c2](https://github.com/chio-labs/sqlbuild/commit/01e19c250a365f9bc4739f23e4edca9b876327c1))

## [0.96.1](https://github.com/chio-labs/sqlbuild/compare/v0.96.0...v0.96.1) (2026-09-11)


### Bug Fixes

* improve SQL rule accuracy and scoping ([#620](https://github.com/chio-labs/sqlbuild/issues/620)) ([a2f7f85](https://github.com/chio-labs/sqlbuild/commit/a2f7f855126c8e4cf2183146e6d777b0548e508d))

## [0.96.0](https://github.com/chio-labs/sqlbuild/compare/v0.95.0...v0.96.0) (2026-09-11)


### Features

* add deterministic sampling for data diffs ([#618](https://github.com/chio-labs/sqlbuild/issues/618)) ([8ba77ae](https://github.com/chio-labs/sqlbuild/commit/8ba77aeac70e5ffc457d07489f56d7b58b72b819))

## [0.95.0](https://github.com/chio-labs/sqlbuild/compare/v0.94.6...v0.95.0) (2026-09-10)


### Features

* improve native test diagnostics ([#616](https://github.com/chio-labs/sqlbuild/issues/616)) ([44231e7](https://github.com/chio-labs/sqlbuild/commit/44231e79fd82933e2ebdf7da74a2a98e85449dd0))

## [0.94.6](https://github.com/chio-labs/sqlbuild/compare/v0.94.5...v0.94.6) (2026-09-10)


### Bug Fixes

* use one connection for target diffs ([#614](https://github.com/chio-labs/sqlbuild/issues/614)) ([7d009f5](https://github.com/chio-labs/sqlbuild/commit/7d009f5c9d89a6346ce79c46cb3df430aee23b46))

## [0.94.5](https://github.com/chio-labs/sqlbuild/compare/v0.94.4...v0.94.5) (2026-09-10)


### Documentation

* sync simplified benchmark headers ([#611](https://github.com/chio-labs/sqlbuild/issues/611)) ([13e99c9](https://github.com/chio-labs/sqlbuild/commit/13e99c931ae40df3849bcc840422e5e7cd1e0b15))

## [0.94.4](https://github.com/chio-labs/sqlbuild/compare/v0.94.3...v0.94.4) (2026-09-10)


### Documentation

* sync responsive benchmark table ([#609](https://github.com/chio-labs/sqlbuild/issues/609)) ([33d3116](https://github.com/chio-labs/sqlbuild/commit/33d311634bf0731dc229c9d8a5c7ebcb7bb00fab))

## [0.94.3](https://github.com/chio-labs/sqlbuild/compare/v0.94.2...v0.94.3) (2026-09-10)


### Documentation

* sync benchmark guidance ([#607](https://github.com/chio-labs/sqlbuild/issues/607)) ([fd64821](https://github.com/chio-labs/sqlbuild/commit/fd648214bffc36ac990011372a7fd4d4004522c6))

## [0.94.2](https://github.com/chio-labs/sqlbuild/compare/v0.94.1...v0.94.2) (2026-09-10)


### Documentation

* sync rules overview skill ([#605](https://github.com/chio-labs/sqlbuild/issues/605)) ([2d71f57](https://github.com/chio-labs/sqlbuild/commit/2d71f57d3b7206299138a6312e687a449e6d087e))

## [0.94.1](https://github.com/chio-labs/sqlbuild/compare/v0.94.0...v0.94.1) (2026-09-09)


### Bug Fixes

* activate custom rule family prefixes ([#603](https://github.com/chio-labs/sqlbuild/issues/603)) ([665413e](https://github.com/chio-labs/sqlbuild/commit/665413eb0bad3d3fcf7d65d4dd27b33fc10f8b0e))

## [0.94.0](https://github.com/chio-labs/sqlbuild/compare/v0.93.1...v0.94.0) (2026-09-09)


### Features

* integrate configurable compiler rules ([#601](https://github.com/chio-labs/sqlbuild/issues/601)) ([697ff36](https://github.com/chio-labs/sqlbuild/commit/697ff36fe917fcb2c890783df5557d8d997304a1))

## [0.93.1](https://github.com/chio-labs/sqlbuild/compare/v0.93.0...v0.93.1) (2026-09-09)


### Bug Fixes

* render macro declarations safely ([#599](https://github.com/chio-labs/sqlbuild/issues/599)) ([a82cee2](https://github.com/chio-labs/sqlbuild/commit/a82cee20ce140e6410d31d27ebb4c00c6f4293f4))

## [0.93.0](https://github.com/chio-labs/sqlbuild/compare/v0.92.3...v0.93.0) (2026-09-09)


### Features

* expose scoped declarations to macros ([#595](https://github.com/chio-labs/sqlbuild/issues/595)) ([c074918](https://github.com/chio-labs/sqlbuild/commit/c074918b27fafcda7de652a7f23f8556abaef1b7))

## [0.92.3](https://github.com/chio-labs/sqlbuild/compare/v0.92.2...v0.92.3) (2026-09-09)


### Bug Fixes

* use patched polyglot scope validation ([#596](https://github.com/chio-labs/sqlbuild/issues/596)) ([5788959](https://github.com/chio-labs/sqlbuild/commit/5788959de887b4f55159e490e96b8db8554272c2))

## [0.92.2](https://github.com/chio-labs/sqlbuild/compare/v0.92.1...v0.92.2) (2026-09-09)


### Bug Fixes

* attribute failures to exact SQL ([#593](https://github.com/chio-labs/sqlbuild/issues/593)) ([c17a205](https://github.com/chio-labs/sqlbuild/commit/c17a20510c5e79a01461b574fd42ba40e148c057))

## [0.92.1](https://github.com/chio-labs/sqlbuild/compare/v0.92.0...v0.92.1) (2026-09-08)


### Bug Fixes

* quote enforced source identifiers ([#590](https://github.com/chio-labs/sqlbuild/issues/590)) ([9024406](https://github.com/chio-labs/sqlbuild/commit/902440679e6a253f598a36bf84f3cb03928a4b40))

## [0.92.0](https://github.com/chio-labs/sqlbuild/compare/v0.91.4...v0.92.0) (2026-09-08)


### Features

* replace kata with project policy ([#588](https://github.com/chio-labs/sqlbuild/issues/588)) ([18cee6b](https://github.com/chio-labs/sqlbuild/commit/18cee6b888c0eff9db39694d0a0ca728948c002c))

## [0.91.4](https://github.com/chio-labs/sqlbuild/compare/v0.91.3...v0.91.4) (2026-09-08)


### Bug Fixes

* preserve best-effort audit projection ([#582](https://github.com/chio-labs/sqlbuild/issues/582)) ([6c663c1](https://github.com/chio-labs/sqlbuild/commit/6c663c18017dd80705a9ee713d1ea94aa4f3a500))

## [0.91.3](https://github.com/chio-labs/sqlbuild/compare/v0.91.2...v0.91.3) (2026-09-08)


### Bug Fixes

* report contract command progress ([#585](https://github.com/chio-labs/sqlbuild/issues/585)) ([3dae082](https://github.com/chio-labs/sqlbuild/commit/3dae082e37333f793a181cabc22a448ceaab30cd))

## [0.91.2](https://github.com/chio-labs/sqlbuild/compare/v0.91.1...v0.91.2) (2026-09-08)


### Bug Fixes

* render parameterized model types without quotes ([#583](https://github.com/chio-labs/sqlbuild/issues/583)) ([ead4b60](https://github.com/chio-labs/sqlbuild/commit/ead4b60003a194dcd244d7f8dd8bb1e26c22b8da))

## [0.91.1](https://github.com/chio-labs/sqlbuild/compare/v0.91.0...v0.91.1) (2026-09-08)


### Bug Fixes

* preserve timestamp timezone inference ([#580](https://github.com/chio-labs/sqlbuild/issues/580)) ([d596d4c](https://github.com/chio-labs/sqlbuild/commit/d596d4cb076d9f8f78c8c649726e2108f7013af2))

## [0.91.0](https://github.com/chio-labs/sqlbuild/compare/v0.90.5...v0.91.0) (2026-09-07)


### Features

* correlate lifecycle events with orchestrators ([#576](https://github.com/chio-labs/sqlbuild/issues/576)) ([6169cb7](https://github.com/chio-labs/sqlbuild/commit/6169cb73635816524486d912b0ffaa821c3e5f33))

## [0.90.5](https://github.com/chio-labs/sqlbuild/compare/v0.90.4...v0.90.5) (2026-09-07)


### Bug Fixes

* complete audit result projection ([#577](https://github.com/chio-labs/sqlbuild/issues/577)) ([9215dd8](https://github.com/chio-labs/sqlbuild/commit/9215dd8dbf3e523de2d369357ffc9e96259926eb))

## [0.90.4](https://github.com/chio-labs/sqlbuild/compare/v0.90.3...v0.90.4) (2026-09-07)


### Bug Fixes

* honor model SQL analysis opt-outs ([#574](https://github.com/chio-labs/sqlbuild/issues/574)) ([0dbbbda](https://github.com/chio-labs/sqlbuild/commit/0dbbbdaaaddd18243a3902d279e91aa1cdce928b))

## [0.90.3](https://github.com/chio-labs/sqlbuild/compare/v0.90.2...v0.90.3) (2026-09-07)


### Bug Fixes

* preserve contract uncertainty and aliases ([#572](https://github.com/chio-labs/sqlbuild/issues/572)) ([8384308](https://github.com/chio-labs/sqlbuild/commit/8384308c390a76d6a4993612ab197f23ba691220))

## [0.90.2](https://github.com/chio-labs/sqlbuild/compare/v0.90.1...v0.90.2) (2026-09-07)


### Bug Fixes

* quote generated model contract types ([#570](https://github.com/chio-labs/sqlbuild/issues/570)) ([80a6110](https://github.com/chio-labs/sqlbuild/commit/80a6110462fb5acbca0a46c6e052571b99f782c2))

## [0.90.1](https://github.com/chio-labs/sqlbuild/compare/v0.90.0...v0.90.1) (2026-09-07)


### Bug Fixes

* retain shared source contract edits ([#568](https://github.com/chio-labs/sqlbuild/issues/568)) ([9f90f1f](https://github.com/chio-labs/sqlbuild/commit/9f90f1f6b5800cae554df5ef0e87f805b75b8caa))

## [0.90.0](https://github.com/chio-labs/sqlbuild/compare/v0.89.1...v0.90.0) (2026-09-07)


### Features

* bind SQL columns and adopt contracts ([#564](https://github.com/chio-labs/sqlbuild/issues/564)) ([e8e032f](https://github.com/chio-labs/sqlbuild/commit/e8e032f9df275554ed49b6b70fa49272dd61516e))

## [0.89.1](https://github.com/chio-labs/sqlbuild/compare/v0.89.0...v0.89.1) (2026-09-07)


### Bug Fixes

* speed up selected lint and unblock formatting ([#565](https://github.com/chio-labs/sqlbuild/issues/565)) ([ab96142](https://github.com/chio-labs/sqlbuild/commit/ab96142e58373194ad43e2d5e44c468ead8d439b))

## [0.89.0](https://github.com/chio-labs/sqlbuild/compare/v0.88.0...v0.89.0) (2026-09-07)


### Features

* support focused project compilation ([#562](https://github.com/chio-labs/sqlbuild/issues/562)) ([aed7170](https://github.com/chio-labs/sqlbuild/commit/aed717005d01ef15b8760021e7cc82db412cb567))

## [0.88.0](https://github.com/chio-labs/sqlbuild/compare/v0.87.3...v0.88.0) (2026-09-07)


### Features

* add sqb --version flag ([#560](https://github.com/chio-labs/sqlbuild/issues/560)) ([dd3f0d7](https://github.com/chio-labs/sqlbuild/commit/dd3f0d7b1845ebf130ca785e143098bba0cf6314))

## [0.87.3](https://github.com/chio-labs/sqlbuild/compare/v0.87.2...v0.87.3) (2026-09-06)


### Bug Fixes

* lint generic audits and accelerate large projects ([#558](https://github.com/chio-labs/sqlbuild/issues/558)) ([b8dca63](https://github.com/chio-labs/sqlbuild/commit/b8dca635e3788c3fe73b7be493141d69a99e7327))

## [0.87.2](https://github.com/chio-labs/sqlbuild/compare/v0.87.1...v0.87.2) (2026-09-06)


### Bug Fixes

* batch command output capture ([#556](https://github.com/chio-labs/sqlbuild/issues/556)) ([4bed50c](https://github.com/chio-labs/sqlbuild/commit/4bed50ca4946b66b49581604c49213cb123d8f06))

## [0.87.1](https://github.com/chio-labs/sqlbuild/compare/v0.87.0...v0.87.1) (2026-09-06)


### Bug Fixes

* report measurement warnings correctly in Dagster ([#554](https://github.com/chio-labs/sqlbuild/issues/554)) ([77530f4](https://github.com/chio-labs/sqlbuild/commit/77530f42d5404e3e0ac735c7f45fb8bfb143cb82))

## [0.87.0](https://github.com/chio-labs/sqlbuild/compare/v0.86.0...v0.87.0) (2026-09-06)


### Features

* complete native lint and safe fixes ([#552](https://github.com/chio-labs/sqlbuild/issues/552)) ([452cc28](https://github.com/chio-labs/sqlbuild/commit/452cc282367cf1e78c759c0f6bdf75765d7c4bef))

## [0.86.0](https://github.com/chio-labs/sqlbuild/compare/v0.85.0...v0.86.0) (2026-09-06)


### Features

* add actionable native lint diagnostics ([#550](https://github.com/chio-labs/sqlbuild/issues/550)) ([2c215d9](https://github.com/chio-labs/sqlbuild/commit/2c215d9ce541e113e39ed12d5293adfaa866ddb1))

## [0.85.0](https://github.com/chio-labs/sqlbuild/compare/v0.84.0...v0.85.0) (2026-09-06)


### Features

* make Dagster integration translator-native ([#548](https://github.com/chio-labs/sqlbuild/issues/548)) ([9fa32ff](https://github.com/chio-labs/sqlbuild/commit/9fa32ffee7f6aaa123a0ea7e37982a561d828af8))

## [0.84.0](https://github.com/chio-labs/sqlbuild/compare/v0.83.4...v0.84.0) (2026-09-06)


### Features

* replace Sqruff with native SQL lint and format ([#546](https://github.com/chio-labs/sqlbuild/issues/546)) ([e553caa](https://github.com/chio-labs/sqlbuild/commit/e553caaa5adc4f98b4f94b7abd5bb444ad499c49))

## [0.83.4](https://github.com/chio-labs/sqlbuild/compare/v0.83.3...v0.83.4) (2026-09-06)


### Performance Improvements

* add Dagster-shaped compile guard ([#544](https://github.com/chio-labs/sqlbuild/issues/544)) ([a1fe6e7](https://github.com/chio-labs/sqlbuild/commit/a1fe6e7b3f9259f01cebf1af72dbe1bd09b955a4))

## [0.83.3](https://github.com/chio-labs/sqlbuild/compare/v0.83.2...v0.83.3) (2026-09-06)


### Bug Fixes

* reject Python outside supported project roots ([#542](https://github.com/chio-labs/sqlbuild/issues/542)) ([31030e7](https://github.com/chio-labs/sqlbuild/commit/31030e745715cf0cea5468c4b73350057471e1b5))

## [0.83.2](https://github.com/chio-labs/sqlbuild/compare/v0.83.1...v0.83.2) (2026-09-06)


### Bug Fixes

* include required audit definition metadata in results and events ([#540](https://github.com/chio-labs/sqlbuild/issues/540)) ([b2c0819](https://github.com/chio-labs/sqlbuild/commit/b2c0819ac6e4df0bc6b30bf43eda1d560ac35ed5))

## [0.83.1](https://github.com/chio-labs/sqlbuild/compare/v0.83.0...v0.83.1) (2026-09-06)


### Performance Improvements

* cache unchanged compile artifacts ([#538](https://github.com/chio-labs/sqlbuild/issues/538)) ([0affb0c](https://github.com/chio-labs/sqlbuild/commit/0affb0c2ef273af6200995a760e3852d63c9d04a))

## [0.83.0](https://github.com/chio-labs/sqlbuild/compare/v0.82.6...v0.83.0) (2026-09-06)


### Features

* add measurement audits, audit factories, result history, and audit lifecycle events ([#532](https://github.com/chio-labs/sqlbuild/issues/532)) ([3028406](https://github.com/chio-labs/sqlbuild/commit/3028406c634694dd0aefcc7df01bf873c681557e))

## [0.82.6](https://github.com/chio-labs/sqlbuild/compare/v0.82.5...v0.82.6) (2026-09-05)


### Bug Fixes

* preserve adapter SQL in native tests ([#535](https://github.com/chio-labs/sqlbuild/issues/535)) ([c7ab2d6](https://github.com/chio-labs/sqlbuild/commit/c7ab2d6d9a82ad5d810542e91d9046e2e6b509c0))

## [0.82.5](https://github.com/chio-labs/sqlbuild/compare/v0.82.4...v0.82.5) (2026-09-05)


### Bug Fixes

* bound and accelerate SQL test compilation ([#533](https://github.com/chio-labs/sqlbuild/issues/533)) ([0e0eff9](https://github.com/chio-labs/sqlbuild/commit/0e0eff913d9d9290fe4f3e3bd806ff3be93c4fa7))

## [0.82.4](https://github.com/chio-labs/sqlbuild/compare/v0.82.3...v0.82.4) (2026-09-05)


### Bug Fixes

* run chained SQL tests concurrently ([#530](https://github.com/chio-labs/sqlbuild/issues/530)) ([08cb4c8](https://github.com/chio-labs/sqlbuild/commit/08cb4c8196d61919d1baf50b84281247bbb19230))

## [0.82.3](https://github.com/chio-labs/sqlbuild/compare/v0.82.2...v0.82.3) (2026-09-05)


### Bug Fixes

* preserve incremental target column types ([#528](https://github.com/chio-labs/sqlbuild/issues/528)) ([ec8fb6f](https://github.com/chio-labs/sqlbuild/commit/ec8fb6f5e892710f3a7321746cc13c05a2a98f11))

## [0.82.2](https://github.com/chio-labs/sqlbuild/compare/v0.82.1...v0.82.2) (2026-09-04)


### Bug Fixes

* harden typed seams for durations, node kinds, destinations, config, severity, and state literals ([#526](https://github.com/chio-labs/sqlbuild/issues/526)) ([ca965b7](https://github.com/chio-labs/sqlbuild/commit/ca965b78f524ac9cb62321e7a916fbf49d2b271c))

## [0.82.1](https://github.com/chio-labs/sqlbuild/compare/v0.82.0...v0.82.1) (2026-09-04)


### Bug Fixes

* record source freshness observations in UTC ([#522](https://github.com/chio-labs/sqlbuild/issues/522)) ([a828f3a](https://github.com/chio-labs/sqlbuild/commit/a828f3a76d2e377c74487c42742d64d4ab92fbe2))

## [0.82.0](https://github.com/chio-labs/sqlbuild/compare/v0.81.2...v0.82.0) (2026-09-04)


### Features

* separate typed command output sinks ([#523](https://github.com/chio-labs/sqlbuild/issues/523)) ([ac554f1](https://github.com/chio-labs/sqlbuild/commit/ac554f152929333b3afd3a2f1eb6a11964739b2a))

## [0.81.2](https://github.com/chio-labs/sqlbuild/compare/v0.81.1...v0.81.2) (2026-09-04)


### Bug Fixes

* show SQL test difference samples ([#520](https://github.com/chio-labs/sqlbuild/issues/520)) ([627ab95](https://github.com/chio-labs/sqlbuild/commit/627ab95fe13b63ed6ffa308120f006a446042eeb))

## [0.81.1](https://github.com/chio-labs/sqlbuild/compare/v0.81.0...v0.81.1) (2026-09-04)


### Bug Fixes

* release loader decimal acceptance and UTC cursor normalization ([#517](https://github.com/chio-labs/sqlbuild/issues/517)) ([6d621d1](https://github.com/chio-labs/sqlbuild/commit/6d621d141f084ff1eb7dcbce06d8fc374cabd9f7))

## [0.81.0](https://github.com/chio-labs/sqlbuild/compare/v0.80.2...v0.81.0) (2026-09-04)


### Features

* add live statement observability, output capture, and typed cursor algebra ([#514](https://github.com/chio-labs/sqlbuild/issues/514)) ([cce6a79](https://github.com/chio-labs/sqlbuild/commit/cce6a795a81cdd5468cdbeb230a955f9c9a9bbc5))

## [0.80.2](https://github.com/chio-labs/sqlbuild/compare/v0.80.1...v0.80.2) (2026-09-04)


### Bug Fixes

* canonicalize cursor boundaries and isolate integration failures ([#512](https://github.com/chio-labs/sqlbuild/issues/512)) ([1d48a23](https://github.com/chio-labs/sqlbuild/commit/1d48a230d5cec7d42138f5661884318cbc7bba75))

## [0.80.1](https://github.com/chio-labs/sqlbuild/compare/v0.80.0...v0.80.1) (2026-09-04)


### Bug Fixes

* preserve capped microbatch cursor boundaries ([#510](https://github.com/chio-labs/sqlbuild/issues/510)) ([9197813](https://github.com/chio-labs/sqlbuild/commit/91978132b52b6f4a0d1bebad733742ae36999474))

## [0.80.0](https://github.com/chio-labs/sqlbuild/compare/v0.79.0...v0.80.0) (2026-09-03)


### Features

* add watermark microbatch cap policies ([#508](https://github.com/chio-labs/sqlbuild/issues/508)) ([1b44742](https://github.com/chio-labs/sqlbuild/commit/1b447423729a289f04bbb543261dd02118856e18))

## [0.79.0](https://github.com/chio-labs/sqlbuild/compare/v0.78.0...v0.79.0) (2026-09-03)


### Features

* enforce canonical native project contracts ([#506](https://github.com/chio-labs/sqlbuild/issues/506)) ([500aae8](https://github.com/chio-labs/sqlbuild/commit/500aae8778293b5aee5cfedec40aa45ce36c921b))

## [0.78.0](https://github.com/chio-labs/sqlbuild/compare/v0.77.2...v0.78.0) (2026-09-03)


### Features

* redesign microbatch cursor semantics ([#504](https://github.com/chio-labs/sqlbuild/issues/504)) ([c438447](https://github.com/chio-labs/sqlbuild/commit/c438447541f8131b8da2ce32ee4877775f7a335d))

## [0.77.2](https://github.com/chio-labs/sqlbuild/compare/v0.77.1...v0.77.2) (2026-09-03)


### Bug Fixes

* preserve attached audit instance names ([#502](https://github.com/chio-labs/sqlbuild/issues/502)) ([6c19882](https://github.com/chio-labs/sqlbuild/commit/6c198829b61def6f1aee8a5ff5f3290f097b7fbe))

## [0.77.1](https://github.com/chio-labs/sqlbuild/compare/v0.77.0...v0.77.1) (2026-09-03)


### Bug Fixes

* restore Dagster integration contracts ([#500](https://github.com/chio-labs/sqlbuild/issues/500)) ([c02cfd0](https://github.com/chio-labs/sqlbuild/commit/c02cfd00feb3dfaccb54f4040666f6dc720c23df))

## [0.77.0](https://github.com/chio-labs/sqlbuild/compare/v0.76.9...v0.77.0) (2026-09-03)


### Features

* add causal microbatch replay propagation ([#498](https://github.com/chio-labs/sqlbuild/issues/498)) ([9c49381](https://github.com/chio-labs/sqlbuild/commit/9c493817d97815873c0efde9effed6d83d2f517b))

## [0.76.9](https://github.com/chio-labs/sqlbuild/compare/v0.76.8...v0.76.9) (2026-09-03)


### Bug Fixes

* collect standalone audit execution errors ([#496](https://github.com/chio-labs/sqlbuild/issues/496)) ([b4d196c](https://github.com/chio-labs/sqlbuild/commit/b4d196c1726debb6d4ac9eea5e959a0a792ac326))

## [0.76.8](https://github.com/chio-labs/sqlbuild/compare/v0.76.7...v0.76.8) (2026-09-03)


### Bug Fixes

* install skills at repository root ([#494](https://github.com/chio-labs/sqlbuild/issues/494)) ([cdd1ea5](https://github.com/chio-labs/sqlbuild/commit/cdd1ea56991d6ca86184128fad7a952ae46d2fc2))

## [0.76.7](https://github.com/chio-labs/sqlbuild/compare/v0.76.6...v0.76.7) (2026-09-03)


### Bug Fixes

* add bounded standalone audit concurrency ([#492](https://github.com/chio-labs/sqlbuild/issues/492)) ([16af9a4](https://github.com/chio-labs/sqlbuild/commit/16af9a4aecde35c27acd1e582b126ffcf08b2663))

## [0.76.6](https://github.com/chio-labs/sqlbuild/compare/v0.76.5...v0.76.6) (2026-09-03)


### Bug Fixes

* avoid build planning for focused commands ([#490](https://github.com/chio-labs/sqlbuild/issues/490)) ([6e2a82d](https://github.com/chio-labs/sqlbuild/commit/6e2a82d1b061cc6d520ea62078b10b3dee043a46))

## [0.76.5](https://github.com/chio-labs/sqlbuild/compare/v0.76.4...v0.76.5) (2026-09-03)


### Bug Fixes

* make execution history persistence explicit ([#488](https://github.com/chio-labs/sqlbuild/issues/488)) ([8f2928d](https://github.com/chio-labs/sqlbuild/commit/8f2928db95486512a0898eae255c7ae3af4b80c6))

## [0.76.4](https://github.com/chio-labs/sqlbuild/compare/v0.76.3...v0.76.4) (2026-09-03)


### Bug Fixes

* preserve grants during table type conversion ([#486](https://github.com/chio-labs/sqlbuild/issues/486)) ([55c866c](https://github.com/chio-labs/sqlbuild/commit/55c866cbe03a72431b1195c531af232a82a86527))

## [0.76.3](https://github.com/chio-labs/sqlbuild/compare/v0.76.2...v0.76.3) (2026-09-03)


### Bug Fixes

* remove execution observability compatibility shims ([#483](https://github.com/chio-labs/sqlbuild/issues/483)) ([eefccf6](https://github.com/chio-labs/sqlbuild/commit/eefccf6cf2dd3b66b7194eb43e5897693918bb50))

## [0.76.2](https://github.com/chio-labs/sqlbuild/compare/v0.76.1...v0.76.2) (2026-09-03)


### Bug Fixes

* align clone progress output ([#474](https://github.com/chio-labs/sqlbuild/issues/474)) ([0031034](https://github.com/chio-labs/sqlbuild/commit/0031034763344b88cb883e30097a00941cb02ba2))

## [0.76.1](https://github.com/chio-labs/sqlbuild/compare/v0.76.0...v0.76.1) (2026-09-03)


### Bug Fixes

* separate audit dependencies from model scheduling ([#481](https://github.com/chio-labs/sqlbuild/issues/481)) ([8fe5e30](https://github.com/chio-labs/sqlbuild/commit/8fe5e301ceea48d5d5a851f7e043c3ef7129e345))

## [0.76.0](https://github.com/chio-labs/sqlbuild/compare/v0.75.1...v0.76.0) (2026-09-02)


### Features

* add execution observability foundation ([#479](https://github.com/chio-labs/sqlbuild/issues/479)) ([80ee9a9](https://github.com/chio-labs/sqlbuild/commit/80ee9a9f601e771c720b881057743776a04ffae8))

## [0.75.1](https://github.com/chio-labs/sqlbuild/compare/v0.75.0...v0.75.1) (2026-09-02)


### Bug Fixes

* scope Dagster check-only execution ([#477](https://github.com/chio-labs/sqlbuild/issues/477)) ([ba43f41](https://github.com/chio-labs/sqlbuild/commit/ba43f41a597e0208d3a52eeb9a941eb13c7aa566))

## [0.75.0](https://github.com/chio-labs/sqlbuild/compare/v0.74.4...v0.75.0) (2026-09-02)


### Features

* add explicit column contract mode ([#475](https://github.com/chio-labs/sqlbuild/issues/475)) ([62c7724](https://github.com/chio-labs/sqlbuild/commit/62c7724870de760124734d0e67e4a063baa0900b))

## [0.74.4](https://github.com/chio-labs/sqlbuild/compare/v0.74.3...v0.74.4) (2026-09-02)


### Bug Fixes

* defer inferred watermark columns to warehouse ([#472](https://github.com/chio-labs/sqlbuild/issues/472)) ([e77283b](https://github.com/chio-labs/sqlbuild/commit/e77283b69d2d08683c505ddf5ab3c75033cb3ef0))

## [0.74.3](https://github.com/chio-labs/sqlbuild/compare/v0.74.2...v0.74.3) (2026-09-02)


### Bug Fixes

* align incremental writes by column name ([#470](https://github.com/chio-labs/sqlbuild/issues/470)) ([361ab15](https://github.com/chio-labs/sqlbuild/commit/361ab15deb857e711746e324f1b11fa657a30a3a))

## [0.74.2](https://github.com/chio-labs/sqlbuild/compare/v0.74.1...v0.74.2) (2026-09-02)


### Bug Fixes

* allow zero-day future cursor caps ([#468](https://github.com/chio-labs/sqlbuild/issues/468)) ([c6cad5e](https://github.com/chio-labs/sqlbuild/commit/c6cad5e65ca3c483334fc1c3b6d57d82afc8da34))

## [0.74.1](https://github.com/chio-labs/sqlbuild/compare/v0.74.0...v0.74.1) (2026-09-02)


### Documentation

* clarify automated delivery workflow ([#466](https://github.com/chio-labs/sqlbuild/issues/466)) ([b11048d](https://github.com/chio-labs/sqlbuild/commit/b11048dea014ba8b73db77f7fca568ff0a3777a9))

## [0.74.0](https://github.com/chio-labs/sqlbuild/compare/v0.73.0...v0.74.0) (2026-09-02)


### Features

* bound automatic cursor starts ([#464](https://github.com/chio-labs/sqlbuild/issues/464)) ([5b1d19a](https://github.com/chio-labs/sqlbuild/commit/5b1d19a196cc13fc391e2a689b3669e6406e7ec8))

## [0.73.0](https://github.com/chio-labs/sqlbuild/compare/v0.72.2...v0.73.0) (2026-09-01)


### Features

* add configurable microbatch limits ([#462](https://github.com/chio-labs/sqlbuild/issues/462)) ([5a82b36](https://github.com/chio-labs/sqlbuild/commit/5a82b363439871fb989a72a6f0d385fce4493a91))

## [0.72.2](https://github.com/chio-labs/sqlbuild/compare/v0.72.1...v0.72.2) (2026-09-01)


### Bug Fixes

* bound implausible future cursor watermarks ([#460](https://github.com/chio-labs/sqlbuild/issues/460)) ([cb22a2f](https://github.com/chio-labs/sqlbuild/commit/cb22a2fa9499d0ae5ae4df3bf62809e769bc9983))

## [0.72.1](https://github.com/chio-labs/sqlbuild/compare/v0.72.0...v0.72.1) (2026-09-01)


### Bug Fixes

* scope cursor validation to selected models ([#458](https://github.com/chio-labs/sqlbuild/issues/458)) ([bb0c99a](https://github.com/chio-labs/sqlbuild/commit/bb0c99aa6bac6fc263b30c6a745757461e97169f))

## [0.72.0](https://github.com/chio-labs/sqlbuild/compare/v0.71.6...v0.72.0) (2026-09-01)


### Features

* separate cursor filter and watermark inputs ([#456](https://github.com/chio-labs/sqlbuild/issues/456)) ([2296306](https://github.com/chio-labs/sqlbuild/commit/22963069e173f909255ef7d345deed2c479e510d))

## [0.71.6](https://github.com/chio-labs/sqlbuild/compare/v0.71.5...v0.71.6) (2026-09-01)


### Bug Fixes

* exclude views from table type conversion planning ([#454](https://github.com/chio-labs/sqlbuild/issues/454)) ([416550a](https://github.com/chio-labs/sqlbuild/commit/416550ad5c59990bc84ee1fc3975e962e3a582dd))

## [0.71.5](https://github.com/chio-labs/sqlbuild/compare/v0.71.4...v0.71.5) (2026-09-01)


### Bug Fixes

* execute cursor bound inspections per physical relation ([#452](https://github.com/chio-labs/sqlbuild/issues/452)) ([8074020](https://github.com/chio-labs/sqlbuild/commit/8074020378ee56854f3d45fa4685a1f1fffe689c))

## [0.71.4](https://github.com/chio-labs/sqlbuild/compare/v0.71.3...v0.71.4) (2026-09-01)


### Documentation

* refresh declaration scope guidance ([#450](https://github.com/chio-labs/sqlbuild/issues/450)) ([8499c6e](https://github.com/chio-labs/sqlbuild/commit/8499c6e52f07e30ddc6cdee023935f24fe4c548d))

## [0.71.3](https://github.com/chio-labs/sqlbuild/compare/v0.71.2...v0.71.3) (2026-08-31)


### Bug Fixes

* suppress false removals for unresolved stars ([#448](https://github.com/chio-labs/sqlbuild/issues/448)) ([a4955e6](https://github.com/chio-labs/sqlbuild/commit/a4955e6d8c27b8751cde674c60dbf90a3bb52b8b))

## [0.71.2](https://github.com/chio-labs/sqlbuild/compare/v0.71.1...v0.71.2) (2026-08-31)


### Bug Fixes

* derive serial microbatch progress from the target cursor ([#444](https://github.com/chio-labs/sqlbuild/issues/444)) ([79eab16](https://github.com/chio-labs/sqlbuild/commit/79eab1660a360963fc997acb53a04922ead93c54))

## [0.71.1](https://github.com/chio-labs/sqlbuild/compare/v0.71.0...v0.71.1) (2026-08-31)


### Bug Fixes

* add scopes enforce_placement flag for staged policy adoption ([#442](https://github.com/chio-labs/sqlbuild/issues/442)) ([262922e](https://github.com/chio-labs/sqlbuild/commit/262922e8f1735532b8242897ba725c44522b3b6a))

## [0.71.0](https://github.com/chio-labs/sqlbuild/compare/v0.70.0...v0.71.0) (2026-08-31)


### Features

* rebuild microbatch and snapshot full refresh aside and swap ([#440](https://github.com/chio-labs/sqlbuild/issues/440)) ([69eb2fd](https://github.com/chio-labs/sqlbuild/commit/69eb2fda787c6e3584ea51f8ec5d0553dc215848))

## [0.70.0](https://github.com/chio-labs/sqlbuild/compare/v0.69.0...v0.70.0) (2026-08-31)


### Features

* manage Snowflake table type declaratively with copy-swap conversion ([#438](https://github.com/chio-labs/sqlbuild/issues/438)) ([8b1db2f](https://github.com/chio-labs/sqlbuild/commit/8b1db2f1739c1469ec20cc38c499706fdd467153))

## [0.69.0](https://github.com/chio-labs/sqlbuild/compare/v0.68.0...v0.69.0) (2026-08-31)


### Features

* remove archive machinery and make direct janitor report-only ([#436](https://github.com/chio-labs/sqlbuild/issues/436)) ([bee3aa9](https://github.com/chio-labs/sqlbuild/commit/bee3aa98e32d32a0f3efb0a6c27b512ab80d5814))

## [0.68.0](https://github.com/chio-labs/sqlbuild/compare/v0.67.2...v0.68.0) (2026-08-31)


### Features

* add declarative retention and reversible direct cleanup ([#434](https://github.com/chio-labs/sqlbuild/issues/434)) ([2f66df8](https://github.com/chio-labs/sqlbuild/commit/2f66df8c6907f7b8244e6ca0f77f96ab55cc9322))

## [0.67.2](https://github.com/chio-labs/sqlbuild/compare/v0.67.1...v0.67.2) (2026-08-31)


### Bug Fixes

* resolve placement anchors by fixed point and require explicit declaration scope ([#432](https://github.com/chio-labs/sqlbuild/issues/432)) ([58f4c7e](https://github.com/chio-labs/sqlbuild/commit/58f4c7ef1569ff662372b3c58adc74436090254c))

## [0.67.1](https://github.com/chio-labs/sqlbuild/compare/v0.67.0...v0.67.1) (2026-08-31)


### Bug Fixes

* keep Dagster failure metadata concise ([#429](https://github.com/chio-labs/sqlbuild/issues/429)) ([38e7362](https://github.com/chio-labs/sqlbuild/commit/38e736223f1ca3a42c0c8985a5afda4880b729b1))

## [0.67.0](https://github.com/chio-labs/sqlbuild/compare/v0.66.6...v0.67.0) (2026-08-31)


### Features

* enforce declaration and Kata owner layouts ([#428](https://github.com/chio-labs/sqlbuild/issues/428)) ([5bd8286](https://github.com/chio-labs/sqlbuild/commit/5bd8286bbf330d64d51314afe28a82d1d7eaad1c))

## [0.66.6](https://github.com/chio-labs/sqlbuild/compare/v0.66.5...v0.66.6) (2026-08-31)


### Bug Fixes

* match dbt model full-refresh semantics ([#422](https://github.com/chio-labs/sqlbuild/issues/422)) ([f5340ce](https://github.com/chio-labs/sqlbuild/commit/f5340ce372260488f056a05256492f863a3b01f2))

## [0.66.5](https://github.com/chio-labs/sqlbuild/compare/v0.66.4...v0.66.5) (2026-08-30)


### Bug Fixes

* add bounded build runtime diagnostics ([#425](https://github.com/chio-labs/sqlbuild/issues/425)) ([e7b0714](https://github.com/chio-labs/sqlbuild/commit/e7b0714fd6aeda4512d34e15e171d6fc1761f8e7))

## [0.66.4](https://github.com/chio-labs/sqlbuild/compare/v0.66.3...v0.66.4) (2026-08-30)


### Bug Fixes

* report resolved build run context ([#423](https://github.com/chio-labs/sqlbuild/issues/423)) ([db0c47b](https://github.com/chio-labs/sqlbuild/commit/db0c47b735de12ffa9fb343c5c9354aab24b9628))

## [0.66.3](https://github.com/chio-labs/sqlbuild/compare/v0.66.2...v0.66.3) (2026-08-30)


### Bug Fixes

* enrich Dagster non-SQL asset descriptions ([#420](https://github.com/chio-labs/sqlbuild/issues/420)) ([b84bc36](https://github.com/chio-labs/sqlbuild/commit/b84bc360a1fdf408dcc786eb8ba24d6bf8dd0608))

## [0.66.2](https://github.com/chio-labs/sqlbuild/compare/v0.66.1...v0.66.2) (2026-08-30)


### Bug Fixes

* qualify Snowflake cost query history database ([#418](https://github.com/chio-labs/sqlbuild/issues/418)) ([3c748fb](https://github.com/chio-labs/sqlbuild/commit/3c748fba975c7269fbb8fdee06bbcb6cf075d3fd))

## [0.66.1](https://github.com/chio-labs/sqlbuild/compare/v0.66.0...v0.66.1) (2026-08-30)


### Bug Fixes

* load empty seed fields as null ([#416](https://github.com/chio-labs/sqlbuild/issues/416)) ([0506f4e](https://github.com/chio-labs/sqlbuild/commit/0506f4e7dd0f813b14e63586510f685ed2a3346b))

## [0.66.0](https://github.com/chio-labs/sqlbuild/compare/v0.65.6...v0.66.0) (2026-08-30)


### Features

* make debug include verbose output ([#414](https://github.com/chio-labs/sqlbuild/issues/414)) ([9a62459](https://github.com/chio-labs/sqlbuild/commit/9a6245941dba3a4f5c53e0f42c7262322ee1c57b))

## [0.65.6](https://github.com/chio-labs/sqlbuild/compare/v0.65.5...v0.65.6) (2026-08-30)


### Bug Fixes

* preserve Dagster compute stdout ([#412](https://github.com/chio-labs/sqlbuild/issues/412)) ([858b486](https://github.com/chio-labs/sqlbuild/commit/858b4869212bbc71172ecda6171847881d9d9c26))

## [0.65.5](https://github.com/chio-labs/sqlbuild/compare/v0.65.4...v0.65.5) (2026-08-30)


### Bug Fixes

* stream SQLBuild execution events live ([#410](https://github.com/chio-labs/sqlbuild/issues/410)) ([b22a47d](https://github.com/chio-labs/sqlbuild/commit/b22a47d4dd446aa6b1c970604d1fe93fc963983e))

## [0.65.4](https://github.com/chio-labs/sqlbuild/compare/v0.65.3...v0.65.4) (2026-08-30)


### Bug Fixes

* make clone dependency outcomes explicit ([#408](https://github.com/chio-labs/sqlbuild/issues/408)) ([34c54b2](https://github.com/chio-labs/sqlbuild/commit/34c54b26e5185abd1be9f5df336242faaf0459fe))

## [0.65.3](https://github.com/chio-labs/sqlbuild/compare/v0.65.2...v0.65.3) (2026-08-30)


### Bug Fixes

* stream SQLBuild output to Dagster logs ([#406](https://github.com/chio-labs/sqlbuild/issues/406)) ([df6148a](https://github.com/chio-labs/sqlbuild/commit/df6148a35c362ca2c0f0c101939d0cc1397c1139))

## [0.65.2](https://github.com/chio-labs/sqlbuild/compare/v0.65.1...v0.65.2) (2026-08-30)


### Bug Fixes

* avoid implicit Snowflake role switching ([#404](https://github.com/chio-labs/sqlbuild/issues/404)) ([77d6297](https://github.com/chio-labs/sqlbuild/commit/77d629786ed71d63f0960a338cd22f6ed6030084))

## [0.65.1](https://github.com/chio-labs/sqlbuild/compare/v0.65.0...v0.65.1) (2026-08-30)


### Bug Fixes

* support restricted Snowflake role sessions ([#402](https://github.com/chio-labs/sqlbuild/issues/402)) ([ce7666b](https://github.com/chio-labs/sqlbuild/commit/ce7666bddbba5b3d48954dadfc6988f4560c1d3c))

## [0.65.0](https://github.com/chio-labs/sqlbuild/compare/v0.64.0...v0.65.0) (2026-08-30)


### Features

* separate connections from target namespaces ([#399](https://github.com/chio-labs/sqlbuild/issues/399)) ([99b58f6](https://github.com/chio-labs/sqlbuild/commit/99b58f6eb91648193c6e1ccf10cc2b9429eb032e))

## [0.64.0](https://github.com/chio-labs/sqlbuild/compare/v0.63.8...v0.64.0) (2026-08-29)


### Features

* support glob path defaults ([#395](https://github.com/chio-labs/sqlbuild/issues/395)) ([717d57f](https://github.com/chio-labs/sqlbuild/commit/717d57fad586b86608b091b3752d1a9d59e991b3))

## [0.63.8](https://github.com/chio-labs/sqlbuild/compare/v0.63.7...v0.63.8) (2026-08-29)


### Bug Fixes

* require named targets to declare schema strategy ([#393](https://github.com/chio-labs/sqlbuild/issues/393)) ([e823e1c](https://github.com/chio-labs/sqlbuild/commit/e823e1c9c38edc000e2318412b5f8384e673116d))

## [0.63.7](https://github.com/chio-labs/sqlbuild/compare/v0.63.6...v0.63.7) (2026-08-29)


### Bug Fixes

* require explicit warehouse write schemas ([#391](https://github.com/chio-labs/sqlbuild/issues/391)) ([253303e](https://github.com/chio-labs/sqlbuild/commit/253303ee2ba48cdbb1fbcba63dc6b2612ec7a39e))

## [0.63.6](https://github.com/chio-labs/sqlbuild/compare/v0.63.5...v0.63.6) (2026-08-29)


### Bug Fixes

* add seed and function namespace defaults ([#389](https://github.com/chio-labs/sqlbuild/issues/389)) ([318b5d4](https://github.com/chio-labs/sqlbuild/commit/318b5d425e2a8fc76f23932f5ed81fa9c08b6b28))

## [0.63.5](https://github.com/chio-labs/sqlbuild/compare/v0.63.4...v0.63.5) (2026-08-29)


### Bug Fixes

* preserve logical schemas across clone targets ([#387](https://github.com/chio-labs/sqlbuild/issues/387)) ([8bdb79d](https://github.com/chio-labs/sqlbuild/commit/8bdb79dec4af835b08838e39f41cd271955438aa))

## [0.63.4](https://github.com/chio-labs/sqlbuild/compare/v0.63.3...v0.63.4) (2026-08-29)


### Bug Fixes

* **snowflake:** disable secondary roles by default ([#385](https://github.com/chio-labs/sqlbuild/issues/385)) ([1f7457b](https://github.com/chio-labs/sqlbuild/commit/1f7457b3cfd70bd928dd1a037adcc974d6bec68f))

## [0.63.3](https://github.com/chio-labs/sqlbuild/compare/v0.63.2...v0.63.3) (2026-08-29)


### Bug Fixes

* enrich Dagster metadata and Snowflake inference ([#383](https://github.com/chio-labs/sqlbuild/issues/383)) ([9bcc4d3](https://github.com/chio-labs/sqlbuild/commit/9bcc4d36290e0fdcca1e6cef5dac74f718252a82))

## [0.63.2](https://github.com/chio-labs/sqlbuild/compare/v0.63.1...v0.63.2) (2026-08-28)


### Bug Fixes

* **snowflake:** complete cost and accelerate planning ([#380](https://github.com/chio-labs/sqlbuild/issues/380)) ([003d08f](https://github.com/chio-labs/sqlbuild/commit/003d08f5f0e50ba855b9c36dff0e404d6c166f8a))

## [0.63.1](https://github.com/chio-labs/sqlbuild/compare/v0.63.0...v0.63.1) (2026-08-28)


### Bug Fixes

* **snowflake:** use local timestamps for query history ([#378](https://github.com/chio-labs/sqlbuild/issues/378)) ([0a1b39c](https://github.com/chio-labs/sqlbuild/commit/0a1b39c8323ee39516232ef68e2e2250160f2771))

## [0.63.0](https://github.com/chio-labs/sqlbuild/compare/v0.62.2...v0.63.0) (2026-08-27)


### Features

* **macros:** restore Python composition ([#376](https://github.com/chio-labs/sqlbuild/issues/376)) ([b43eac7](https://github.com/chio-labs/sqlbuild/commit/b43eac784e7341473996a77324cd5607f68f3313))

## [0.62.2](https://github.com/chio-labs/sqlbuild/compare/v0.62.1...v0.62.2) (2026-08-27)


### Documentation

* **skills:** simplify declaration entry points ([#374](https://github.com/chio-labs/sqlbuild/issues/374)) ([c9a92dc](https://github.com/chio-labs/sqlbuild/commit/c9a92dce72b52196018b29c12a4160c7a1944ef5))

## [0.62.1](https://github.com/chio-labs/sqlbuild/compare/v0.62.0...v0.62.1) (2026-08-27)


### Documentation

* **skills:** restructure declaration scopes guidance ([#372](https://github.com/chio-labs/sqlbuild/issues/372)) ([73a26c4](https://github.com/chio-labs/sqlbuild/commit/73a26c4cd3cc873d0f84f8e087979edb41464fd3))

## [0.62.0](https://github.com/chio-labs/sqlbuild/compare/v0.61.0...v0.62.0) (2026-08-26)


### Features

* **tests:** add typed SQL test parameterization ([#370](https://github.com/chio-labs/sqlbuild/issues/370)) ([2b0f2d1](https://github.com/chio-labs/sqlbuild/commit/2b0f2d14ab1b21e74b30555175af50111f306e27))

## [0.61.0](https://github.com/chio-labs/sqlbuild/compare/v0.60.0...v0.61.0) (2026-08-26)


### Features

* **kata:** add SQL test policy rules ([#368](https://github.com/chio-labs/sqlbuild/issues/368)) ([9379152](https://github.com/chio-labs/sqlbuild/commit/9379152bd38ae4dfe0f7eabaebb2387e51906922))

## [0.60.0](https://github.com/chio-labs/sqlbuild/compare/v0.59.0...v0.60.0) (2026-08-26)


### Features

* **scopes:** add compiler-enforced declaration scopes ([#366](https://github.com/chio-labs/sqlbuild/issues/366)) ([3bcd274](https://github.com/chio-labs/sqlbuild/commit/3bcd274ccdda48d366ce4ea18eb1d3abbf670315))

## [0.59.0](https://github.com/chio-labs/sqlbuild/compare/v0.58.0...v0.59.0) (2026-08-26)


### Features

* **constants:** support typed SQL values ([#364](https://github.com/chio-labs/sqlbuild/issues/364)) ([530231d](https://github.com/chio-labs/sqlbuild/commit/530231df3ec731f72a0d0cad7395b07d52f140be))

## [0.58.0](https://github.com/chio-labs/sqlbuild/compare/v0.57.0...v0.58.0) (2026-08-26)


### Features

* **sql:** unify native resource headers ([#362](https://github.com/chio-labs/sqlbuild/issues/362)) ([fbeafab](https://github.com/chio-labs/sqlbuild/commit/fbeafabb57e02a25a4da4c3771928a7191e806d6))

## [0.57.0](https://github.com/chio-labs/sqlbuild/compare/v0.56.2...v0.57.0) (2026-08-26)


### Features

* **kata:** complete native policy verification ([#360](https://github.com/chio-labs/sqlbuild/issues/360)) ([cdd74c6](https://github.com/chio-labs/sqlbuild/commit/cdd74c6a9bff283093e1ed0df63491353779af25))

## [0.56.2](https://github.com/chio-labs/sqlbuild/compare/v0.56.1...v0.56.2) (2026-08-26)


### Bug Fixes

* simplify SQL hook validation ([#358](https://github.com/chio-labs/sqlbuild/issues/358)) ([7222e59](https://github.com/chio-labs/sqlbuild/commit/7222e5954753d9c83327a339dcb1661dc6977ab7))

## [0.56.1](https://github.com/chio-labs/sqlbuild/compare/v0.56.0...v0.56.1) (2026-08-25)


### Documentation

* support navigation root pages ([#356](https://github.com/chio-labs/sqlbuild/issues/356)) ([021fa1f](https://github.com/chio-labs/sqlbuild/commit/021fa1ffccd6c17ce2b9eddd9a6fc8cefe83b322))

## [0.56.0](https://github.com/chio-labs/sqlbuild/compare/v0.55.9...v0.56.0) (2026-08-25)


### Features

* add first-class SQL hook resources ([#354](https://github.com/chio-labs/sqlbuild/issues/354)) ([499a083](https://github.com/chio-labs/sqlbuild/commit/499a0838024341e6d1d535a3d3b5b77dc6d02cef))

## [0.55.9](https://github.com/chio-labs/sqlbuild/compare/v0.55.8...v0.55.9) (2026-08-25)


### Documentation

* clarify standalone and dbt positioning ([#351](https://github.com/chio-labs/sqlbuild/issues/351)) ([68e3d68](https://github.com/chio-labs/sqlbuild/commit/68e3d68cbeed6048ba23d4e6cb4458e1ebe2dacb))

## [0.55.8](https://github.com/chio-labs/sqlbuild/compare/v0.55.7...v0.55.8) (2026-08-25)


### Performance Improvements

* **cli:** lazy-load command handlers ([#347](https://github.com/chio-labs/sqlbuild/issues/347)) ([77b1b1d](https://github.com/chio-labs/sqlbuild/commit/77b1b1d6744eda784e6cbd58fc857d009c7a1894))

## [0.55.7](https://github.com/chio-labs/sqlbuild/compare/v0.55.6...v0.55.7) (2026-08-25)


### Performance Improvements

* **compiler:** reduce warm compile overhead ([#345](https://github.com/chio-labs/sqlbuild/issues/345)) ([360766e](https://github.com/chio-labs/sqlbuild/commit/360766e64d77c22d522d1b71dcd28c623387a2b6))

## [0.55.6](https://github.com/chio-labs/sqlbuild/compare/v0.55.5...v0.55.6) (2026-08-25)


### Performance Improvements

* upgrade Polyglot and increase E2E workers ([#343](https://github.com/chio-labs/sqlbuild/issues/343)) ([30832e9](https://github.com/chio-labs/sqlbuild/commit/30832e9e4eada2f6baf25ea9b903d12f2d707076))

## [0.55.5](https://github.com/chio-labs/sqlbuild/compare/v0.55.4...v0.55.5) (2026-08-25)


### Documentation

* simplify README positioning and CI scope ([#341](https://github.com/chio-labs/sqlbuild/issues/341)) ([7a9b39d](https://github.com/chio-labs/sqlbuild/commit/7a9b39d72a1c4393bb711e603fc6ef9d76670d82))

## [0.55.4](https://github.com/chio-labs/sqlbuild/compare/v0.55.3...v0.55.4) (2026-08-25)


### Performance Improvements

* **compiler:** parallelize cache closure analysis ([#338](https://github.com/chio-labs/sqlbuild/issues/338)) ([64f15f3](https://github.com/chio-labs/sqlbuild/commit/64f15f361e169597f17cd5f4663531b5ece31206))

## [0.55.3](https://github.com/chio-labs/sqlbuild/compare/v0.55.2...v0.55.3) (2026-08-25)


### Performance Improvements

* **compiler:** scope schema cache invalidation ([#336](https://github.com/chio-labs/sqlbuild/issues/336)) ([ac797bf](https://github.com/chio-labs/sqlbuild/commit/ac797bfd9be0ee89e7139d7884b332e9bdfb240f))

## [0.55.2](https://github.com/chio-labs/sqlbuild/compare/v0.55.1...v0.55.2) (2026-08-24)


### Performance Improvements

* **compiler:** complete large-project roadmap ([#334](https://github.com/chio-labs/sqlbuild/issues/334)) ([b482ee9](https://github.com/chio-labs/sqlbuild/commit/b482ee966f3f1556840aa0a8c589306387486147))

## [0.55.1](https://github.com/chio-labs/sqlbuild/compare/v0.55.0...v0.55.1) (2026-08-24)


### Performance Improvements

* **compiler:** scan SQL references linearly ([#329](https://github.com/chio-labs/sqlbuild/issues/329)) ([bd45462](https://github.com/chio-labs/sqlbuild/commit/bd4546227ba100fb3bd6bc5462e175a65889c189))

## [0.55.0](https://github.com/chio-labs/sqlbuild/compare/v0.54.2...v0.55.0) (2026-08-24)


### Features

* add concurrent microbatch recovery ([#326](https://github.com/chio-labs/sqlbuild/issues/326)) ([6092202](https://github.com/chio-labs/sqlbuild/commit/6092202ad9a3b6b001f7e7a3006d2c22d0c17994))
* add Snowflake build cost telemetry ([#325](https://github.com/chio-labs/sqlbuild/issues/325)) ([bf7af06](https://github.com/chio-labs/sqlbuild/commit/bf7af06d9836f774c821fbba0cffefd25d923caf))
* **compiler:** add reusable model schemas ([#323](https://github.com/chio-labs/sqlbuild/issues/323)) ([9e9bd48](https://github.com/chio-labs/sqlbuild/commit/9e9bd4831e4b41aa6f9a6c49ce21d626fdcaf45f))
* support managed table functions and cursor bounds ([#322](https://github.com/chio-labs/sqlbuild/issues/322)) ([49915f9](https://github.com/chio-labs/sqlbuild/commit/49915f9e593d0cb70c8b3c84967f4340b0600f9a))


### Bug Fixes

* **cli:** support interactive connection authentication ([#328](https://github.com/chio-labs/sqlbuild/issues/328)) ([dd30532](https://github.com/chio-labs/sqlbuild/commit/dd3053266468dde063828672b5566ed38fd6bfd3))
* **compiler:** deduplicate model schema audits ([#324](https://github.com/chio-labs/sqlbuild/issues/324)) ([41414d4](https://github.com/chio-labs/sqlbuild/commit/41414d4ffd4bc503ddaff3938f8a76d06b151fa0))
* **compiler:** preserve custom cast type names ([#327](https://github.com/chio-labs/sqlbuild/issues/327)) ([bbbe62d](https://github.com/chio-labs/sqlbuild/commit/bbbe62d78cd5e0656dee70907fa86b9147b5d237))
* **release:** use delivery app for release PRs ([#332](https://github.com/chio-labs/sqlbuild/issues/332)) ([f2d7956](https://github.com/chio-labs/sqlbuild/commit/f2d79562c54d15769c1a86936fbe2273ff54c8be))
* **virtual:** serialize microbatch state operations ([#330](https://github.com/chio-labs/sqlbuild/issues/330)) ([4f14b39](https://github.com/chio-labs/sqlbuild/commit/4f14b3971b43347ae020ae43b540ee8d65eb8df8))

## [0.54.2](https://github.com/chio-labs/sqlbuild/compare/v0.54.1...v0.54.2) (2026-08-23)


### Bug Fixes

* **kata:** enable standard policy and tighten enums ([#320](https://github.com/chio-labs/sqlbuild/issues/320)) ([076e041](https://github.com/chio-labs/sqlbuild/commit/076e0410da88ae207ead9ba303f5a61d360be098))

## [0.54.1](https://github.com/chio-labs/sqlbuild/compare/v0.54.0...v0.54.1) (2026-08-23)


### Bug Fixes

* include license in source distributions ([#318](https://github.com/chio-labs/sqlbuild/issues/318)) ([fbf84bb](https://github.com/chio-labs/sqlbuild/commit/fbf84bb695308e721cb11e803dc37cfe1ba1b89c))

## [0.54.0](https://github.com/chio-labs/sqlbuild/compare/v0.53.0...v0.54.0) (2026-08-23)


### Features

* add kata SQL architecture checks ([#314](https://github.com/chio-labs/sqlbuild/issues/314)) ([0bdb16e](https://github.com/chio-labs/sqlbuild/commit/0bdb16efade964374adc7d1a9537c00d21544f2b))

## [0.53.0](https://github.com/chio-labs/sqlbuild/compare/v0.52.0...v0.53.0) (2026-08-22)


### Features

* **incremental:** guard selective merge state ([#315](https://github.com/chio-labs/sqlbuild/issues/315)) ([9126551](https://github.com/chio-labs/sqlbuild/commit/9126551293221d88b290c9dfc46f9e46aac573b2))

## [0.52.0](https://github.com/chio-labs/sqlbuild/compare/v0.51.0...v0.52.0) (2026-08-21)


### Features

* **compiler:** add expansion span models and offset mapping ([9bfbd7b](https://github.com/chio-labs/sqlbuild/commit/9bfbd7b09e986de42b3a0b1dfd29890a58d0e3dc))
* **compiler:** emit substitution spans from every expansion pass ([1ce9d32](https://github.com/chio-labs/sqlbuild/commit/1ce9d322faf17542fc72c938d78c2851d7f8f25b))
* **lint:** add sqb lint and sqb format with sqruff and native header rules ([c4e47cd](https://github.com/chio-labs/sqlbuild/commit/c4e47cda9b1e6f6a49eb66be03437633710cb615))
* **lint:** lint the SQL the project actually produces ([1833273](https://github.com/chio-labs/sqlbuild/commit/18332735d5825a29c3f23e14c442bff9f7f2568f))
* **lint:** neutralize sqlbuild interpolation with unique sentinels ([e99a4be](https://github.com/chio-labs/sqlbuild/commit/e99a4befbc8e32671d4857f5eccba68d1d3d4bf0))
* **lint:** reject unsupported sqruff dialects instead of linting silently ([a09157b](https://github.com/chio-labs/sqlbuild/commit/a09157b570aa4978083be85df866f524b60d0ca2))
* **lint:** translate adapter dialect and scaffold .sqruff on lint/format ([504c425](https://github.com/chio-labs/sqlbuild/commit/504c425c630cda1f7e946f86984a535c9fe464a1))


### Bug Fixes

* **lint:** handle scoped declarations and formatting edge cases ([f367ed9](https://github.com/chio-labs/sqlbuild/commit/f367ed98cde77652250a7962af6376c68ba231dc))
* **lint:** satisfy policy checks for formatting fixes ([76cc7cd](https://github.com/chio-labs/sqlbuild/commit/76cc7cd3269fe9eed3751c51a6ed6c36162f9559))
* **plan:** render all model change diffs ([f57ac80](https://github.com/chio-labs/sqlbuild/commit/f57ac80d8fe210ae0fa4b0af5ab676ae26adc97c))
* **plan:** render all model change diffs ([b8ae726](https://github.com/chio-labs/sqlbuild/commit/b8ae726b259005bdbcdb80ddb70ca72a0f2e68bf))

## [0.51.0](https://github.com/chio-labs/sqlbuild/compare/v0.50.0...v0.51.0) (2026-08-18)


### Features

* **compiler:** add enum and constant declarations ([9100727](https://github.com/chio-labs/sqlbuild/commit/91007278a55f3a588e04ff0f4401c184bf0e68b7))


### Bug Fixes

* **ci:** keep release lockfile synchronized ([55bba3b](https://github.com/chio-labs/sqlbuild/commit/55bba3bc6ac8a33ee921cdc75f355b68de889264))

## [0.50.0](https://github.com/chio-labs/sqlbuild/compare/v0.49.0...v0.50.0) (2026-08-18)


### Features

* **cli:** adopt balanced color hierarchy for human output ([eac4ddc](https://github.com/chio-labs/sqlbuild/commit/eac4ddc9842a670038a1f57c9821fb6ab92d8fe4))
* **cli:** clarify virtual state and expand previews ([3b94316](https://github.com/chio-labs/sqlbuild/commit/3b943167e06784f8cc174b92ef71ef489b2f67ee))
* **cli:** dim kind columns, accent change actions, fix spinner wrap and error-label padding ([b17b1ef](https://github.com/chio-labs/sqlbuild/commit/b17b1efc31f98fa77c0596707a282373dff95025))
* **cli:** refine hierarchy and add output previews ([7607c4f](https://github.com/chio-labs/sqlbuild/commit/7607c4f08636a00d525fdc406a7b65e4939d8804))
* **cli:** structural output hierarchy across SQLBuild CLI ([7c46d43](https://github.com/chio-labs/sqlbuild/commit/7c46d43174f1e61238e919a4201e37e699b63d80))
* **cli:** structure source freshness output ([c0e42f4](https://github.com/chio-labs/sqlbuild/commit/c0e42f4ef110995d985b9b32e559ae154b657ae7))
* **cli:** structured build surface - phase lines, status cells, dim timings ([e321e11](https://github.com/chio-labs/sqlbuild/commit/e321e11232e414fc9d954c61c23cafd59da5a1d2))
* **cli:** structured clone completion and connection phase lines ([e5a154d](https://github.com/chio-labs/sqlbuild/commit/e5a154d711125854449c8e309eaee285bbb7e7a5))
* **cli:** structured dbt plan, compile, lineage, promote, rollback surfaces ([7c9e51a](https://github.com/chio-labs/sqlbuild/commit/7c9e51aa1f9d4cb9bf0e8e69ac22b11318d038e8))
* **cli:** structured load, seed, test surfaces and e2e/integration expectations ([16667c5](https://github.com/chio-labs/sqlbuild/commit/16667c547bc756a5e42509828ed3287d74f30eff))
* **cli:** structured plan surface - tree entries, dim leaf details, warnings tree ([ded8df1](https://github.com/chio-labs/sqlbuild/commit/ded8df1b4f7a0dc92eb001cefba3b1f23ead9910))
* **cli:** surface failing tests inline and roll up skipped nodes in the build footer ([0be7a8e](https://github.com/chio-labs/sqlbuild/commit/0be7a8e9a6f8cc2801866e71f67bcb9c0cad71c0))


### Bug Fixes

* repair mangled f-string in virtual build state e2e ([90fed70](https://github.com/chio-labs/sqlbuild/commit/90fed70e95943fd0d5f1ddef99462e6d00408a10))


### Documentation

* **readme:** drop stale dbt clone/diff and standard changes-only references ([e2455cd](https://github.com/chio-labs/sqlbuild/commit/e2455cddb58554a8cb1bbde7366e05258af808d5))
* rename chained unit tests to multi-model tests ([89a6bd6](https://github.com/chio-labs/sqlbuild/commit/89a6bd665f4c9feaaff670beb5311a52d249e226))

## [0.49.0](https://github.com/chio-labs/sqlbuild/compare/v0.48.7...v0.49.0) (2026-08-16)


### Features

* **build:** add per-batch spinner timing and mutation-proof tests ([6ad998e](https://github.com/chio-labs/sqlbuild/commit/6ad998ebed037b981f16cec15a977e7d086bcc1b))
* **build:** render whole-day cursor bounds as dates in progress and summaries ([326d0ed](https://github.com/chio-labs/sqlbuild/commit/326d0ed2b3ddde6c499fc0ffa490c54252475a26))
* **build:** report microbatch batch count and cursor range ([ced59ed](https://github.com/chio-labs/sqlbuild/commit/ced59ed1b389ab0bfdf41be213dad8117fc38878))
* **build:** show live batch counter on the active row and batch size in the summary ([8cc8916](https://github.com/chio-labs/sqlbuild/commit/8cc8916363abc3636ad01d29bf217d42cc09a078))
* **build:** stream per-batch progress and batch summary during execution ([d9f790a](https://github.com/chio-labs/sqlbuild/commit/d9f790a23af53e8d6a259f18ab2b7ab404270550))
* **build:** surface row counts from adapter DML through microbatch output ([6e7697c](https://github.com/chio-labs/sqlbuild/commit/6e7697cb6bb00478904a395e0210fb55b736860a))
* **plan:** expose effective microbatch cursor work ([ddd421e](https://github.com/chio-labs/sqlbuild/commit/ddd421ee2e835eb30cd5111adf005f66614c8d1c))


### Bug Fixes

* **ci:** repair full verification failures ([8ce2fd3](https://github.com/chio-labs/sqlbuild/commit/8ce2fd3ec574ed2a4358c4c5510f057e8e3f8cef))
* **cli:** stop truncating batch progress in spinner line ([ca493d7](https://github.com/chio-labs/sqlbuild/commit/ca493d744da853b67dc1816887897ebd10a48dc6))
* **cursor:** honour --start/--end-cursor-ts for model-backed cursor inputs ([e70c044](https://github.com/chio-labs/sqlbuild/commit/e70c0443ccdd13fd458a5c39f313f45a2baf3826))
* **cursor:** resolve branch review findings ([c4a66d4](https://github.com/chio-labs/sqlbuild/commit/c4a66d4d744f09aa4f0b7d0c13cc6b6e1e59f236))
* **cursors:** include the final cursor value for date and decimal bounds ([889c267](https://github.com/chio-labs/sqlbuild/commit/889c267a56a807614791f90aa4c4ecd5e024948a))
* **cursors:** treat operator --end-cursor-ts as inclusive via one canonical step ([7096596](https://github.com/chio-labs/sqlbuild/commit/709659671489597d3896e67698df014b6b3ef68d))
* **plan:** make the query-changed warning consequence-first and actionable ([b373d2a](https://github.com/chio-labs/sqlbuild/commit/b373d2abb4a9f67cf318239438feb00b0e9c22eb))
* **planner:** compare warehouse state schemas case-insensitively ([7eae7f1](https://github.com/chio-labs/sqlbuild/commit/7eae7f12ddf621b7d2c41cc654fba4af24168d52))
* **plan:** show the resolved inclusive cursor range for microbatch models ([861b976](https://github.com/chio-labs/sqlbuild/commit/861b97636bf0c4781c6d9d8326bd55696270ba4c))

## [0.48.7](https://github.com/chio-labs/sqlbuild/compare/v0.48.6...v0.48.7) (2026-08-02)


### Bug Fixes

* **loaders:** isolate schemas and clone managed sources ([253cd2b](https://github.com/chio-labs/sqlbuild/commit/253cd2b44732cc34adf6e753bf0746f3d8f648b9))
* **loaders:** isolate schemas and clone managed sources ([3003167](https://github.com/chio-labs/sqlbuild/commit/300316787a6bf5678a942a5534f986c0c7cbf7d1))


### Documentation

* surface local configuration workflow ([ae7cdd7](https://github.com/chio-labs/sqlbuild/commit/ae7cdd703c4d6329c8697ec6da4a95fc4bebe007))

## [0.48.6](https://github.com/chio-labs/sqlbuild/compare/v0.48.5...v0.48.6) (2026-08-02)


### Bug Fixes

* **cli:** complete query and SQL Server diff parity ([29b131a](https://github.com/chio-labs/sqlbuild/commit/29b131ac55785ee6b4b1e4da18ed525dc46a4bda))
* **cli:** complete query and SQL Server diff parity ([1084a1e](https://github.com/chio-labs/sqlbuild/commit/1084a1e05708e0b0f626dcd9177cfe7160a9d330))

## [0.48.5](https://github.com/chio-labs/sqlbuild/compare/v0.48.4...v0.48.5) (2026-08-01)


### Bug Fixes

* **cli:** scope freshness diagnostics to changes-only ([dc7f043](https://github.com/chio-labs/sqlbuild/commit/dc7f043609dbc883236d1a453e1773095a35c75b))
* **cli:** scope freshness diagnostics to changes-only ([07bd489](https://github.com/chio-labs/sqlbuild/commit/07bd489c1bc4f79d549c8448b680eba2abebf959))

## [0.48.4](https://github.com/chio-labs/sqlbuild/compare/v0.48.3...v0.48.4) (2026-08-01)


### Bug Fixes

* **sqlserver:** type loader cursor delete bounds ([79c2213](https://github.com/chio-labs/sqlbuild/commit/79c221364f83612f3cdd512e4fe9852e1f4d01f8))
* **sqlserver:** type loader cursor delete bounds ([7a53481](https://github.com/chio-labs/sqlbuild/commit/7a534813859723e331214476a00c6c38fce0b7f4))

## [0.48.3](https://github.com/chio-labs/sqlbuild/compare/v0.48.2...v0.48.3) (2026-08-01)


### Bug Fixes

* **sqlserver:** harden test SQL and cursor bounds ([d73edbf](https://github.com/chio-labs/sqlbuild/commit/d73edbf77d1c8239fd6d300c7b5fb9c24ef4e0b5))
* **sqlserver:** harden test SQL and cursor bounds ([8e5fb24](https://github.com/chio-labs/sqlbuild/commit/8e5fb24bbadac12d67ccd833920b707e22243c0d))

## [0.48.2](https://github.com/chio-labs/sqlbuild/compare/v0.48.1...v0.48.2) (2026-07-31)


### Bug Fixes

* preserve build errors and normalize schema types ([4753e41](https://github.com/chio-labs/sqlbuild/commit/4753e418fd0add3161435727321902d9ed051f88))
* preserve build errors and normalize schema types ([3480cd1](https://github.com/chio-labs/sqlbuild/commit/3480cd168887c3c1b2e2abd0a821fba53f823768))

## [0.48.1](https://github.com/chio-labs/sqlbuild/compare/v0.48.0...v0.48.1) (2026-07-30)


### Bug Fixes

* **dbt:** simplify interop planning and test accounting ([320e908](https://github.com/chio-labs/sqlbuild/commit/320e9081eb3da0b22dc0cb1e7d3e2da24b5ade35))
* **dbt:** simplify interop planning and test accounting ([2f87b66](https://github.com/chio-labs/sqlbuild/commit/2f87b666c6372279dc3c54ed3afcf0b3c5360e87))

## [0.48.0](https://github.com/chio-labs/sqlbuild/compare/v0.47.0...v0.48.0) (2026-07-23)


### Features

* add dupscore duplication-risk advisory tool ([4b791de](https://github.com/chio-labs/sqlbuild/commit/4b791dea48f445fa4b05bcbb5d48a4001b834052))


### Bug Fixes

* converge virtual planning and lifecycle identity ([867fab7](https://github.com/chio-labs/sqlbuild/commit/867fab7c105efaf95dc17ac63d279f2c94a5840b))
* resolve current physical seeds in virtual plan buildability ([c3f7256](https://github.com/chio-labs/sqlbuild/commit/c3f72569a5b7acc88c442836e42642cbb0bf2b28))

## [0.47.0](https://github.com/chio-labs/sqlbuild/compare/v0.46.1...v0.47.0) (2026-07-17)


### Features

* add SQLBuild Strata policies ([d097d33](https://github.com/chio-labs/sqlbuild/commit/d097d33c46b28a090acd489edee9c1450a9c8e8d))


### Bug Fixes

* make changes-only execution opt-in ([945570c](https://github.com/chio-labs/sqlbuild/commit/945570c189d34118dbaf0101ac54987a641c3660))
* make changes-only execution opt-in ([4ffa692](https://github.com/chio-labs/sqlbuild/commit/4ffa692e82a2de6a81949ef22e8b5fb42ac8949a))
* restore public positional API compatibility ([6c47496](https://github.com/chio-labs/sqlbuild/commit/6c4749609e5fdef3d7927393230cbe700cd980d4))
* update playground adapter contract import ([28712b4](https://github.com/chio-labs/sqlbuild/commit/28712b48c2e22854a96fca6c73e403c0ba993f46))


### Performance Improvements

* make custom Strata rules cacheable ([0140242](https://github.com/chio-labs/sqlbuild/commit/01402429008d5cb51e96aeff717652bfa81623c2))
* make custom Strata rules cacheable ([a17bb55](https://github.com/chio-labs/sqlbuild/commit/a17bb5555239b8e4ee0245e5d2c99e3bd8408736))

## [0.46.1](https://github.com/chio-labs/sqlbuild/compare/v0.46.0...v0.46.1) (2026-07-04)


### Bug Fixes

* align dbt selection staleness warnings ([bc9e73b](https://github.com/chio-labs/sqlbuild/commit/bc9e73b12ba8c091c9611dec449b51ab5f438f09))

## [0.46.0](https://github.com/chio-labs/sqlbuild/compare/v0.45.5...v0.46.0) (2026-07-02)


### Features

* add node source watermark state ([aaac057](https://github.com/chio-labs/sqlbuild/commit/aaac0577bc3255648ab6ddba3b9deab3c9968305))
* add watermark frontier resolution ([554294a](https://github.com/chio-labs/sqlbuild/commit/554294aa6b6483ff46c8b5207cec2cb2d0bc8727))
* complete watermark stale input handling ([5ca0e56](https://github.com/chio-labs/sqlbuild/commit/5ca0e56ba0ffabe366dbf18bf7830d309e6c57b5))
* warn on stale node source watermarks ([8afb164](https://github.com/chio-labs/sqlbuild/commit/8afb16470375108e9d3cf4de7f7c7a289fe2c310))
* wire dbt node source watermarks ([763b0e6](https://github.com/chio-labs/sqlbuild/commit/763b0e6cee741555791dc05f185b3ca53e36945e))
* wire native node source watermarks ([380eac9](https://github.com/chio-labs/sqlbuild/commit/380eac93f6ef7dd1231794fc3350b0c1165a3741))


### Bug Fixes

* batch dbt lineage schema inspection ([8f44588](https://github.com/chio-labs/sqlbuild/commit/8f445883620990f94dbfaba7cbc64ecfb9f0a271))
* refresh dbt prephase status output ([59a842b](https://github.com/chio-labs/sqlbuild/commit/59a842b96987a142b1e050a4684206544365147f))
* refresh dbt prephase status output ([42a142b](https://github.com/chio-labs/sqlbuild/commit/42a142b43dfc3bbcb4a6e8966ba5cef02f2d40de))

## [0.45.5](https://github.com/chio-labs/sqlbuild/compare/v0.45.4...v0.45.5) (2026-06-28)


### Bug Fixes

* polish defer clone prephase output ([f533d1f](https://github.com/chio-labs/sqlbuild/commit/f533d1f578c06ab7e2b90936a993d2a9d0bf1e30))
* polish defer clone prephase output ([97436cb](https://github.com/chio-labs/sqlbuild/commit/97436cbb57ac9b5e64982fc5d4172d593b15e843))

## [0.45.4](https://github.com/chio-labs/sqlbuild/compare/v0.45.3...v0.45.4) (2026-06-28)


### Bug Fixes

* clean defer clone prephase output ([dc50d2d](https://github.com/chio-labs/sqlbuild/commit/dc50d2dcf9e1f24e31e962b5087152dc8e63e0f7))
* clean defer clone prephase output ([16acf85](https://github.com/chio-labs/sqlbuild/commit/16acf856cc1c33b1456cee5fc06322719b05d28e))

## [0.45.3](https://github.com/chio-labs/sqlbuild/compare/v0.45.2...v0.45.3) (2026-06-28)


### Bug Fixes

* stream defer clone prephase progress ([bb2956f](https://github.com/chio-labs/sqlbuild/commit/bb2956f99bd742c7bcc80120c32e4d3daaa4684c))
* stream defer clone prephase progress ([f3c1398](https://github.com/chio-labs/sqlbuild/commit/f3c139803c6b1a6d58d14fe9bfac0b851aa0cbc7))

## [0.45.2](https://github.com/chio-labs/sqlbuild/compare/v0.45.1...v0.45.2) (2026-06-28)


### Bug Fixes

* **dbt:** isolate defer clone prephase output ([9806c24](https://github.com/chio-labs/sqlbuild/commit/9806c24e4dfac3362b78e42dffc5af61248a8912))
* **dbt:** isolate defer clone prephase output ([4a6e9ff](https://github.com/chio-labs/sqlbuild/commit/4a6e9ff60393de9904b62bef5c6fae3880216456))

## [0.45.1](https://github.com/chio-labs/sqlbuild/compare/v0.45.0...v0.45.1) (2026-06-28)


### Bug Fixes

* **dbt:** defer clone pure dbt selections ([299fc98](https://github.com/chio-labs/sqlbuild/commit/299fc98c38782844ca4574068e87c3857717bfc6))
* **dbt:** defer clone pure dbt selections ([978bbdd](https://github.com/chio-labs/sqlbuild/commit/978bbdd36dd8f91a2de8549fb7b8dbd0cb7929ed))

## [0.45.0](https://github.com/chio-labs/sqlbuild/compare/v0.44.4...v0.45.0) (2026-06-28)


### Features

* configure defer clone from project config ([6242def](https://github.com/chio-labs/sqlbuild/commit/6242def3f72b6d14bae98bd680143793ce71aa94))


### Performance Improvements

* optimize source freshness metadata planning ([ac737bf](https://github.com/chio-labs/sqlbuild/commit/ac737bf3f9135668260107d4819172643031c8f0))

## [0.44.4](https://github.com/chio-labs/sqlbuild/compare/v0.44.3...v0.44.4) (2026-06-28)


### Bug Fixes

* **dbt:** order clone views after dependencies ([e135b50](https://github.com/chio-labs/sqlbuild/commit/e135b5004752d3ad8926abb357a1035566b63f7e))
* **dbt:** order clone views after dependencies ([f201639](https://github.com/chio-labs/sqlbuild/commit/f201639edf3a2cc9b74d2144446ae5965387b76a))

## [0.44.3](https://github.com/chio-labs/sqlbuild/compare/v0.44.2...v0.44.3) (2026-06-28)


### Bug Fixes

* **dbt:** align clone output with native clone ([7aecc9e](https://github.com/chio-labs/sqlbuild/commit/7aecc9ec3572754bdd2c08c379bd4b54b5fb616e))
* **dbt:** align clone output with native clone ([43f50c2](https://github.com/chio-labs/sqlbuild/commit/43f50c25ae307a02db89cd7771d2acdc7a47f78b))

## [0.44.2](https://github.com/chio-labs/sqlbuild/compare/v0.44.1...v0.44.2) (2026-06-28)


### Bug Fixes

* **dbt:** make clone selection fail-safe ([ecf7f81](https://github.com/chio-labs/sqlbuild/commit/ecf7f81c7d1caf4b824459861b621f9b00357945))
* **dbt:** make clone selection fail-safe ([cbbb4fb](https://github.com/chio-labs/sqlbuild/commit/cbbb4fbc6e09cfebfae5c61c13fff9915dd5e41a))

## [0.44.1](https://github.com/chio-labs/sqlbuild/compare/v0.44.0...v0.44.1) (2026-06-28)


### Bug Fixes

* **dbt:** show persistent node start progress ([08bb7e2](https://github.com/chio-labs/sqlbuild/commit/08bb7e2a71efbf5d7d2777594e94e9612e5b4002))
* **dbt:** show persistent node start progress ([ca08eb6](https://github.com/chio-labs/sqlbuild/commit/ca08eb6008dae01580d155234d00be55e623a4e0))

## [0.44.0](https://github.com/chio-labs/sqlbuild/compare/v0.43.0...v0.44.0) (2026-06-28)


### Features

* **dbt:** consolidate native and dbt planning helpers ([d99a437](https://github.com/chio-labs/sqlbuild/commit/d99a437a992861567eb5a7133554f042da35c9f7))
* **structure:** prevent dbt planning drift ([91825b1](https://github.com/chio-labs/sqlbuild/commit/91825b152ec59f15da9dba563b82a450955c63e4))

## [0.43.0](https://github.com/chio-labs/sqlbuild/compare/v0.42.1...v0.43.0) (2026-06-27)


### Features

* **structure:** add SC051 flagging warehouse metadata calls inside loops ([36456c8](https://github.com/chio-labs/sqlbuild/commit/36456c824eba3a0f27fa6bf5f0e398e92dbda188))
* **structure:** make SC051 transitive ([dae144d](https://github.com/chio-labs/sqlbuild/commit/dae144d7736f5b69168e257712d315ef5ec05274))
* **structure:** make SC051 transitive and fix the hidden helper-in-loop N+1s ([ef5de4b](https://github.com/chio-labs/sqlbuild/commit/ef5de4b43ea8145ba3bf5d5d9c7b3f23b4e9a534))


### Performance Improvements

* collapse per-item warehouse metadata lookups into one batched RelationLookup ([f5a75b9](https://github.com/chio-labs/sqlbuild/commit/f5a75b922d7e55cdefb381cf47ade319bc4c772e))

## [0.42.1](https://github.com/chio-labs/sqlbuild/compare/v0.42.0...v0.42.1) (2026-06-26)


### Bug Fixes

* **snowflake:** default tables to transient and mirror origin transient-ness on clone ([61b2146](https://github.com/chio-labs/sqlbuild/commit/61b2146afd1a40223d8acdba8a3a0c6d48ca6df9))

## [0.42.0](https://github.com/chio-labs/sqlbuild/compare/v0.41.3...v0.42.0) (2026-06-26)


### Features

* **clone:** stream per-relation progress for native and dbt clone with shared op core ([9fec3d7](https://github.com/chio-labs/sqlbuild/commit/9fec3d78796b338741e191f4afba8db57d0bf44c))


### Bug Fixes

* **planner:** collapse redundant information_schema prechecks into one relation scan ([64e3834](https://github.com/chio-labs/sqlbuild/commit/64e3834605c1b66e6f29beecf55c422363563610))

## [0.41.3](https://github.com/chio-labs/sqlbuild/compare/v0.41.2...v0.41.3) (2026-06-26)


### Bug Fixes

* **dbt:** resolve seed existence from the bulk relation set instead of per-seed queries ([b7aaefb](https://github.com/chio-labs/sqlbuild/commit/b7aaefb4f79c7c021c9da802e380c517b9511cf1))
* **dbt:** resolve seed existence from the bulk relation set instead of per-seed queries ([7a8639d](https://github.com/chio-labs/sqlbuild/commit/7a8639db6570cc3f07199ef1726f13ea07591907))
* **planner:** restructure stale-upstream warning into a capped bulleted message ([68e8d59](https://github.com/chio-labs/sqlbuild/commit/68e8d591f16789efa69ffb916b515c15dfae899a))
* **planner:** restructure stale-upstream warning into a capped bullets ([ccdeb2d](https://github.com/chio-labs/sqlbuild/commit/ccdeb2d75d1cce3e04f51d66544ae9d76ee58af1))

## [0.41.2](https://github.com/chio-labs/sqlbuild/compare/v0.41.1...v0.41.2) (2026-06-26)


### Bug Fixes

* **dbt:** run independent planning state reads in parallel ([76d60a0](https://github.com/chio-labs/sqlbuild/commit/76d60a0af5585926a40049bef88e51e455e02db0))
* **dbt:** run independent planning state reads in parallel ([d422783](https://github.com/chio-labs/sqlbuild/commit/d422783b5ee7ffec71becb02543ca2fb0aa917fa))

## [0.41.1](https://github.com/chio-labs/sqlbuild/compare/v0.41.0...v0.41.1) (2026-06-26)


### Bug Fixes

* **planner:** memoize write-identity hashes to avoid exponential hang ([df812df](https://github.com/chio-labs/sqlbuild/commit/df812df5762a45bf07d3cdec68ff9c23d5076cdb))
* **planner:** memoize write-identity hashes to avoid exponential hang ([c98f865](https://github.com/chio-labs/sqlbuild/commit/c98f8658df26080c9dc9ee274b165210f922086a))

## [0.41.0](https://github.com/chio-labs/sqlbuild/compare/v0.40.1...v0.41.0) (2026-06-26)


### Features

* **build:** add --defer-clone-from and per-command --target ([237b8b9](https://github.com/chio-labs/sqlbuild/commit/237b8b9650baa43789919801a4bb9198e6b7deab))
* **build:** add --defer-clone-from and per-command --target ([c451b02](https://github.com/chio-labs/sqlbuild/commit/c451b020aedd027f70170c89398736e47259b14f))
* **dbt:** persist a Finalizing dbt run progress line after the event stream ([4b93f42](https://github.com/chio-labs/sqlbuild/commit/4b93f4207f71817f8ff071a68b4b49b687dad529))
* **dbt:** remove dbt reuse-from-production, keep change-aware builds + clone/diff ([fd6243a](https://github.com/chio-labs/sqlbuild/commit/fd6243a390ae853e7226e39ddbec0c3eb2767a9a))
* **dbt:** remove sqb dbt identity-diff (dead dbt reuse preview) ([ba8d19d](https://github.com/chio-labs/sqlbuild/commit/ba8d19d687c469a5298160b17c276261de7dd2f4))
* **dbt:** remove sqb dbt identity-diff (dead dbt reuse preview) ([4eca08e](https://github.com/chio-labs/sqlbuild/commit/4eca08e6eccd1b44ed85655c31835df2ae2800e2))
* **dbt:** render a completion status and PASS/WARN/FAIL/SKIP/TOTAL f… ([d40576b](https://github.com/chio-labs/sqlbuild/commit/d40576b7c353cf59eb36de213753e092156dea11))
* **dbt:** render a completion status and PASS/WARN/FAIL/SKIP/TOTAL footer for dbt build ([b3a2447](https://github.com/chio-labs/sqlbuild/commit/b3a2447c1873e94bf27e7eaaef33d107383c8230))
* **dbt:** report progress during post-build state persistence ([d2d1ba1](https://github.com/chio-labs/sqlbuild/commit/d2d1ba1141cebf8740d54044a0a991fd17c280c3))
* **reuse:** enforce strict native reuse and render source freshness via adapter ([beca26e](https://github.com/chio-labs/sqlbuild/commit/beca26ecf6d6f371629f83e59e5ea783241d8416))


### Bug Fixes

* **dbt:** batch model relation planning checks ([19105d7](https://github.com/chio-labs/sqlbuild/commit/19105d7a6b1146a6b68015d99fad0cb599d83625))
* **dbt:** ensure fingerprint schema exists before writing dbt fingerprints ([5fb28a8](https://github.com/chio-labs/sqlbuild/commit/5fb28a8a7029a8f82761cc085ad858b91e89b49f))
* **dbt:** only run changed seeds when models build ([2f5d3a4](https://github.com/chio-labs/sqlbuild/commit/2f5d3a4de7e1ec8a36e0bd8af02d54c7dde211c3))
* **reuse:** satisfy type and test-convention checks for reuse rework ([7ff2829](https://github.com/chio-labs/sqlbuild/commit/7ff2829f9f0ce6c3e275817223a8435a1bf610f3))
* sync release-please manifest to 0.40.1 after revert ([0be12c6](https://github.com/chio-labs/sqlbuild/commit/0be12c637f28ec462b5cc92b265a07881445b764))
* sync release-please manifest to 0.40.1 after revert ([cfd5bcb](https://github.com/chio-labs/sqlbuild/commit/cfd5bcbe15616315447a594450aea3f7d564785b))
* **virtual:** disable reuse planning in virtual environment plans ([309a17d](https://github.com/chio-labs/sqlbuild/commit/309a17d8b5f6917979dd79a7824926bc1c6d620f))

## [0.39.0](https://github.com/chio-labs/sqlbuild/compare/v0.38.0...v0.39.0) (2026-06-26)


### Features

* align reuse policy across native and dbt ([6ef4b8d](https://github.com/chio-labs/sqlbuild/commit/6ef4b8dd08ddc241cde1aaaaa455ff22c977e655))
* align reuse policy across native and dbt ([d50f9f8](https://github.com/chio-labs/sqlbuild/commit/d50f9f868b7102ff15144b7d896197348f81eb56))
* **build:** add --defer-clone-from and per-command --target ([237b8b9](https://github.com/chio-labs/sqlbuild/commit/237b8b9650baa43789919801a4bb9198e6b7deab))
* **build:** add --defer-clone-from and per-command --target ([c451b02](https://github.com/chio-labs/sqlbuild/commit/c451b020aedd027f70170c89398736e47259b14f))
* **dbt:** add identity diff command ([1d7627e](https://github.com/chio-labs/sqlbuild/commit/1d7627e70561081ec0c58691184dde3b5a731397))
* **dbt:** add identity diff command ([900c155](https://github.com/chio-labs/sqlbuild/commit/900c1555d14364bb1b9b9464bd9bdb099bb0d5ce))
* **dbt:** persist a Finalizing dbt run progress line after the event stream ([4b93f42](https://github.com/chio-labs/sqlbuild/commit/4b93f4207f71817f8ff071a68b4b49b687dad529))
* **dbt:** remove dbt reuse-from-production, keep change-aware builds + clone/diff ([fd6243a](https://github.com/chio-labs/sqlbuild/commit/fd6243a390ae853e7226e39ddbec0c3eb2767a9a))
* **dbt:** remove sqb dbt identity-diff (dead dbt reuse preview) ([ba8d19d](https://github.com/chio-labs/sqlbuild/commit/ba8d19d687c469a5298160b17c276261de7dd2f4))
* **dbt:** remove sqb dbt identity-diff (dead dbt reuse preview) ([4eca08e](https://github.com/chio-labs/sqlbuild/commit/4eca08e6eccd1b44ed85655c31835df2ae2800e2))
* **dbt:** render a completion status and PASS/WARN/FAIL/SKIP/TOTAL f… ([d40576b](https://github.com/chio-labs/sqlbuild/commit/d40576b7c353cf59eb36de213753e092156dea11))
* **dbt:** render a completion status and PASS/WARN/FAIL/SKIP/TOTAL footer for dbt build ([b3a2447](https://github.com/chio-labs/sqlbuild/commit/b3a2447c1873e94bf27e7eaaef33d107383c8230))
* **dbt:** report progress during post-build state persistence ([d2d1ba1](https://github.com/chio-labs/sqlbuild/commit/d2d1ba1141cebf8740d54044a0a991fd17c280c3))
* **reuse:** enforce strict native reuse and render source freshness via adapter ([beca26e](https://github.com/chio-labs/sqlbuild/commit/beca26ecf6d6f371629f83e59e5ea783241d8416))


### Bug Fixes

* **dbt:** align identity diff with reuse identity ([d9bee2c](https://github.com/chio-labs/sqlbuild/commit/d9bee2c88556ae3b9b7f308e6803edac18f8f760))
* **dbt:** align identity diff with reuse identity ([dbf00f7](https://github.com/chio-labs/sqlbuild/commit/dbf00f7e510966f3aeea3b9882215422e78770b8))
* **dbt:** batch model relation planning checks ([19105d7](https://github.com/chio-labs/sqlbuild/commit/19105d7a6b1146a6b68015d99fad0cb599d83625))
* **dbt:** batch model relation planning checks ([ffa4b90](https://github.com/chio-labs/sqlbuild/commit/ffa4b906f94660566b1c54b033031f8dab231c6a))
* **dbt:** batch model relation planning checks ([3d88523](https://github.com/chio-labs/sqlbuild/commit/3d885238efe7f0e71ae5bc60b94a812f823b5f7e))
* **dbt:** bound identity diff rendering ([d9ff0af](https://github.com/chio-labs/sqlbuild/commit/d9ff0afd88d27331fbdd309b467481023c188bf4))
* **dbt:** bound identity diff rendering ([3064094](https://github.com/chio-labs/sqlbuild/commit/306409418dda424fcc4bd011b176430348b75370))
* **dbt:** bound reuse git refresh ([bd40a8c](https://github.com/chio-labs/sqlbuild/commit/bd40a8c6abef5c08e6940391ba146ffc5543d490))
* **dbt:** bound reuse git refresh ([0bbf200](https://github.com/chio-labs/sqlbuild/commit/0bbf2006c70f6248f05f43d2a62d3d3832838539))
* **dbt:** cache reuse manifests ([6562e29](https://github.com/chio-labs/sqlbuild/commit/6562e29841493ffa6476957acfc3f9812e27b5d0))
* **dbt:** cache reuse manifests ([bbe1bc3](https://github.com/chio-labs/sqlbuild/commit/bbe1bc33b63dc91cc69175610775b8631e340a8c))
* **dbt:** ensure fingerprint schema exists before writing dbt fingerprints ([5fb28a8](https://github.com/chio-labs/sqlbuild/commit/5fb28a8a7029a8f82761cc085ad858b91e89b49f))
* **dbt:** improve state and execution progress ([24f1e90](https://github.com/chio-labs/sqlbuild/commit/24f1e90743d2b88faf5d20e2f4ca3d766941f3da))
* **dbt:** improve state and execution progress ([f7a9227](https://github.com/chio-labs/sqlbuild/commit/f7a92279a89fdba31e2ccb5ace57a78b76012068))
* **dbt:** improve target and routing resolution ([8235bc7](https://github.com/chio-labs/sqlbuild/commit/8235bc7f0667fb48a3fee55f4f4a762cedfa7706))
* **dbt:** improve target and routing resolution ([5b03b6c](https://github.com/chio-labs/sqlbuild/commit/5b03b6cc3ee1a0eb22b28481ff7632c3a0ef2d0e))
* **dbt:** only run changed seeds when models build ([2f5d3a4](https://github.com/chio-labs/sqlbuild/commit/2f5d3a4de7e1ec8a36e0bd8af02d54c7dde211c3))
* **dbt:** persist execution progress statuses ([3a521f0](https://github.com/chio-labs/sqlbuild/commit/3a521f042b62c6d285d0694aa9b443cf76dcbd92))
* **dbt:** persist execution progress statuses ([c7cb38e](https://github.com/chio-labs/sqlbuild/commit/c7cb38e31cc9e292a75fe43b2ecd847bb531412c))
* **dbt:** preserve snowflake auth cache ([1ab74d0](https://github.com/chio-labs/sqlbuild/commit/1ab74d07d19a9491c5d52352dcc466a795633de6))
* **dbt:** preserve snowflake auth cache ([e95e18a](https://github.com/chio-labs/sqlbuild/commit/e95e18acfc4aafd488bb71b8b65172f15cbecf29))
* **dbt:** render identity diff as set output ([da80307](https://github.com/chio-labs/sqlbuild/commit/da803079186cf3298a78ebd966b33b35b892f376))
* **dbt:** render identity diff as set output ([227d905](https://github.com/chio-labs/sqlbuild/commit/227d90584868d456696924c7a68b72e82a86d2fe))
* **dbt:** render node starts as running rows ([06a56a0](https://github.com/chio-labs/sqlbuild/commit/06a56a0a5b4f465246a140922bbdaae36c9f80f9))
* **dbt:** render node starts as running rows ([9c52d10](https://github.com/chio-labs/sqlbuild/commit/9c52d108d015402cdb9321504f34650b876811ed))
* **dbt:** report identity diff phases ([c3f56fc](https://github.com/chio-labs/sqlbuild/commit/c3f56fccefec6c3f1a7b1dbba70a3af410c16890))
* **dbt:** report identity diff phases ([1eefdf8](https://github.com/chio-labs/sqlbuild/commit/1eefdf8dc39548915db01b93c33374b4608e9c4c))
* **reuse:** satisfy type and test-convention checks for reuse rework ([7ff2829](https://github.com/chio-labs/sqlbuild/commit/7ff2829f9f0ce6c3e275817223a8435a1bf610f3))
* **virtual:** disable reuse planning in virtual environment plans ([309a17d](https://github.com/chio-labs/sqlbuild/commit/309a17d8b5f6917979dd79a7824926bc1c6d620f))

## [0.38.0](https://github.com/chio-labs/sqlbuild/compare/v0.37.7...v0.38.0) (2026-06-23)


### Features

* **dbt:** add configured force and vars merging ([b8bf236](https://github.com/chio-labs/sqlbuild/commit/b8bf23629c37c1808870b03f45830f0119b72cb1))
* **dbt:** add configured force and vars merging ([fecc042](https://github.com/chio-labs/sqlbuild/commit/fecc04240c3dd9e30fbd718b01add1e61f9f7a78))
* **lineage:** use polyglot for rich column lineage ([e269447](https://github.com/chio-labs/sqlbuild/commit/e269447cfc33d1f195390f4e0632bfa57dbdda7c))
* **lineage:** use polyglot for rich column lineage ([7493ecf](https://github.com/chio-labs/sqlbuild/commit/7493ecf3df3bfd18373c3a9182be801725eb766e))

## [0.37.7](https://github.com/chio-labs/sqlbuild/compare/v0.37.6...v0.37.7) (2026-06-23)


### Bug Fixes

* refresh docs skill and dependency lock ([544171b](https://github.com/chio-labs/sqlbuild/commit/544171b9d01ee298407957da5b2f07709d3893df))
* refresh docs skill and dependency lock ([fb5e52c](https://github.com/chio-labs/sqlbuild/commit/fb5e52c43a887109e2dd85e73917d54b50d0a082))

## [0.37.6](https://github.com/chio-labs/sqlbuild/compare/v0.37.5...v0.37.6) (2026-06-23)


### Bug Fixes

* **dbt:** clarify reuse skip messaging, detect default branch, accept  trailing global flags ([cd3d093](https://github.com/chio-labs/sqlbuild/commit/cd3d093a6b4732d17b01d76b468271f166373139))
* **dbt:** clarify reuse skip messaging, detect default branch, accept trailing global flags ([7d0a3b7](https://github.com/chio-labs/sqlbuild/commit/7d0a3b77a4185bd976628eb7957eb619f7ce6284))

## [0.37.5](https://github.com/chio-labs/sqlbuild/compare/v0.37.4...v0.37.5) (2026-06-23)


### Bug Fixes

* **tests:** align real warehouse targets with current dbt behavior ([e57e857](https://github.com/chio-labs/sqlbuild/commit/e57e8575fcb8248a8f399bf0240b63a6ae5fa1e5))
* **tests:** align real warehouse targets with current dbt behavior ([f9818dc](https://github.com/chio-labs/sqlbuild/commit/f9818dc9d10462f93cafd1c049c5094f749b8dbd))
* **tests:** restore dbt profile prebuild table check ([b5a0812](https://github.com/chio-labs/sqlbuild/commit/b5a08121ca49150279ba00ea89f4cf3a81176b80))
* **tests:** restore dbt profile prebuild table check ([9aad85f](https://github.com/chio-labs/sqlbuild/commit/9aad85f11fc45a6efec8032362c4f5d842908108))
* **tests:** unskip databricks warehouse coverage ([80a37bc](https://github.com/chio-labs/sqlbuild/commit/80a37bc95c91f33dbe1bed10d531b7ab2d1ed914))
* **tests:** unskip databricks warehouse coverage ([8a5b661](https://github.com/chio-labs/sqlbuild/commit/8a5b661a5f1ee0dd8c05cb046546a1b55087fab5))

## [0.37.4](https://github.com/chio-labs/sqlbuild/compare/v0.37.3...v0.37.4) (2026-06-22)


### Bug Fixes

* **dbt:** improve lineage output labels ([d09ee4f](https://github.com/chio-labs/sqlbuild/commit/d09ee4fe938dab41d9f87a3c0a753725d6f921a8))
* **dbt:** normalize supported resource types ([57b67c6](https://github.com/chio-labs/sqlbuild/commit/57b67c6b14e6332af0668f3e81cf33f876061be4))
* **dbt:** stabilize dependency-aware version hashes and required upstream execution ([02df9cb](https://github.com/chio-labs/sqlbuild/commit/02df9cbf39e08c1a5eb5e03245a5a0da11d31a38))

## [0.37.3](https://github.com/chio-labs/sqlbuild/compare/v0.37.2...v0.37.3) (2026-06-22)


### Bug Fixes

* **dbt:** fold independent seed content hash and full config into seed identity ([d11e1a7](https://github.com/chio-labs/sqlbuild/commit/d11e1a7878a2b6ef57d2bb33def14e212b329f7e))
* **dbt:** harden exact selection staleness ([7b3fb12](https://github.com/chio-labs/sqlbuild/commit/7b3fb1272fddf755e54b72321e5a8b1325bee401))
* **dbt:** selection-aware seed change tracking, full-refresh scoping, reuse graceful skip ([fe21783](https://github.com/chio-labs/sqlbuild/commit/fe217837b4ad77704073c122eb34f1c0ba938ee1))
* **planner:** harden selection-aware staleness ([da37882](https://github.com/chio-labs/sqlbuild/commit/da3788281610541f0ce6e111498dd66528b7e36c))
* **planner:** share selection staleness classification ([99c24a5](https://github.com/chio-labs/sqlbuild/commit/99c24a59cb6777f1d2b131d3399c99c2b083fdb9))

## [0.37.2](https://github.com/chio-labs/sqlbuild/compare/v0.37.1...v0.37.2) (2026-06-22)


### Bug Fixes

* **executor:** preflight schemas before concurrent builds ([591401b](https://github.com/chio-labs/sqlbuild/commit/591401b96e40abe7ae838eff458c6ca3046b5123))
* **executor:** preflight schemas before concurrent builds ([58c5cce](https://github.com/chio-labs/sqlbuild/commit/58c5ccefa55f1686bcbd51241bf2d6b6b3f3ac09))

## [0.37.1](https://github.com/chio-labs/sqlbuild/compare/v0.37.0...v0.37.1) (2026-06-21)


### Bug Fixes

* **loaders:** run external loaders before opening warehouse connection ([582b089](https://github.com/chio-labs/sqlbuild/commit/582b08964843636569c1058d72cd49e1f6345496))
* **loaders:** run external loaders before opening warehouse connection ([32710f7](https://github.com/chio-labs/sqlbuild/commit/32710f794f512fef4634696be19c087cf118a7fa))

## [0.37.0](https://github.com/chio-labs/sqlbuild/compare/v0.36.0...v0.37.0) (2026-06-21)


### Features

* **dlt:** cover real warehouse destinations ([0a595b6](https://github.com/chio-labs/sqlbuild/commit/0a595b62c436cbe6b6a6440c7b475b5b3f9a5f4c))
* **dlt:** expose destination settings ([1fa1cd8](https://github.com/chio-labs/sqlbuild/commit/1fa1cd8f272e9c31c1730e3f5a0668b079dba962))
* **dlt:** report live source progress ([de4609b](https://github.com/chio-labs/sqlbuild/commit/de4609bd2d9dcc7d0061fcaedadcdc257ab8e303))
* **integrations:** add declarative dlt sources ([c25d721](https://github.com/chio-labs/sqlbuild/commit/c25d721a57cc5aa83724495cc796d9679a5f9dc9))

## [0.36.0](https://github.com/chio-labs/sqlbuild/compare/v0.35.0...v0.36.0) (2026-06-21)


### Features

* add dbt column lineage ([457af59](https://github.com/chio-labs/sqlbuild/commit/457af59dac5dd2c596ce742eceaa2d31aa1d31cd))
* add dbt mixed lineage ([0a438a4](https://github.com/chio-labs/sqlbuild/commit/0a438a4d5b44b75c5174d66f4c6fad6ac74d85f8))
* add dependency baseline reuse ([6fa023d](https://github.com/chio-labs/sqlbuild/commit/6fa023d600b3e9dfdf69e4b3e7dac35cbc3a08ce))
* **dbt:** add dbt clone command ([dad43fa](https://github.com/chio-labs/sqlbuild/commit/dad43fa41e4a64b0e9968a2ab845fa4310b01ffa))
* **dbt:** add dbt clone command ([9aa8bcc](https://github.com/chio-labs/sqlbuild/commit/9aa8bcc8ad13cc648e81bb993bdc629407947532))
* **dbt:** add dbt reuse playground and fix reuse defects ([190260e](https://github.com/chio-labs/sqlbuild/commit/190260e76efd0f2de73fc7bc25cd4e5da88aa5b1))
* **dbt:** add sqb dbt diff against production reuse ref ([ae9d0db](https://github.com/chio-labs/sqlbuild/commit/ae9d0dbe93b4d1cc78e93c8d3d37d3d35608caa4))
* **dbt:** add sqb dbt scenario capture and test --local replay ([3f0fde1](https://github.com/chio-labs/sqlbuild/commit/3f0fde100114a0af03c49373173b236fef386dee))
* **dbt:** add sqb dbt scenario test for warehouse-direct dbt scenarios ([cfa761e](https://github.com/chio-labs/sqlbuild/commit/cfa761eadb4843c30c002a1352559d21397679cc))
* **dbt:** declare sqb dbt execution flags with -- passthrough ([5a18910](https://github.com/chio-labs/sqlbuild/commit/5a18910dc0b314993cd6523f5f2b06772058154e))
* **dbt:** enforce snapshot and ephemeral chain boundaries in dbt tests ([68e4a3f](https://github.com/chio-labs/sqlbuild/commit/68e4a3fd8e29c799854973d17cb904226d0a4a09))
* **dbt:** make dbt executable configurable via DBT_EXECUTABLE ([70e98f9](https://github.com/chio-labs/sqlbuild/commit/70e98f939da1402cc6e0518a6da6604e4374dda2))
* **dbt:** polish dbt init/auto-init reuse setup and harden plan UX ([4fe1b41](https://github.com/chio-labs/sqlbuild/commit/4fe1b41f8720a54c51827565a08ca24adc47ee6c))
* **dbt:** rich column lineage via SQLGlot, fix CTE and seed lineage ([e3dc014](https://github.com/chio-labs/sqlbuild/commit/e3dc0147e4f47de4ffa9d885a563bdd7f7130354))
* **dbt:** support chained dbt model unit tests with cleaner test output ([346402c](https://github.com/chio-labs/sqlbuild/commit/346402c9cb89839117180537b9eecb882d74977d))
* **dbt:** support SQLBuild source and seed mocks ([8267b41](https://github.com/chio-labs/sqlbuild/commit/8267b41cd63ff96ca7793349a61d8af7540efff3))
* **dbt:** support SQLBuild tests targeting dbt models ([d61f404](https://github.com/chio-labs/sqlbuild/commit/d61f404fbe4a83daba20c9ec780ef7daf9bf2499))
* **dbt:** unify __dbt_ref spelling and extend dbt scenario tests ([3da04ab](https://github.com/chio-labs/sqlbuild/commit/3da04ab1f54418d73cb2b74a01339caffe3403aa))


### Bug Fixes

* **dbt:** capture dbt_ref scenario fixtures for local replay ([15e0aa3](https://github.com/chio-labs/sqlbuild/commit/15e0aa3726ed9a3c55937519425f61c492e81591))
* **dbt:** cascade upstream changes into reuse eligibility ([a0d4056](https://github.com/chio-labs/sqlbuild/commit/a0d4056016c68c9d39cb62e6561f3b44c4db2e70))
* **dbt:** correct reuse change-detection, block guard, lineage scope, test pruning ([ddce3c4](https://github.com/chio-labs/sqlbuild/commit/ddce3c4eba5da59362214695b35c5fe8c9e603ca))
* **dbt:** make dbt interop tests and reuse compatible with the Fusion engine ([e5a6bb7](https://github.com/chio-labs/sqlbuild/commit/e5a6bb7fbfb814a9036880cc9e2df916a274b341))
* **dbt:** show only selected models in dbt plan, not passive upstreams ([4289c2b](https://github.com/chio-labs/sqlbuild/commit/4289c2b48bf587eba99b66ad3e54e5e49f4692cb))
* **dbt:** stabilize runner tests with configured executable ([555af17](https://github.com/chio-labs/sqlbuild/commit/555af1794a522e47b9444a4fdefb7f9330201e03))
* run all verify phases ([9b14147](https://github.com/chio-labs/sqlbuild/commit/9b141470386de276d7063d13f577f2cec41d7874))


### Documentation

* refresh README positioning ([49de44a](https://github.com/chio-labs/sqlbuild/commit/49de44ad531d2969b01c7eb340d55928c29d41e7))

## [0.35.0](https://github.com/chio-labs/sqlbuild/compare/v0.34.0...v0.35.0) (2026-06-17)


### Features

* add dbt outcome overlay ([adf4e65](https://github.com/chio-labs/sqlbuild/commit/adf4e65053bb2b8c3a44241d62cee6556671c026))
* add dbt profile init support ([240e86e](https://github.com/chio-labs/sqlbuild/commit/240e86ed14c5d7551bb0b137885f64b902184611))
* add dbt reuse_from compile foundation ([063ceb8](https://github.com/chio-labs/sqlbuild/commit/063ceb8fe4c2c8508caac50d9b3f5e9682d6b6bd))
* add dbt seeded reuse runtime ([bea0981](https://github.com/chio-labs/sqlbuild/commit/bea09816d3d7c7b672ba330d52ae821d3e9904d2))
* add source freshness age policy ([4bcb816](https://github.com/chio-labs/sqlbuild/commit/4bcb816431b2ddb126bc6e6ed8f4e9ea887abb6f))
* auto-init dbt interop projects ([9ba14b5](https://github.com/chio-labs/sqlbuild/commit/9ba14b59dd69c4e79dd1fca83c2cc944f8987752))
* batch table freshness metadata ([78c4802](https://github.com/chio-labs/sqlbuild/commit/78c4802b69e9c5a0c857ce1004f1301940dafd78))
* block stale source downstreams ([bb45f71](https://github.com/chio-labs/sqlbuild/commit/bb45f718fa7b957f6459029aa90da9812940548d))
* execute dbt complete reuse ([f573ba2](https://github.com/chio-labs/sqlbuild/commit/f573ba2cfb100ecb162a923f20c41a5cc2b4ca35))
* expand dbt profile adapters ([cfbc1aa](https://github.com/chio-labs/sqlbuild/commit/cfbc1aa3546319053ea679b8a88a66bdc5329d25))
* expand dbt reuse coverage ([1138005](https://github.com/chio-labs/sqlbuild/commit/113800589ac513d01fdea31ed4749d7d90f14778))
* persist dbt source freshness state ([5a41763](https://github.com/chio-labs/sqlbuild/commit/5a4176318d12bed6e7ff5429ef36f3c91ab5ae94))
* plan dbt reuse candidates ([a2295d1](https://github.com/chio-labs/sqlbuild/commit/a2295d10d4e10f8b25ac25f19cdd132d71fe6d22))
* prune current dbt models ([640b218](https://github.com/chio-labs/sqlbuild/commit/640b218820bc13a4ca8382707a24a9dacc323628))
* resume dbt complete reuse ([69436c6](https://github.com/chio-labs/sqlbuild/commit/69436c653d0a5ea2e81d0a62fc4e67b1b922a9fb))
* show dbt sql diffs ([b037d44](https://github.com/chio-labs/sqlbuild/commit/b037d44a59b51314544d356b37c9b5f0bac30df1))
* track dbt node state ([64c568a](https://github.com/chio-labs/sqlbuild/commit/64c568ab718e55f91f3b9aa5042b0c4669ded4b9))
* translate dbt source freshness ([2868bf2](https://github.com/chio-labs/sqlbuild/commit/2868bf25bad0730b34be85161c98695049029ef7))


### Bug Fixes

* align hook target context naming ([b55e412](https://github.com/chio-labs/sqlbuild/commit/b55e412f8b8227d087b173cc2f253aeacc8bb3b5))
* ban internal re-export shims ([4cd0ddb](https://github.com/chio-labs/sqlbuild/commit/4cd0ddbde1e9fa2e9ff190c8c475247e64c84a60))
* clarify dbt reuse output ([98fe959](https://github.com/chio-labs/sqlbuild/commit/98fe9592da64cc52f693eb08739c4dd985d696b9))
* polish dbt interop output ([1c9f1e2](https://github.com/chio-labs/sqlbuild/commit/1c9f1e29039f6726e08dd592d85dceb9784b8bbb))
* space dbt-only no-work output ([99adedd](https://github.com/chio-labs/sqlbuild/commit/99adedd54b89ba97394196dcf28e91e681c8148b))
* use exact dbt fqn selectors ([012c3e8](https://github.com/chio-labs/sqlbuild/commit/012c3e8a85a7c32711270a1b3c5b19aaa3f219db))

## [0.34.0](https://github.com/chio-labs/sqlbuild/compare/v0.33.0...v0.34.0) (2026-06-13)


### Features

* expose loader node result context ([337f799](https://github.com/chio-labs/sqlbuild/commit/337f799a5362a1d4622eb34b3e58cc031423feab))
* expose loader node result context ([72e4b25](https://github.com/chio-labs/sqlbuild/commit/72e4b25bd72c50940a0cbc0721402b2b377b67aa))
* split SQL function resource types ([6739f49](https://github.com/chio-labs/sqlbuild/commit/6739f493beb580d3406917a3f8027c8262c3e1ea))
* split SQL function resource types ([41c6572](https://github.com/chio-labs/sqlbuild/commit/41c657267224616e4cee4aa2bb1caa01dd9bde78))
* support lifecycle hook skips ([19b7bee](https://github.com/chio-labs/sqlbuild/commit/19b7bee281184809d89b00309462351080d96d91))


### Bug Fixes

* atomically update VDE current refs ([362345a](https://github.com/chio-labs/sqlbuild/commit/362345a9059d36d0974fc3049c6ea6ccf2060520))
* atomically update VDE current refs ([2648201](https://github.com/chio-labs/sqlbuild/commit/26482018b1908925eb6c5be86a07c6e243bb60f8))
* avoid DuckDB duplicate VDE ref inserts ([a7b07a5](https://github.com/chio-labs/sqlbuild/commit/a7b07a5c321f42b22a868d7da84d5e6751136d64))
* avoid DuckDB duplicate VDE ref inserts ([c28481a](https://github.com/chio-labs/sqlbuild/commit/c28481aff197e10b4d3b65a890063bd0982ce855))
* validate Python hook returns ([aff8d57](https://github.com/chio-labs/sqlbuild/commit/aff8d5747af5e233f92a53e55b3c5e4dc3fb9354))

## [0.33.0](https://github.com/chio-labs/sqlbuild/compare/v0.32.0...v0.33.0) (2026-06-13)


### Features

* persist direct node runtime results ([7b1763e](https://github.com/chio-labs/sqlbuild/commit/7b1763e8fe3753a83042b61006081b2089629c10))
* persist virtual node runtime results ([2d35a6e](https://github.com/chio-labs/sqlbuild/commit/2d35a6eabc781d9358e5713ff251e4397dc92137))


### Bug Fixes

* block VDE models after skipped loaders ([ba26393](https://github.com/chio-labs/sqlbuild/commit/ba2639393d85529a71ebd32e3da0cf724f54e8ad))
* make check selectors check-only ([6e84e60](https://github.com/chio-labs/sqlbuild/commit/6e84e60b81b57d9f10b6517fe2c07c9ebb23a533))
* serialize node result DDL ([3a2187a](https://github.com/chio-labs/sqlbuild/commit/3a2187af9e16f52f56565e5fd20decac45ca2a50))
* validate node results across adapters ([068811f](https://github.com/chio-labs/sqlbuild/commit/068811fecf6869da4fa0c2324c135aca9904b652))

## [0.32.0](https://github.com/chio-labs/sqlbuild/compare/v0.31.0...v0.32.0) (2026-06-12)


### Features

* replace run with build flags ([4f65879](https://github.com/chio-labs/sqlbuild/commit/4f65879e1cf0b8499ded9295b97e184216b3a16f))

## [0.31.0](https://github.com/chio-labs/sqlbuild/compare/v0.30.1...v0.31.0) (2026-06-12)


### Features

* add audit always-run reuse guard ([9197f89](https://github.com/chio-labs/sqlbuild/commit/9197f8988136abd7d13d5e937b81d2a9b1887fd7))
* add audit gate identity hashing ([5729672](https://github.com/chio-labs/sqlbuild/commit/5729672c007ea94252733b7c13864a4d6d92b30d))
* add audit gate reuse proof decisions ([c5c38e6](https://github.com/chio-labs/sqlbuild/commit/c5c38e6032e8a2dfaa9e787a6094770ebb379690))
* add direct plan identity diagnostics ([c58a833](https://github.com/chio-labs/sqlbuild/commit/c58a833f2a3c1111c0cfceabb45686c4dc003015))
* add direct replay-on-change planning ([5a6d8d8](https://github.com/chio-labs/sqlbuild/commit/5a6d8d89b8955f45b8957ac8ad73afd26d9874f2))
* add direct reuse planning decisions ([d3c3202](https://github.com/chio-labs/sqlbuild/commit/d3c32027f6540a9d7c4ea29d05427fb5b0bea4af))
* add direct target reuse config ([83c0deb](https://github.com/chio-labs/sqlbuild/commit/83c0deb6197fe4d6c34768dee8f827367aadcda2))
* add run despite unchanged planning ([5dc3973](https://github.com/chio-labs/sqlbuild/commit/5dc3973e613efb4c88bfe1f12a681091464e4d68))
* add standard relation reuse execution ([7805ffa](https://github.com/chio-labs/sqlbuild/commit/7805ffa459ab39a4cd4c13058ce47a89ee4b2bc9))
* add standard reuse origin plumbing ([6b52ea6](https://github.com/chio-labs/sqlbuild/commit/6b52ea67cdb7c8b79352b29da93c5174b481d12b))
* add virtual seed state refs ([a996052](https://github.com/chio-labs/sqlbuild/commit/a996052e3429da6ff912de0f128e07cdcc0f3362))
* complete virtual seed lifecycle ([d266670](https://github.com/chio-labs/sqlbuild/commit/d2666709dea604baf49f52726ddec38e80794e5c))
* compute python node identities ([2315096](https://github.com/chio-labs/sqlbuild/commit/231509668d3f8f568c89a32cb65106d2d0a4af00))
* enforce hard-copy relation reuse ([6f864dd](https://github.com/chio-labs/sqlbuild/commit/6f864dd592f2e17fc97f2868874e4bc04de9fd73))
* gate expanded polyglot compile analysis ([e806367](https://github.com/chio-labs/sqlbuild/commit/e806367541149d6198f273aef630ce9f350f6803))
* generate structure skill ([c059bcd](https://github.com/chio-labs/sqlbuild/commit/c059bcda7b0c804f873928c57627d15d0bf79a3b))
* harden standard relation reuse coverage ([819f592](https://github.com/chio-labs/sqlbuild/commit/819f5921b127d984941d85a210e86482848ce5a7))
* include seeds in virtual version identity ([d7e555c](https://github.com/chio-labs/sqlbuild/commit/d7e555c6a996448f9a8d7d8de8a1c77003b0b401))
* make change pruning the default ([8daaf63](https://github.com/chio-labs/sqlbuild/commit/8daaf636714de7d311bb848eeb1a246f77d7d7ac))
* make change pruning the default ([425463c](https://github.com/chio-labs/sqlbuild/commit/425463c08bc20e94512bfe5e9d8326077766eddf))
* persist audit gate fingerprint metadata ([7b32376](https://github.com/chio-labs/sqlbuild/commit/7b32376f55add2d7d26d94789e22dd99e908d9e2))
* persist python node identities ([27e47a7](https://github.com/chio-labs/sqlbuild/commit/27e47a7a95b7f1b301d9d48cde1878fa4622caa8))
* physicalize virtual seed artifacts ([d296a18](https://github.com/chio-labs/sqlbuild/commit/d296a18a5fa950d0b4e7405c12f343fcb5e738a2))
* prune direct state history in janitor ([1fb16ad](https://github.com/chio-labs/sqlbuild/commit/1fb16ad5fad8ef51e16956b890b6c2b9114f68b8))
* remove sqlglot runtime dependency ([82d432e](https://github.com/chio-labs/sqlbuild/commit/82d432e97e0632337e563151ca89cf77d60d9bbb))
* reuse origin audit proof for table reuse ([2e611e1](https://github.com/chio-labs/sqlbuild/commit/2e611e14141f01bc0c449853dd30248c0bae4247))
* show compile phase progress ([12c642a](https://github.com/chio-labs/sqlbuild/commit/12c642a59dab3c065634b2e2e8de7f3ba017a4d6))
* snapshot direct reuse source state ([bbf2c58](https://github.com/chio-labs/sqlbuild/commit/bbf2c5833b503171528e8447834026a82da96171))
* store virtual python identities in state ([5c4edeb](https://github.com/chio-labs/sqlbuild/commit/5c4edebb0f75abb43b57e8023b3dd6cae2078c89))
* store virtual python identities in state ([b47a42e](https://github.com/chio-labs/sqlbuild/commit/b47a42e0ef8a475a6a1b1773fd27241ec2a239a7))
* support custom baseline relation reuse ([76282ed](https://github.com/chio-labs/sqlbuild/commit/76282ed4cbf3d4e6dcf1a13fd0a49f9600b9cff5))
* support run despite unchanged in virtual planning ([d6480f0](https://github.com/chio-labs/sqlbuild/commit/d6480f00dca67c2efa882aef72fce9ab8a02e8df))
* support snapshot relation reuse seeding ([efa8c5e](https://github.com/chio-labs/sqlbuild/commit/efa8c5efba2cb64ca0572de9ba97fe83d55d49dd))
* support virtual seed state planning ([08146a5](https://github.com/chio-labs/sqlbuild/commit/08146a5c523d53a0dca5cd5c966db002475b376b))
* track seed changes in standard planning ([538faf8](https://github.com/chio-labs/sqlbuild/commit/538faf8e0a54a13e2470250a7413e523170cf045))
* use compact polyglot query analysis ([7c42dfa](https://github.com/chio-labs/sqlbuild/commit/7c42dfac83a70a90fabf82497dfb68bae7a9ee3e))


### Bug Fixes

* align virtual seed artifact selection ([01e82b2](https://github.com/chio-labs/sqlbuild/commit/01e82b21f069ba4897d230fccaacacfa625cadea))
* harden metadata state handling ([71acc4b](https://github.com/chio-labs/sqlbuild/commit/71acc4b3adab3200179513192c9f3d2d27317970))
* keep required functions in pruned plans ([f082fa7](https://github.com/chio-labs/sqlbuild/commit/f082fa7b5f47213f79135af631e6924e4175b9dd))
* normalize databricks string casts ([261b9a2](https://github.com/chio-labs/sqlbuild/commit/261b9a2d68cccab43d9ee939988036767d5e82e0))
* render scenario relation names through the adapter ([aeb7956](https://github.com/chio-labs/sqlbuild/commit/aeb79565973e5348d88c6fa6e4ba82a13d9c4d0f))
* repair sqlserver virtual test config ([abb6557](https://github.com/chio-labs/sqlbuild/commit/abb65570dcc77d397a37e4d1879855db35a9e6cc))
* replace exception-swallowing existence probes with adapter metadata checks ([c55248d](https://github.com/chio-labs/sqlbuild/commit/c55248d50d4d4a0f360ea8a5c55352e48201093d))
* restore bigquery regression coverage ([05ecaee](https://github.com/chio-labs/sqlbuild/commit/05ecaee8c36ccf6f3fe0eda28565ebf4ed45154a))
* update reuse audit output expectations ([efa5770](https://github.com/chio-labs/sqlbuild/commit/efa5770bdf474e255371424489d55727a7b95a87))


### Performance Improvements

* expand compact sql analysis usage ([a2b30df](https://github.com/chio-labs/sqlbuild/commit/a2b30df23afd62747d6bbf467cf343e66d41a84e))
* read latest direct state in SQL ([616284b](https://github.com/chio-labs/sqlbuild/commit/616284b892fc24d08bc69c88bfbc96fa605ef3cf))


### Documentation

* refresh project readme ([bd92c31](https://github.com/chio-labs/sqlbuild/commit/bd92c3185fe8d705e7089c3e090289cdfe75b2ee))

## [0.30.1](https://github.com/chio-labs/sqlbuild/compare/v0.30.0...v0.30.1) (2026-06-08)


### Bug Fixes

* avoid recursive compile layer counting ([312090f](https://github.com/chio-labs/sqlbuild/commit/312090f894f3284ab2a962f0d8ef3760fe74466d))
* restore compile analysis parity ([f3bdf6e](https://github.com/chio-labs/sqlbuild/commit/f3bdf6ed9495bdc485e97b619232274b609bb25b))


### Performance Improvements

* fuse compact sql analysis ([07982a5](https://github.com/chio-labs/sqlbuild/commit/07982a588e10a76f16a13093e2b4c550894a8dbc))
* parallelize compile sql analysis ([703550f](https://github.com/chio-labs/sqlbuild/commit/703550fe0db48373c0135a840bbb5674dd4c72c3))
* reduce compile discovery overhead ([64375cf](https://github.com/chio-labs/sqlbuild/commit/64375cf8996e0ec52afa197884cfc228a1612442))
* reduce compile scanner overhead ([d4f2d75](https://github.com/chio-labs/sqlbuild/commit/d4f2d750a3a6daaf941dd0bab813e5cc9b187ea2))
* speed up compile analysis ([507b61d](https://github.com/chio-labs/sqlbuild/commit/507b61da545fb30a245d3e2143e23a0a8977f640))
* speed up compile analysis ([e33eedc](https://github.com/chio-labs/sqlbuild/commit/e33eedca6cbb90c6402920168e7f8e4401067a30))

## [0.30.0](https://github.com/chio-labs/sqlbuild/compare/v0.29.0...v0.30.0) (2026-06-07)


### Features

* add provider base API ([595ca69](https://github.com/chio-labs/sqlbuild/commit/595ca694b7f34487986837de60ba624a6322e882))
* add provider runtime sessions ([19518f9](https://github.com/chio-labs/sqlbuild/commit/19518f9f893586cdba1c8f17890391d94a5f6e88))
* discover project providers ([d2b921d](https://github.com/chio-labs/sqlbuild/commit/d2b921d99928d5f1c1f81d05dad8fd7e4bfd1421))
* expose providers on python contexts ([cc86c65](https://github.com/chio-labs/sqlbuild/commit/cc86c65db3569482b886303245e01ce1636dab4f))
* inject providers into python nodes ([a085da5](https://github.com/chio-labs/sqlbuild/commit/a085da51899b4d36bea34b957f6001fac7ebd340))
* show provider usage in plans ([6c06f16](https://github.com/chio-labs/sqlbuild/commit/6c06f16e61a44437335205a40a6dfc9a2ec4277c))
* support providers in python hooks ([ce276da](https://github.com/chio-labs/sqlbuild/commit/ce276daaa9fd1597a4f7315905d595ddae5dd9fb))
* wire providers into command execution ([86f5d05](https://github.com/chio-labs/sqlbuild/commit/86f5d059529424b7ed238b03fb9d9c17aa7f65a6))


### Bug Fixes

* isolate provider discovery imports ([96b6101](https://github.com/chio-labs/sqlbuild/commit/96b61016a3e7e69a66ed7eaaa3227622b70f588c))
* polish provider diagnostics ([4a87c81](https://github.com/chio-labs/sqlbuild/commit/4a87c810aace7719449a88098d91c699a8be00ee))
* preserve provider suffix in default names ([e095507](https://github.com/chio-labs/sqlbuild/commit/e0955070c8c61d357659833ae534173bf8d9972d))

## [0.29.0](https://github.com/chio-labs/sqlbuild/compare/v0.28.1...v0.29.0) (2026-06-06)


### Features

* add typed model lifecycle hooks ([0f92dd2](https://github.com/chio-labs/sqlbuild/commit/0f92dd233c65ef71b28d96b9122c334efe80bba4))
* discover python lifecycle hooks ([692c318](https://github.com/chio-labs/sqlbuild/commit/692c318d9d2530c07dc5b032061d3243b1c11dfe))
* invoke python lifecycle hooks ([9506f9d](https://github.com/chio-labs/sqlbuild/commit/9506f9ddad5b9cd8ea4752608d082e78d2cabb1f))
* scaffold hooks directory on init ([5a773b4](https://github.com/chio-labs/sqlbuild/commit/5a773b4aa86094f6a704323726022cb3caac3013))
* show lifecycle hooks in build output ([75db00b](https://github.com/chio-labs/sqlbuild/commit/75db00bbf574caf37f25571bd3bb4be735095d0b))
* validate python lifecycle hooks ([1b63282](https://github.com/chio-labs/sqlbuild/commit/1b63282a4ddc21c75680bac1e1983dc68f18fca4))

## [0.28.1](https://github.com/chio-labs/sqlbuild/compare/v0.28.0...v0.28.1) (2026-06-06)


### Documentation

* clarify playground quick start ([75fa2c0](https://github.com/chio-labs/sqlbuild/commit/75fa2c0449fd96a241d29689c6d6d0b0b3cdb603))
* clarify playground quick start ([e0da66e](https://github.com/chio-labs/sqlbuild/commit/e0da66ee5203ea94585d1d81a2bb43c78702429c))

## [0.28.0](https://github.com/chio-labs/sqlbuild/compare/v0.27.0...v0.28.0) (2026-06-06)


### Features

* add source freshness command ([a2d543d](https://github.com/chio-labs/sqlbuild/commit/a2d543d4efeab7df472c7d02385c7a75a6fa116f))
* add source freshness state comparison ([77c70d9](https://github.com/chio-labs/sqlbuild/commit/77c70d90310ca399f346a8390089a7b0cf540b55))


### Bug Fixes

* place provider default target at top level ([a861c03](https://github.com/chio-labs/sqlbuild/commit/a861c03cf263db723f16d1ae1f2447805d23a588))

## [0.27.0](https://github.com/chio-labs/sqlbuild/compare/v0.26.2...v0.27.0) (2026-06-03)


### Features

* add bigquery and databricks freshness metadata ([875796d](https://github.com/chio-labs/sqlbuild/commit/875796d1990dda1ec590854a6367d0af4660b85b))
* add source freshness metadata foundation ([60d9d98](https://github.com/chio-labs/sqlbuild/commit/60d9d98a5ce90024014b7fbf13d06e3bd9fddb41))
* enforce virtual source freshness skips ([278e753](https://github.com/chio-labs/sqlbuild/commit/278e75351fe3f262b52e66448016f4846ca622ed))
* harden virtual execution signatures ([75c4dc3](https://github.com/chio-labs/sqlbuild/commit/75c4dc354aa84520009c1f3a7e67db9b0f864d80))
* include source freshness in virtual hashes ([952a18e](https://github.com/chio-labs/sqlbuild/commit/952a18e9bd3cd8c153232a707e811d59f2e7411f))
* persist virtual source freshness observations ([9bd6380](https://github.com/chio-labs/sqlbuild/commit/9bd6380f7cb0570da9e2d8045d90889d9a646252))


### Bug Fixes

* run read-side check dependencies ([6e4e8f8](https://github.com/chio-labs/sqlbuild/commit/6e4e8f8a2ec92c284ed77709325550420397d219))

## [0.26.2](https://github.com/chio-labs/sqlbuild/compare/v0.26.1...v0.26.2) (2026-06-03)


### Bug Fixes

* harden soft and hard skip semantics ([f7459d3](https://github.com/chio-labs/sqlbuild/commit/f7459d3294d164eaf2044dec6eb6e52bd5a9e4c2))
* harden soft and hard skip semantics ([22db952](https://github.com/chio-labs/sqlbuild/commit/22db952587b41f1f7e2e4933cf02482088458d4f))

## [0.26.1](https://github.com/chio-labs/sqlbuild/compare/v0.26.0...v0.26.1) (2026-06-02)


### Bug Fixes

* enforce python node factory folders ([ec2b7ac](https://github.com/chio-labs/sqlbuild/commit/ec2b7acf3e0eea237f209b394f9db1c4994fc602))
* enforce python node factory folders ([27117d6](https://github.com/chio-labs/sqlbuild/commit/27117d69e83dd5e63e73f796d5f02bf462796fcf))

## [0.26.0](https://github.com/chio-labs/sqlbuild/compare/v0.25.1...v0.26.0) (2026-06-02)


### Features

* add python node factories ([ba64a5d](https://github.com/chio-labs/sqlbuild/commit/ba64a5dcc221bcf6d106b4dd69bd97d352696476))
* add python node factories ([7386f73](https://github.com/chio-labs/sqlbuild/commit/7386f7392d98317b1f464044541218409bb0fa82))
* support explicit loader names ([9af93ea](https://github.com/chio-labs/sqlbuild/commit/9af93ea815c5a8a1f296a290ccc6d2be6332756b))


### Bug Fixes

* clarify target rename cleanup ([8c3cd72](https://github.com/chio-labs/sqlbuild/commit/8c3cd720154934fc46805d75d2d75bc84d01c4f0))
* clean up remaining rename fixtures ([b9c42c4](https://github.com/chio-labs/sqlbuild/commit/b9c42c4545f88946d1b7332037226c0ea60e71d5))
* clean up target rename fallout ([f3ed2be](https://github.com/chio-labs/sqlbuild/commit/f3ed2be5e1a226c6a2d6bf4acbe7c88ad909ef3e))

## [0.25.1](https://github.com/chio-labs/sqlbuild/compare/v0.25.0...v0.25.1) (2026-06-01)


### Bug Fixes

* expose skip mode in public python APIs ([2019224](https://github.com/chio-labs/sqlbuild/commit/201922489477c1acfc242e13f89459d3f7d91601))
* expose skip mode in public python APIs ([403b4e4](https://github.com/chio-labs/sqlbuild/commit/403b4e46d8e2377ee94f20586b10cc110b193f06))

## [0.25.0](https://github.com/chio-labs/sqlbuild/compare/v0.24.0...v0.25.0) (2026-06-01)


### Features

* add cli document helper ([bbbb2cb](https://github.com/chio-labs/sqlbuild/commit/bbbb2cbf35e407b6899052723f7cdaac5c331918))
* add python check result context ([8542fcc](https://github.com/chio-labs/sqlbuild/commit/8542fcc270063f4dc9eb0d591f2438ffbc6bd30c))
* add python node contexts ([00536e7](https://github.com/chio-labs/sqlbuild/commit/00536e73330bb7abbcf85035062164fc2e640841))
* add python node executor ([4bed0fb](https://github.com/chio-labs/sqlbuild/commit/4bed0fba89e9b3c9aade7483aa3818f8b99eaf3a))
* add python node foundation ([6f0a20f](https://github.com/chio-labs/sqlbuild/commit/6f0a20f67bef9f33dd735b133daba660f51ef5c7))
* add python node graph inventory ([e8ffa75](https://github.com/chio-labs/sqlbuild/commit/e8ffa75aa457220d092f91049879cae005e5d854))
* add python node result policies ([dec97fe](https://github.com/chio-labs/sqlbuild/commit/dec97fed017095d20f4f043393cff5f6d8405d5d))
* add python node retry policy ([ba2303a](https://github.com/chio-labs/sqlbuild/commit/ba2303a3bfe29e7b0c1e54516848dee1c4f80aa5))
* add python node selector resolver ([3bd4339](https://github.com/chio-labs/sqlbuild/commit/3bd433961a9e4141ae971465860d3c531002b841))
* add python nodes playground ([f6114b1](https://github.com/chio-labs/sqlbuild/commit/f6114b19bde901aa87e8a735619649772eea95d3))
* add semantic cli style helper ([9f3e049](https://github.com/chio-labs/sqlbuild/commit/9f3e0495aa044777ef46f22900bf79634beba305))
* add task and asset decorators ([c6dcfd5](https://github.com/chio-labs/sqlbuild/commit/c6dcfd5fa72473b15c1c3605125f6f037e441198))
* add unified python sql selector validation ([53ee19c](https://github.com/chio-labs/sqlbuild/commit/53ee19c2439e40c1a794be2da7431c463447d921))
* discover check functions ([040aa55](https://github.com/chio-labs/sqlbuild/commit/040aa555710c662aa5173d776e6c20be041cb8f3))
* discover task and asset functions ([bfde31f](https://github.com/chio-labs/sqlbuild/commit/bfde31fd6bb9408a82575859c25db11ce961207c))
* enforce selectable resource uniqueness ([3332c63](https://github.com/chio-labs/sqlbuild/commit/3332c63deb38e17809f65c9cd660834d06e28e77))
* execute python checks ([806c7f5](https://github.com/chio-labs/sqlbuild/commit/806c7f588246f2d34b78d42e775ce4284c6a8b5c))
* harden python build lifecycle ([d39df74](https://github.com/chio-labs/sqlbuild/commit/d39df7469fe29945b7059431f7bc330e277bcff6))
* include python nodes in dag artifact ([18a9992](https://github.com/chio-labs/sqlbuild/commit/18a999253ba4ddf883387c95e2b073eb3bd9b8f6))
* integrate python nodes with run lifecycle ([7f7bc17](https://github.com/chio-labs/sqlbuild/commit/7f7bc17d820739fa3acf3ddc05da65fb05dc4d97))
* integrate python nodes with virtual builds ([150b2c5](https://github.com/chio-labs/sqlbuild/commit/150b2c5f52697124c388528205c70bbb822e0473))
* map python nodes in integrations ([3e5b324](https://github.com/chio-labs/sqlbuild/commit/3e5b3247763cf0389fdfb544fcf6fd9e224f0645))
* require explicit path selector roots ([d4118ab](https://github.com/chio-labs/sqlbuild/commit/d4118aba4afac1d07eb23354df1ae2f742d567e4))
* show python nodes in plan output ([abc278d](https://github.com/chio-labs/sqlbuild/commit/abc278d90d91d1b344408952f3ba922d4ac8df8f))


### Bug Fixes

* avoid stale diagnostics stderr handlers ([aa13c96](https://github.com/chio-labs/sqlbuild/commit/aa13c96d2658b2c2aac733d76aa56970ca889972))
* preserve managed source auto load expansion ([a3a8ba7](https://github.com/chio-labs/sqlbuild/commit/a3a8ba76d71546822bd06b615fffb0e049162285))
* show compile progress during builds ([e9838e8](https://github.com/chio-labs/sqlbuild/commit/e9838e8a38c8f3796d6d299f6f43a63b2654e0f6))
* stabilize python lifecycle selection ([fefca3f](https://github.com/chio-labs/sqlbuild/commit/fefca3f459ab7db87638e8e52b685c3b721d1c9e))

## [0.24.0](https://github.com/chio-labs/sqlbuild/compare/v0.23.0...v0.24.0) (2026-05-28)


### Features

* add rivers integration ([82de30c](https://github.com/chio-labs/sqlbuild/commit/82de30ca29f4cde1deba35f08d6575fa82657c62))
* add rivers integration ([9268beb](https://github.com/chio-labs/sqlbuild/commit/9268beb1cae3bd78f51f9e17241767c892d51722))

## [0.23.0](https://github.com/chio-labs/sqlbuild/compare/v0.22.1...v0.23.0) (2026-05-28)


### Features

* support virtual custom materializations ([c9a1447](https://github.com/chio-labs/sqlbuild/commit/c9a144788ad77b301c830856d917a177234f359d))
* support virtual custom materializations ([21bba32](https://github.com/chio-labs/sqlbuild/commit/21bba32086ca9ad5c175b7525679de76fced9752))


### Bug Fixes

* include virtual table functions ([68d2781](https://github.com/chio-labs/sqlbuild/commit/68d278112d008f1ece4eae744ca507dd52ad8347))
* include virtual table functions ([b667184](https://github.com/chio-labs/sqlbuild/commit/b6671848990eb8a824faf2fe636197be26a08bd4))
* seed virtual versions lazily ([0b2d683](https://github.com/chio-labs/sqlbuild/commit/0b2d683105a7777f94a4fd20675601d98811e632))
* seed virtual versions lazily ([400c161](https://github.com/chio-labs/sqlbuild/commit/400c16109bd1bc1a805c9859083c628e10b160b1))

## [0.22.1](https://github.com/chio-labs/sqlbuild/compare/v0.22.0...v0.22.1) (2026-05-28)


### Bug Fixes

* gate virtual operations on state status ([a89f806](https://github.com/chio-labs/sqlbuild/commit/a89f8063a04c54174e67dcd0f812dae968cd01f1))
* gate virtual operations on state status ([c2edad2](https://github.com/chio-labs/sqlbuild/commit/c2edad2c79a9e3806d3294b234f35676271a03b8))

## [0.22.0](https://github.com/chio-labs/sqlbuild/compare/v0.21.0...v0.22.0) (2026-05-28)


### Features

* add virtual playground template ([9aa01a4](https://github.com/chio-labs/sqlbuild/commit/9aa01a4d292faaf75059bb25aa4771eae5a7c948))
* add virtual playground template ([a4ce508](https://github.com/chio-labs/sqlbuild/commit/a4ce5088f62ea1ebe0f6077ebdfc8357f2edbf06))

## [0.21.0](https://github.com/chio-labs/sqlbuild/compare/v0.20.1...v0.21.0) (2026-05-28)


### Features

* add virtual reconcile adopt detach ([ad63d06](https://github.com/chio-labs/sqlbuild/commit/ad63d06e58d9ad80895bf238d36b96fbf535ca0c))
* harden cross-schema virtual moves ([ee7e1de](https://github.com/chio-labs/sqlbuild/commit/ee7e1de76ea2044edf11bc8989a850e8045d51ee))
* harden virtual janitor cleanup ([da58879](https://github.com/chio-labs/sqlbuild/commit/da588794601110aa935c80c0d29a7808f61280c3))
* harden virtual rollback checkpoints ([92ce8a5](https://github.com/chio-labs/sqlbuild/commit/92ce8a52d7302fa293857aa3f4623ab005936037))
* harden virtual state repair cleanup ([3f599dc](https://github.com/chio-labs/sqlbuild/commit/3f599dc8a16f26a36851abe2a7ecc417c09ab615))
* prune virtual checkpoints with janitor ([66cbd2c](https://github.com/chio-labs/sqlbuild/commit/66cbd2c842ae00a9b4efbce384892a4dcd7ca730))
* seed virtual model versions ([7e0f07f](https://github.com/chio-labs/sqlbuild/commit/7e0f07fc76223696c989109bffda0ba51bba8e18))
* version virtual functions ([33cea90](https://github.com/chio-labs/sqlbuild/commit/33cea909541ef442837259ca8dbfd13aa6ea56e5))


### Bug Fixes

* polish virtual ops output ([e9a549a](https://github.com/chio-labs/sqlbuild/commit/e9a549ab4ec1dfe05903389f4561d029521602f1))
* polish virtual reconcile output ([f298ac9](https://github.com/chio-labs/sqlbuild/commit/f298ac9ec4bb6fb6b065cb693fb475c554033ee6))
* suppress virtual fingerprint table writes ([7bae1eb](https://github.com/chio-labs/sqlbuild/commit/7bae1eb83f6dc5cd1dbd87843471b022847fdf56))


### Documentation

* mention advanced virtual environments ([21b47ee](https://github.com/chio-labs/sqlbuild/commit/21b47ee5b3073780bf8e0c4fa5a09c57768fa972))
* trim scenario selector note ([34bc6e9](https://github.com/chio-labs/sqlbuild/commit/34bc6e917b71521ab257f82a35f0df3f9c8942b6))

## [0.20.1](https://github.com/chio-labs/sqlbuild/compare/v0.20.0...v0.20.1) (2026-05-26)


### Bug Fixes

* make sqlglot a core dependency ([a6e1517](https://github.com/chio-labs/sqlbuild/commit/a6e1517888e45cd4ddd8fdd1184f798bb7a27e69))
* make sqlglot a core dependency ([25119a7](https://github.com/chio-labs/sqlbuild/commit/25119a78d5b22841d680335b9e1e3f56991002d6))
* scaffold all project resource directories ([2fefe84](https://github.com/chio-labs/sqlbuild/commit/2fefe845d66c5fd174aa08d0a20ab6ebedad836d))

## [0.20.0](https://github.com/chio-labs/sqlbuild/compare/v0.19.0...v0.20.0) (2026-05-26)


### Features

* support scenario select options ([bb64dd8](https://github.com/chio-labs/sqlbuild/commit/bb64dd84ffc328f6c85a73998db4a3f86803c2b4))
* support scenario select options ([e6a90a9](https://github.com/chio-labs/sqlbuild/commit/e6a90a9792962acb28cc95802e888ebdfff47405))


### Bug Fixes

* honor build start timestamp cursor override ([49b6396](https://github.com/chio-labs/sqlbuild/commit/49b639647cb543d21df423e0eee8074479622db4))
* honor build start timestamp cursor override ([99be657](https://github.com/chio-labs/sqlbuild/commit/99be65746f6c5dd274319383815247abc4ea1b84))
* normalize path separators for Windows compatibility + add sqb init command ([0c95ecc](https://github.com/chio-labs/sqlbuild/commit/0c95eccf74534e74e53ada17d1673060873e5677))
* normalize path separators for Windows compatibility + init command ([75faf79](https://github.com/chio-labs/sqlbuild/commit/75faf796404f4277a92859ed1bfcbc5239bff39b))

## [0.19.0](https://github.com/chio-labs/sqlbuild/compare/v0.18.0...v0.19.0) (2026-05-24)


### Features

* add ingestr integration loaders ([d824860](https://github.com/chio-labs/sqlbuild/commit/d824860cb186b9a069e42c629b3680180fdd0552))
* formalize adapter identity ([3977fad](https://github.com/chio-labs/sqlbuild/commit/3977fad13c39ab93c7d5dacda2c1074b1a5b62c5))


### Bug Fixes

* run external loaders before warehouse connections ([b4427a4](https://github.com/chio-labs/sqlbuild/commit/b4427a4188f772b71011197ab87d25de52bdf932))

## [0.18.0](https://github.com/chio-labs/sqlbuild/compare/v0.17.0...v0.18.0) (2026-05-23)


### Features

* add sqlserver adapter ([89086bc](https://github.com/chio-labs/sqlbuild/commit/89086bc49c4a91a0e7cdb18c332abf4df4a59603))
* add sqlserver adapter ([8eb0f87](https://github.com/chio-labs/sqlbuild/commit/8eb0f8729be77b00962789845a3b6cf1bfa3e54f))
* expose loaders as dagster assets ([44bf2fc](https://github.com/chio-labs/sqlbuild/commit/44bf2fcf2346e51ff9d946f7226997a56ad39757))


### Bug Fixes

* align snowflake loader identifiers ([aef8920](https://github.com/chio-labs/sqlbuild/commit/aef89209a78b88e468e700beb4a69352adb7a472))
* configure databricks source deferral fixtures ([99c8540](https://github.com/chio-labs/sqlbuild/commit/99c85400245a2c3f3e00c0cb112cf5d0f8b42137))
* configure snowflake waffle source deferral ([31c64fc](https://github.com/chio-labs/sqlbuild/commit/31c64fc5550db211005c98043893b6991bde1745))
* restore portable fingerprint ddl ([9fadc15](https://github.com/chio-labs/sqlbuild/commit/9fadc15ca4331197b7d4f6696c2399ce9703aaff))
* restore postgres real warehouse tests ([ad6f04a](https://github.com/chio-labs/sqlbuild/commit/ad6f04ab09c5208a98a2152cf860a0abde6dc4c5))


### Documentation

* document source loaders ([65f3d6e](https://github.com/chio-labs/sqlbuild/commit/65f3d6e19cd68916c508238f22deb08b98597911))
* refresh sqlbuild skill ([52a28a6](https://github.com/chio-labs/sqlbuild/commit/52a28a69919a2183ed4afbae86edd347babe072a))

## [0.17.0](https://github.com/chio-labs/sqlbuild/compare/v0.16.2...v0.17.0) (2026-05-23)


### Features

* add concurrent load and seed execution ([0efa072](https://github.com/chio-labs/sqlbuild/commit/0efa072d3193a5f9ff3503ad25b11ef41aa2421b))
* add concurrent loader dag scheduling ([2bb66e9](https://github.com/chio-labs/sqlbuild/commit/2bb66e976d040ba481ea75eb7aea78ff25cf1c77))
* add incremental source loader writes ([d4a598b](https://github.com/chio-labs/sqlbuild/commit/d4a598bbe316f4f4d1132ffc0e2a20bb46067029))
* add loader cursor override context ([44abb1b](https://github.com/chio-labs/sqlbuild/commit/44abb1b9af224153011bc78fb8fa6ebd4374cd2e))
* add source deferral for managed sources ([ad03aa8](https://github.com/chio-labs/sqlbuild/commit/ad03aa888f3f7da25187829abc044544a0c1a78d))
* add source load command ([039ab0a](https://github.com/chio-labs/sqlbuild/commit/039ab0a9624fae8625eea60f817d9d2b64dc8523))
* add source loader dag execution ([96fa268](https://github.com/chio-labs/sqlbuild/commit/96fa268102c6907f3b5f93c220f047bf8b45f8fc))
* add source loader delete insert writes ([6aed601](https://github.com/chio-labs/sqlbuild/commit/6aed60105b966ba4f95f394cdeaf9f57ba059d98))
* auto-load sources during builds ([58bc22c](https://github.com/chio-labs/sqlbuild/commit/58bc22c60f06cdc49c3a2932a4b282c2f567672c))
* batch source loader staging writes ([b28e22b](https://github.com/chio-labs/sqlbuild/commit/b28e22bd134a7b34f997bd74f916613a465570e4))
* complete loader context helpers ([66d7836](https://github.com/chio-labs/sqlbuild/commit/66d783603ef5ff6e4976a4f9f111cbd8eab665bf))
* discover source loaders ([963fcf4](https://github.com/chio-labs/sqlbuild/commit/963fcf4ead4adae297cbe7a98b316a75a66d4d46))
* enforce source loader schema changes ([feb4820](https://github.com/chio-labs/sqlbuild/commit/feb4820ec182f6016c7c0e09f2a7d2b7fbf33bcd))
* infer source loader row types ([a345d15](https://github.com/chio-labs/sqlbuild/commit/a345d151a0a3277b92c6fda988f25be93007027e))
* move source SQL rendering behind adapters ([3e57e0b](https://github.com/chio-labs/sqlbuild/commit/3e57e0bf9670de6e380c880a7ff66ab04550463b))
* stage source loader table writes ([5b2c9d6](https://github.com/chio-labs/sqlbuild/commit/5b2c9d6af09325e14e88c7f1ac5f8095a8a59ea8))


### Bug Fixes

* disambiguate source loader selection ([f79424a](https://github.com/chio-labs/sqlbuild/commit/f79424a01823be92d09810bbe62d5da18f10cd03))
* harden source loader runtime ([a0c9f46](https://github.com/chio-labs/sqlbuild/commit/a0c9f469b312ba533a36232b2c11fd806bdad607))
* pass effective config to source loaders ([689f0df](https://github.com/chio-labs/sqlbuild/commit/689f0dff2d4d2fb7e4edd90b035c1428bf8e4d1f))

## [0.16.2](https://github.com/chio-labs/sqlbuild/compare/v0.16.1...v0.16.2) (2026-05-18)


### Documentation

* update README assertion example ([34de007](https://github.com/chio-labs/sqlbuild/commit/34de00739f28fca41237c80f3b13ce8a3dff6c56))
* update README assertion example ([1bbd600](https://github.com/chio-labs/sqlbuild/commit/1bbd600d3bd7f62220a03e68111a4fa3e1bed9aa))

## [0.16.1](https://github.com/chio-labs/sqlbuild/compare/v0.16.0...v0.16.1) (2026-05-17)


### Bug Fixes

* emit skill frontmatter before generated marker ([28c7a7c](https://github.com/chio-labs/sqlbuild/commit/28c7a7c8cdd2030552f7dae2eec1381a6e169c40))
* emit skill frontmatter before generated marker ([fc57032](https://github.com/chio-labs/sqlbuild/commit/fc57032d64eb6add883a00c6ef18633d5a6665e2))

## [0.16.0](https://github.com/chio-labs/sqlbuild/compare/v0.15.0...v0.16.0) (2026-05-17)


### Features

* add MotherDuck adapter ([403d536](https://github.com/chio-labs/sqlbuild/commit/403d536c0af1a65ed793d3dc6744fb923c32e1a3))
* add MotherDuck adapter ([36b70ef](https://github.com/chio-labs/sqlbuild/commit/36b70ef0746954db09a6755e79dd9d1ba585f947))
* add PostgreSQL adapter ([2b7b1c5](https://github.com/chio-labs/sqlbuild/commit/2b7b1c5822a75eda23ab1610541ce8cdcb998802))
* complete PostgresAdapter with testcontainer tests and e2e coverage ([428405c](https://github.com/chio-labs/sqlbuild/commit/428405cc0f9a4292b22bcd361df1584ef76bf1f0))


### Bug Fixes

* address reviewer feedback on postgres adapter coverage ([e1858bd](https://github.com/chio-labs/sqlbuild/commit/e1858bdcc1220a76861103c5c4acb5e4aaf501e1))
* make shared waffle fixture warehouse portable ([d8b84fa](https://github.com/chio-labs/sqlbuild/commit/d8b84fa64bf1c51950958c772b6bfc48d7c5c81d))
* make shared waffle fixture warehouse portable ([1ad677d](https://github.com/chio-labs/sqlbuild/commit/1ad677d47caa1fdb1fc7400e32004f724f285d9e))

## [0.15.0](https://github.com/chio-labs/sqlbuild/compare/v0.14.0...v0.15.0) (2026-05-17)


### Features

* add structured execution diagnostics ([4bb1374](https://github.com/chio-labs/sqlbuild/commit/4bb13749c79fdc7a9c3cba86252f93bc2c4262a1))
* tighten contract runtime diagnostics ([e051d44](https://github.com/chio-labs/sqlbuild/commit/e051d44663baf60b702578bb866499f0e43ad674))
* validate runtime model contracts ([b298ea1](https://github.com/chio-labs/sqlbuild/commit/b298ea1c858ac96f22280f894c95e53b5223e12d))
* validate source contract metadata ([ab906f2](https://github.com/chio-labs/sqlbuild/commit/ab906f2255ee7d2970a59aa05157035abc0aa435))


### Bug Fixes

* replace raw diagnostics with structured errors ([7a10cc6](https://github.com/chio-labs/sqlbuild/commit/7a10cc6602cc159144e42b14aea92d681ac6167d))


### Documentation

* update generated sqlbuild skill ([0d1f18a](https://github.com/chio-labs/sqlbuild/commit/0d1f18ac2abd563aab9fa7257436947b6be1510c))

## [0.14.0](https://github.com/chio-labs/sqlbuild/compare/v0.13.0...v0.14.0) (2026-05-16)


### Features

* add snapshot materialization config skeleton ([c0f220e](https://github.com/chio-labs/sqlbuild/commit/c0f220ea20b7cebb338aefcaf4c392b9024c79cf))
* enforce snapshot full refresh policy ([58a30e8](https://github.com/chio-labs/sqlbuild/commit/58a30e8e0af5d8198160dbf704c1ce912f37ed96))
* execute check snapshot models ([0715946](https://github.com/chio-labs/sqlbuild/commit/07159465cfe2c4ff9f98fb27d92ea1d3a72324c2))
* execute historical check snapshots ([f1d49d6](https://github.com/chio-labs/sqlbuild/commit/f1d49d624943d4542dce975dac5f03b34d554370))
* execute historical timestamp change snapshots ([d955081](https://github.com/chio-labs/sqlbuild/commit/d95508157df6f41fa88ca5067ab2b4a7ebac1e5a))
* execute historical timestamp snapshots ([8044d9c](https://github.com/chio-labs/sqlbuild/commit/8044d9c9447996b190c22dced3be203c5d2b4c37))
* execute timestamp snapshot models ([470b7aa](https://github.com/chio-labs/sqlbuild/commit/470b7aafe7380b61a6bed385f6ef1a43943c009b))
* handle snapshot schema changes ([64f160a](https://github.com/chio-labs/sqlbuild/commit/64f160a0a421f88fadea3871b0f070b9049f6bf0))
* invalidate historical snapshot hard deletes ([e2c148d](https://github.com/chio-labs/sqlbuild/commit/e2c148ddaa1706eada97a41377d833247b21484b))
* invalidate snapshot hard deletes ([9faa501](https://github.com/chio-labs/sqlbuild/commit/9faa501dc8ff1514eecf882255ef5de4cf7dd192))
* support snapshot validity configuration ([b7fd384](https://github.com/chio-labs/sqlbuild/commit/b7fd3843f49c66e099af111221d608af1b3427ce))
* support wildcard snapshot check columns ([90630f6](https://github.com/chio-labs/sqlbuild/commit/90630f630083d012ee39132d233c6c332f73c74c))


### Bug Fixes

* preserve warehouse-specific test SQL behavior ([bce7b6c](https://github.com/chio-labs/sqlbuild/commit/bce7b6c87331c039d2d1a143eeb8b46cb95443b8))
* run snapshot delta audits before mutation ([921b69c](https://github.com/chio-labs/sqlbuild/commit/921b69cb0a1d91a9d6aaf25a145f83e40a4b7343))
* validate source cursor input columns ([fb55504](https://github.com/chio-labs/sqlbuild/commit/fb55504913bff097819211eee53ab3c66184765b))


### Documentation

* update README source names ([f2041b7](https://github.com/chio-labs/sqlbuild/commit/f2041b78b544ecdd4e8a17139cf9d69b28be2551))

## [0.13.0](https://github.com/chio-labs/sqlbuild/compare/v0.12.0...v0.13.0) (2026-05-16)


### Features

* add Dagster asset loader ([6cf6e29](https://github.com/chio-labs/sqlbuild/commit/6cf6e29cb5c4887f2fa7e82bee60bd34768fc0e6))
* add Dagster CLI resource ([c718d55](https://github.com/chio-labs/sqlbuild/commit/c718d55a706e2e205722e548ec3ee5a9346d4019))
* add Dagster playground template ([718e77d](https://github.com/chio-labs/sqlbuild/commit/718e77d2cbc14d6760b7af5dac50d1249ef234db))
* add Dagster project preparation ([df92013](https://github.com/chio-labs/sqlbuild/commit/df9201367a8359fd838c12f142941d4e7a3c11b2))
* add Dagster scenario checks helper ([75473bd](https://github.com/chio-labs/sqlbuild/commit/75473bd36455132be1df12d25a737aa112c9e10a))
* add static DAG JSON output ([1c8f497](https://github.com/chio-labs/sqlbuild/commit/1c8f49752bd72719ab3b5ff4e9c02b28fbcc1d1b))
* bridge Dagster asset selections ([2e60398](https://github.com/chio-labs/sqlbuild/commit/2e603981c9ffc2aeab241645080fbbe41d68c3d3))
* emit Dagster events from execution JSON ([4e5e1b6](https://github.com/chio-labs/sqlbuild/commit/4e5e1b62898a32a9a3d060f5b6f3aca38587ad8b))
* select Dagster scenarios from assets ([6f584c8](https://github.com/chio-labs/sqlbuild/commit/6f584c83702689d90931006ba1bae19a5b2b1407))
* stream SQLBuild output in Dagster ([b603449](https://github.com/chio-labs/sqlbuild/commit/b60344921fb88fb780239dc55aaf0d160b30a5f7))
* tag SQLBuild Dagster asset kinds ([342bb61](https://github.com/chio-labs/sqlbuild/commit/342bb613dc022dd92f242a6035b1d2918ffd4926))


### Bug Fixes

* group Dagster assets by SQLBuild project ([00c324f](https://github.com/chio-labs/sqlbuild/commit/00c324fa7feced8ae78c4fca98e89a2bfff8eccc))
* improve Dagster demo and logging ([e120222](https://github.com/chio-labs/sqlbuild/commit/e12022213990de3c4fdf45708a0dfda8d8ddf157))

## [0.12.0](https://github.com/chio-labs/sqlbuild/compare/v0.11.0...v0.12.0) (2026-05-15)


### Features

* add sqlglot cte parsing fallback ([a45c3ea](https://github.com/chio-labs/sqlbuild/commit/a45c3ea3b68ee184a1a6ce26f93b72f2fb2d4eff))
* allow scenario fixtures to sample sources ([a348eb9](https://github.com/chio-labs/sqlbuild/commit/a348eb99f82456d6203274a7785f46670d4aef40))
* allow scenario fixtures to sample sources ([cc363ca](https://github.com/chio-labs/sqlbuild/commit/cc363cad2bacdfc75b8e115682b574a226242866))

## [0.11.0](https://github.com/chio-labs/sqlbuild/compare/v0.10.0...v0.11.0) (2026-05-15)


### Features

* add macro SQL test mode ([6498527](https://github.com/chio-labs/sqlbuild/commit/64985278ed1fb46d6af50d15e24f09eea8e2d544))
* add macro SQL test mode ([1e6c6c6](https://github.com/chio-labs/sqlbuild/commit/1e6c6c6904c1b304d5c39c11d1464c6103f5a075))
* add table function SQL tests ([3b07fc6](https://github.com/chio-labs/sqlbuild/commit/3b07fc6fb61e3aae995c3cf7b6821600b1ec5b4d))
* add table function SQL tests ([8e8c1e4](https://github.com/chio-labs/sqlbuild/commit/8e8c1e4cbe28be4703dd51cd5bb938fab6e55b8a))
* add UDF SQL test mode ([150b552](https://github.com/chio-labs/sqlbuild/commit/150b55225caab527a227f5568645b8e8a8437d2f))
* add UDF SQL test mode ([5738a42](https://github.com/chio-labs/sqlbuild/commit/5738a42c16d0041dffdb7c7ff5dedc16268d74e3))

## [0.10.0](https://github.com/chio-labs/sqlbuild/compare/v0.9.0...v0.10.0) (2026-05-14)


### Features

* add skills update command ([e2546eb](https://github.com/chio-labs/sqlbuild/commit/e2546eb6b6d8963ba60c80d858612a861f667487))

## [0.9.0](https://github.com/chio-labs/sqlbuild/compare/v0.8.0...v0.9.0) (2026-05-14)


### Features

* add dbt interop debug ([8d71604](https://github.com/chio-labs/sqlbuild/commit/8d7160440446e9a242c3a67b028f50e15a8b7134))
* add dbt interop foundation ([6d548b9](https://github.com/chio-labs/sqlbuild/commit/6d548b9c386895e707fcbb3a118f24bfc9cb6c26))
* add dbt interop plan output ([9e5d012](https://github.com/chio-labs/sqlbuild/commit/9e5d0123001a9530e307e97ba485d7a88d2763cc))
* add warehouse zero copy clone support ([fef64e8](https://github.com/chio-labs/sqlbuild/commit/fef64e8dc1b2cda62c0419d793a1a7ec8cdfa030))
* build dbt combined graph ([a6ca203](https://github.com/chio-labs/sqlbuild/commit/a6ca203fd9b6f4aa989d3d059c0364048884fbc3))
* execute dbt interop run and build ([91e80a5](https://github.com/chio-labs/sqlbuild/commit/91e80a5e10cc9e9c91533c61ae0c86e919c79b23))
* execute dbt interop tests ([cd3c556](https://github.com/chio-labs/sqlbuild/commit/cd3c55678db2df8d2500300ad3624575558f1e2b))
* mock dbt refs in tests and scenarios ([0bc7d40](https://github.com/chio-labs/sqlbuild/commit/0bc7d40e8e6649390b4dd7df969dfb0343ac3ce5))
* orchestrate dbt interop plans ([92e8f39](https://github.com/chio-labs/sqlbuild/commit/92e8f39ae03f81e1b09c08c3aa463a6f61c7d042))
* refine dbt plan output ([4090fa6](https://github.com/chio-labs/sqlbuild/commit/4090fa6a927e7fcf5dada88aa7e124c3d4ed11cd))
* refine plan section layout ([0ea662c](https://github.com/chio-labs/sqlbuild/commit/0ea662c410065efa8363949014668f19e31541c0))
* resolve dbt interop selections ([87cdd3a](https://github.com/chio-labs/sqlbuild/commit/87cdd3a6c3b5f90ca3fdfa0b791be2dd1374234f))
* resolve dbt refs from manifest ([0ab4921](https://github.com/chio-labs/sqlbuild/commit/0ab49216ea0b43c2f293556132aa5074c9bd6ad7))
* route dbt interop arguments ([7fc4da3](https://github.com/chio-labs/sqlbuild/commit/7fc4da3aa2f615584c169a0781cf690ac31b1881))
* support adapter sqlglot dialect setting ([56926f3](https://github.com/chio-labs/sqlbuild/commit/56926f34bf8ad197f7c556355d87eead4a01f7fc))
* wire dbt interop plan cli ([80d72d8](https://github.com/chio-labs/sqlbuild/commit/80d72d8e4fbb0b2ec85486b71840b27a050a02bf))


### Bug Fixes

* default audit severity to error ([98f0dea](https://github.com/chio-labs/sqlbuild/commit/98f0dea12cdbc09c7ef5dbe58b79082985d5931d))
* default audit severity to error ([f9cf03f](https://github.com/chio-labs/sqlbuild/commit/f9cf03f88e00a581c74d3438c5e041bc621724ee))
* escape reference example calls ([bf8273f](https://github.com/chio-labs/sqlbuild/commit/bf8273fb9a9aec2997384d213ca6f4be37470fee))
* keep local replay error fixtures valid ([7f48d4c](https://github.com/chio-labs/sqlbuild/commit/7f48d4c48fdae80979bef3a2b2db943b06b74af5))
* recognize dbt scenario artifacts ([c1f3274](https://github.com/chio-labs/sqlbuild/commit/c1f3274db1ef82aa78f597c97bd25063ea00f6b3))
* recognize dbt scenario artifacts ([6b8a5f5](https://github.com/chio-labs/sqlbuild/commit/6b8a5f542fbe19d4a824ba3a5b310f9ad840b808))
* reject dbt refs in audits ([e384ed1](https://github.com/chio-labs/sqlbuild/commit/e384ed185e31c2e0dbbccb826bed0b40f222240c))
* remove dbt mock fixture model ([e1c3a8d](https://github.com/chio-labs/sqlbuild/commit/e1c3a8da182bb0dce71c9917ad4d29fe19e1cc6a))
* render coded dbt interop errors ([12395a1](https://github.com/chio-labs/sqlbuild/commit/12395a19e7fbbb82971b814858cace906642f192))
* require sqlglot for scenario snapshots ([7b6cf94](https://github.com/chio-labs/sqlbuild/commit/7b6cf94d4fbfad66581b0279909830aaabcf5f9a))
* resolve dbt interop paths absolutely ([c3a21ee](https://github.com/chio-labs/sqlbuild/commit/c3a21ee5e42ac9e9fe8fab79e6c0c08066a8862b))
* stop ambient dbt manifest discovery ([9e35257](https://github.com/chio-labs/sqlbuild/commit/9e35257b0e12f2c68d8525d684d2f96ac179582b))


### Documentation

* use absolute readme logo url ([a5b5d2c](https://github.com/chio-labs/sqlbuild/commit/a5b5d2c8003aa41125752a084b36865e6d7e62bc))

## [0.8.0](https://github.com/chio-labs/sqlbuild/compare/v0.7.0...v0.8.0) (2026-05-09)


### Features

* add scenario test CLI ([8ff989a](https://github.com/chio-labs/sqlbuild/commit/8ff989a4bc98fbf095b39106bf6b73f88aec071e))
* add SQL unit test assertions ([640fbcb](https://github.com/chio-labs/sqlbuild/commit/640fbcb2fdcb090bba221524af32d665427a557e))
* add waffle shop playground ([a91f805](https://github.com/chio-labs/sqlbuild/commit/a91f805ac8a0eeaf5bf830a2dfdfe0d22c85f2e0))
* clean scenario artifacts ([2293a8a](https://github.com/chio-labs/sqlbuild/commit/2293a8a97098c3fa5a90ae7314dc06f1a89875bd))
* clean scenario artifacts with janitor ([19df620](https://github.com/chio-labs/sqlbuild/commit/19df620850dee53bf09724f91a3f3469d2468e13))
* discover SQL scenarios ([d280dff](https://github.com/chio-labs/sqlbuild/commit/d280dff043714a7f48dc1b5de0c63291b06bdd68))
* execute scenario checks ([a32ad16](https://github.com/chio-labs/sqlbuild/commit/a32ad16e2607fa02c6678c4582167c8ce0118cc0))
* execute scenario model graph ([8cba144](https://github.com/chio-labs/sqlbuild/commit/8cba1444d1fa3fd0dadcdae989a69fc810206960))
* extract scenario CTE roles ([0ecbabf](https://github.com/chio-labs/sqlbuild/commit/0ecbabfba117c7eca1d3e4635e5117ac508f47b5))
* infer scenario graph requirements ([555d545](https://github.com/chio-labs/sqlbuild/commit/555d54514cb6e0e4f664040c573901dcc0ed15b6))
* load scenario project seeds ([0b52756](https://github.com/chio-labs/sqlbuild/commit/0b52756863816e7aa1d15af273619b607ea73293))
* materialize scenario fixtures ([b49179a](https://github.com/chio-labs/sqlbuild/commit/b49179a2990aeb9b6cb7fe2c48bdd2c83b7a3784))
* name scenario artifacts ([93e0cb6](https://github.com/chio-labs/sqlbuild/commit/93e0cb6de5eddafbc6b7969fce90069ddba1d63f))
* plan scenario fixture SQL ([8fd0a69](https://github.com/chio-labs/sqlbuild/commit/8fd0a69e732d03620d4fa6cc2b50e83b77a8ceb0))
* plan scenario relation overrides ([1dfb94b](https://github.com/chio-labs/sqlbuild/commit/1dfb94bc8797cf0250f517eedc91015e7040306b))
* show nested SQL unit test checks ([b096564](https://github.com/chio-labs/sqlbuild/commit/b0965646820fc96e27e4576b37e9e25bf00f7eb8))
* write scenario runtime artifacts ([8add531](https://github.com/chio-labs/sqlbuild/commit/8add531783f03dc348368f6547e61da941f0ec16))


### Bug Fixes

* keep playground install minimal ([e67a144](https://github.com/chio-labs/sqlbuild/commit/e67a14464c90653373eddd4bdfef46855802a6fc))

## [0.7.0](https://github.com/chio-labs/sqlbuild/compare/v0.6.0...v0.7.0) (2026-05-07)


### Features

* add accepted values and relationships audits ([7a12fd3](https://github.com/chio-labs/sqlbuild/commit/7a12fd3098dfe885a73f7a8922daa28e3893a1e0))
* add nullable contracts and built-in audits ([611f99a](https://github.com/chio-labs/sqlbuild/commit/611f99a689440b29af5ebe835e27de6967c60d8d))
* add spinner based CLI progress ([9cce465](https://github.com/chio-labs/sqlbuild/commit/9cce465c454895bc79a35e9bee3cadbcd2ca34b3))
* add spinner based CLI progress ([9664cd9](https://github.com/chio-labs/sqlbuild/commit/9664cd9eb608819806cc2d7f9d59d57ecedbf987))
* align config and table function names ([b011ead](https://github.com/chio-labs/sqlbuild/commit/b011eadb0b9d3991dcdf53f522bb8d6f73682974))
* align config and table function names ([ec77895](https://github.com/chio-labs/sqlbuild/commit/ec7789511c095bd1d572a570e73a5a8e5867598d))
* show progress errors below rows ([cbe4a2a](https://github.com/chio-labs/sqlbuild/commit/cbe4a2a76232c0b854a369e5af02f5ebb0fbc5d8))
* show progress errors below rows ([4908b24](https://github.com/chio-labs/sqlbuild/commit/4908b24ef7e9d627d5c354dc5ebebdc255a38991))
* unify authored SQL interpolation ([c1d7e77](https://github.com/chio-labs/sqlbuild/commit/c1d7e77ec2545877949d59288d7c942afe031de3))
* unify CLI progress output ([0695eea](https://github.com/chio-labs/sqlbuild/commit/0695eeaca6b49e15152bead00e578c1f647c4936))


### Bug Fixes

* align plan output columns ([8af7b74](https://github.com/chio-labs/sqlbuild/commit/8af7b740be77138b773fb44a4366a14bbad5d828))
* narrow function change fingerprints ([5bc5896](https://github.com/chio-labs/sqlbuild/commit/5bc58967bfa26e6330a78d4657f19a2bb4fd13ee))
* narrow function change fingerprints ([a0bf9bf](https://github.com/chio-labs/sqlbuild/commit/a0bf9bf4a020a84066078d798b9b9fc76af7b591))
* respect disabled sqlglot setting ([11ca558](https://github.com/chio-labs/sqlbuild/commit/11ca55858ac91a9be060f22b100fca4a938c80ac))


### Documentation

* add contributing guide and update logo ([4e3ce74](https://github.com/chio-labs/sqlbuild/commit/4e3ce748497d3eec80b85387f9f9e261ee715c30))
* use relative README logo path ([0203ad8](https://github.com/chio-labs/sqlbuild/commit/0203ad8ca827f8d9c23309432601ce27b27b4b82))

## [0.6.0](https://github.com/chio-labs/sqlbuild/compare/v0.5.0...v0.6.0) (2026-05-07)


### Features

* add column lineage analyzer ([f60c65d](https://github.com/chio-labs/sqlbuild/commit/f60c65d1b4fd7d7d9512d75023f829888e555fb6))
* add column lineage CLI traces ([83977a0](https://github.com/chio-labs/sqlbuild/commit/83977a048fe6dc978909b85ddbf1c619ba39468f))
* add compile contract diagnostics ([0ba4d65](https://github.com/chio-labs/sqlbuild/commit/0ba4d65e66a1df8b519ac62a6b9e2941f0998b83))
* add conservative nullability inference ([74b6bb2](https://github.com/chio-labs/sqlbuild/commit/74b6bb22a614fae5c106a48032e54107b0b8a479))
* add fast compile lineage mode ([19fb926](https://github.com/chio-labs/sqlbuild/commit/19fb9261d13bb1947b990fe35a5363957cca2bc7))
* add lineage mode metadata ([726ebb0](https://github.com/chio-labs/sqlbuild/commit/726ebb04fa08178dbf323ad1632e03c341272af4))
* improve compile output ([8bb26f9](https://github.com/chio-labs/sqlbuild/commit/8bb26f97e5770895ea472dea27f06cda3d50ff9c))
* make compile offline ([0099e0d](https://github.com/chio-labs/sqlbuild/commit/0099e0d61f458f68ba14b53531826a5ac77c9fa6))
* scope rich column lineage ([521e47a](https://github.com/chio-labs/sqlbuild/commit/521e47adfde3f6cdb42cc8b5772553df4ca20e91))

## [0.5.0](https://github.com/chio-labs/sqlbuild/compare/v0.4.0...v0.5.0) (2026-05-06)


### Features

* switch project config to TOML ([e7127b0](https://github.com/chio-labs/sqlbuild/commit/e7127b00663b4773d1897500ebc1492b25dff285))
* switch project config to TOML ([b6f208d](https://github.com/chio-labs/sqlbuild/commit/b6f208d7ca8f39a7bf9bce15c9d964a2df90f941))

## [0.4.0](https://github.com/chio-labs/sqlbuild/compare/v0.3.0...v0.4.0) (2026-05-05)


### Features

* add debug command ([b7a67f5](https://github.com/chio-labs/sqlbuild/commit/b7a67f5fbc4bcb963ac36988f6a18a9127742d91))
* add debug command ([1193f75](https://github.com/chio-labs/sqlbuild/commit/1193f75149787bac84f3907ef166409020a64814))

## [0.3.0](https://github.com/chio-labs/sqlbuild/compare/v0.2.1...v0.3.0) (2026-05-05)


### Features

* add lineage command ([9aef7d6](https://github.com/chio-labs/sqlbuild/commit/9aef7d69ad556a2ef0ec568f1aed4492a119dfff))
* add lineage command ([a0af73a](https://github.com/chio-labs/sqlbuild/commit/a0af73adfd5dba1870ff5b3b7293a78967a578c8))

## [0.2.1](https://github.com/chio-labs/sqlbuild/compare/v0.2.0...v0.2.1) (2026-05-05)


### Bug Fixes

* render README correctly on PyPI ([8abf270](https://github.com/chio-labs/sqlbuild/commit/8abf270a77aabd1040ce223fc9f377eb168bfe39))
* render README correctly on PyPI ([8251c0d](https://github.com/chio-labs/sqlbuild/commit/8251c0d6bf5b8277d36a7756c1fa95b04d1df698))

## [0.2.0](https://github.com/chio-labs/sqlbuild/compare/v0.1.0...v0.2.0) (2026-05-05)


### Features

* add --defer-to support with deferred target resolution, buildability checks, and SC026 limit fix ([2097d80](https://github.com/chio-labs/sqlbuild/commit/2097d8020b3c0aa970ab7b7b038ab2aa2534fe09))
* add --json output flag for sqb compile and sqb plan commands ([008d938](https://github.com/chio-labs/sqlbuild/commit/008d938ca6dfc53a4dbeab9aa63c5b9024a97596))
* add --verbose flag to build/run showing full model DDL and audit SQL inline ([3f3cfe0](https://github.com/chio-labs/sqlbuild/commit/3f3cfe01dd256effdf71d24200b425cd10660337))
* add adapter spine with base/strict adapters and compile assembly ([b72fbff](https://github.com/chio-labs/sqlbuild/commit/b72fbff3b15680b795ddaa864c867b451cfb9f7f))
* add append cursor boundary control ([796af13](https://github.com/chio-labs/sqlbuild/commit/796af130cfa2fa79e595023187a950043bc34336))
* add audit scheduling with attachment resolution, graph validation, and effective run scope degradation ([587d51e](https://github.com/chio-labs/sqlbuild/commit/587d51ebd44a28b8d64c5cb1413443710a52e4ec))
* add audit/test plan entries with chained test resolution and unresolved ref validation ([b1e0be7](https://github.com/chio-labs/sqlbuild/commit/b1e0be74183a454e34662c1ca675e44fcce1c3fb))
* add backfill cascade propagation with duration comparison, root cause attribution, and cross-type rules ([f27eb09](https://github.com/chio-labs/sqlbuild/commit/f27eb09435bff1a53b9eb27393271e7cb815193e))
* add BigQuery adapter support ([f79ff90](https://github.com/chio-labs/sqlbuild/commit/f79ff90b813cf751d5a5313416eb9a0be3ff60e7))
* add BigQuery warehouse parity ([1415f8e](https://github.com/chio-labs/sqlbuild/commit/1415f8e7ba8e48457d5ed0691a486212a85dee3a))
* add build executor with topo execution, failure propagation, source/end audits, and direct/staged mode support ([d31466d](https://github.com/chio-labs/sqlbuild/commit/d31466d51d55234f4a73c66dd6b59e9a49f853b2))
* add build output formatter with live progress, color support, and duration tracking ([42d1218](https://github.com/chio-labs/sqlbuild/commit/42d12183b2c3dd012f541d5cc8d753ae634bf09e))
* add buildability checks validating upstream deps exist in scope or warehouse ([00f8c99](https://github.com/chio-labs/sqlbuild/commit/00f8c9946be85cb3e79416caa86e854fb3ea2230))
* add change detection with query/schema comparison, backfill policy resolution, and SC033 cross-package import rule ([97e4582](https://github.com/chio-labs/sqlbuild/commit/97e45826398882c381f936ebf15cbf271a11c885))
* add clone command support ([350a94b](https://github.com/chio-labs/sqlbuild/commit/350a94bd2698ee6ced0bd5b3604c0597b6de6c1f))
* add compile input attachment layer ([724c936](https://github.com/chio-labs/sqlbuild/commit/724c9364d565623537a974effff6723966c4adcf))
* add compile-time sql macro expansion ([1e78559](https://github.com/chio-labs/sqlbuild/commit/1e78559591edf9c081c358760a0474206e4494a4))
* add compile-time SQL syntax validation with project, model, and CLI escape hatches ([02b16f5](https://github.com/chio-labs/sqlbuild/commit/02b16f55655936d5d706a9fbcf0878915be8c02b))
* add concurrent build scheduler with ready-queue DAG dispatch ([92ad4c4](https://github.com/chio-labs/sqlbuild/commit/92ad4c48342ee94df59e288838e5f4a20c7e2d87))
* add ctx.execute_sql() for custom materializations and require statement_recorder on adapter write methods ([353b1ea](https://github.com/chio-labs/sqlbuild/commit/353b1eaf995ee7c47c2fa1c27b8d59be2c479ca9))
* add cursor start floors ([a47cfa5](https://github.com/chio-labs/sqlbuild/commit/a47cfa5be7cf7a85718dbc46fe79c599ec56fb54))
* add cursor_type/validation/typed overrides/sql vars with StrEnum enforcement ([418925c](https://github.com/chio-labs/sqlbuild/commit/418925c3b5d9eefe33ed3303b09e647c6354810a))
* add custom materializations with @[@placeholder](https://github.com/placeholder) support and user-callable audits ([d9e1592](https://github.com/chio-labs/sqlbuild/commit/d9e1592560f6fbb31d210046f3f9c84e5fbb43e7))
* add Databricks adapter support ([e692ce6](https://github.com/chio-labs/sqlbuild/commit/e692ce64e19a7156d5c1a6ff7a41b9401107e2e7))
* add Databricks Python UDF support ([849e759](https://github.com/chio-labs/sqlbuild/commit/849e759b5aa95c26c65e733d704e3f7057f7cf3a))
* add DuckDB adapter with full method coverage and remove unused ResolutionMixin ([57ec8d9](https://github.com/chio-labs/sqlbuild/commit/57ec8d97ad5a12d254c492c8ceccadde2797eb18))
* add e2e tests for all CLI commands and fix --project-dir relative database path resolution ([170b4b5](https://github.com/chio-labs/sqlbuild/commit/170b4b564c2e61462f12917b6a7b82fb37d4fbca))
* add environment diff command support ([c46fdca](https://github.com/chio-labs/sqlbuild/commit/c46fdca731bcfef3d30242c14337e7c76afd47b9))
* add executor build runtime stage 1 spec and validation enums, settings, audit severity/run_scope resolution, and non-incremental config guards ([70172f1](https://github.com/chio-labs/sqlbuild/commit/70172f160f3da29b9b7aa1ef882743e70f006d0f))
* add fingerprint storage with per-schema read/write and hash computation ([727b2d5](https://github.com/chio-labs/sqlbuild/commit/727b2d5857fa804035f5b9ff5ae815b911807278))
* add incremental executor with delta/DML lifecycle, schema change handling, and adapter DDL methods ([cf20aec](https://github.com/chio-labs/sqlbuild/commit/cf20aec008dd817599c313f6163a939dad10f203))
* add Layer 5 ref resolution with cursor bounds, source CAST wrapping, and batched snapshot gathering ([4bf9975](https://github.com/chio-labs/sqlbuild/commit/4bf9975a22c95845b4d864770f4f962802df9daf))
* add Layer 6 materialization strategy, plan output, and execution plan orchestration ([4061622](https://github.com/chio-labs/sqlbuild/commit/4061622247faced889fd5504264a54013f2c3672))
* add manifest writer module with dbt v12-compatible serialization ([32f20f2](https://github.com/chio-labs/sqlbuild/commit/32f20f2581edf2c72f8e99bfc7f33c5159059814))
* add max concurrency project setting ([43b3ab5](https://github.com/chio-labs/sqlbuild/commit/43b3ab5d598876d59774217a5b8eea53845cae24))
* add microbatch executor with sentinel substitution, cursor-range delete_insert, and batch splitting ([d1eb879](https://github.com/chio-labs/sqlbuild/commit/d1eb87987412ddb5ef3c42539514b2dcd6d2321c))
* add model target context to compile templates ([45db2e9](https://github.com/chio-labs/sqlbuild/commit/45db2e99ab8f41ab23c3d395d87259e9d55caedd))
* add on_progress callback to custom materialization context ([e717ff5](https://github.com/chio-labs/sqlbuild/commit/e717ff5a6b7b45808953ee12b7112866ff467fba))
* add optional sqlglot test projection parsing ([b75f9d7](https://github.com/chio-labs/sqlbuild/commit/b75f9d7581686b351fcc941c9e23f780ba0aaaa3))
* add path:folder selector resolution and --no-sql-validation CLI flag ([d6b3951](https://github.com/chio-labs/sqlbuild/commit/d6b39515e734d9cf699b3ee2071c881745b42959))
* add plan display before build execution and use_color support on plan formatter ([752bbad](https://github.com/chio-labs/sqlbuild/commit/752bbad4f6ee7a525f7cd41d540ea7846a85bba7))
* add planner graph helpers with topo sort, expansion, and path finding ([3fcab8e](https://github.com/chio-labs/sqlbuild/commit/3fcab8eb25de793c96dfc26d13a42246a6b62493))
* add planner-ready compiled resource models and dependency helpers ([fee76a9](https://github.com/chio-labs/sqlbuild/commit/fee76a97888988a0ec9e6c5ec11f1f098c003459))
* add project-local adapter discovery ([ea7d06d](https://github.com/chio-labs/sqlbuild/commit/ea7d06d45a4dc446ce8190f2f3a6cc6a4ffc2b58))
* add Python UDF support ([e9c8e22](https://github.com/chio-labs/sqlbuild/commit/e9c8e224b152653071a1f653501f1b9ba654cbe2))
* add query command ([c80a508](https://github.com/chio-labs/sqlbuild/commit/c80a508fcaec051a7aaff9a54f0002a090bba239))
* add query command ([ffe1435](https://github.com/chio-labs/sqlbuild/commit/ffe143545ffb0d8dab2ff77ef496601dc99d7ff3))
* add raw schema metadata discovery ([22811f8](https://github.com/chio-labs/sqlbuild/commit/22811f8eba52a603901025e82d11bfcca5e437e1))
* add raw source discovery parsing ([0ff1185](https://github.com/chio-labs/sqlbuild/commit/0ff11850d0d0184c8906f096f668b46eb15792b8))
* add raw sql audit discovery parsing ([e62b123](https://github.com/chio-labs/sqlbuild/commit/e62b1233d2f1cc66c513c3a07395cd4d72b1420a))
* add rich diff summary output ([9743b7b](https://github.com/chio-labs/sqlbuild/commit/9743b7b4fe06579d9a649979ef8a0b22b58a8fda))
* add runtime audit rendering with relation overrides, AuditOutcome enum, and AuditExecutionResult model ([8272844](https://github.com/chio-labs/sqlbuild/commit/82728445a5582a8d3270b6927d081a316027cc4b))
* add runtime SQL statement recorder ([f3e943e](https://github.com/chio-labs/sqlbuild/commit/f3e943ed33ca9697a042970b001d3fa7a224cbae))
* add safe janitor cleanup command ([ec1e02c](https://github.com/chio-labs/sqlbuild/commit/ec1e02ccfb89681a7ffa9b0820b50b8c2d125409))
* add seed target overrides ([4c89d0c](https://github.com/chio-labs/sqlbuild/commit/4c89d0ca21dcc0031a9047a5402090f4fea40689))
* add selector parsing and scope resolution with union, intersection, and exclude ([fe8a17b](https://github.com/chio-labs/sqlbuild/commit/fe8a17b6e13627335ecbe5e3b6d0a43f8aac4b76))
* add shared type normalization ([3a3103a](https://github.com/chio-labs/sqlbuild/commit/3a3103a22072686f10bf688b24328191dcbd6c18))
* add shared type normalization ([ac7d895](https://github.com/chio-labs/sqlbuild/commit/ac7d89533bdd0a0e873ebce77496fa626553c5fd))
* add snowflake adapter support ([0da4ecf](https://github.com/chio-labs/sqlbuild/commit/0da4ecf94528212b59262d640b40cfe23f5aac9a))
* add source quality metadata parsing ([6ec1e1e](https://github.com/chio-labs/sqlbuild/commit/6ec1e1e76834998e47fe83c094d580b5a59c1dd8))
* add sqb compile command with pipeline orchestration, target writer, and adapter defaults ([2446785](https://github.com/chio-labs/sqlbuild/commit/2446785ad6861c2286d027db6125f9aedc2def09))
* add sqb plan command with compact/verbose output, waffle_shop example, and seed ref fixes ([038314b](https://github.com/chio-labs/sqlbuild/commit/038314b788a5da3d15616cfc9d57a75d0ebfaf47))
* add SQL table functions ([52aaadd](https://github.com/chio-labs/sqlbuild/commit/52aaaddbc11b7017a0790f4742f70af789a41667))
* add SQL table functions ([d6e9a04](https://github.com/chio-labs/sqlbuild/commit/d6e9a0468bc8195e2117a1d8409614910b6d667f))
* add SQL UDF resources ([24149b0](https://github.com/chio-labs/sqlbuild/commit/24149b001c7cbe565fda2b66f521932a68f04158))
* add sql-style model headers ([7a80177](https://github.com/chio-labs/sqlbuild/commit/7a801779042dbd5ed18e2729ed695b4b123760de))
* add sqlbuild as an alias for sqb CLI entry point ([d69535e](https://github.com/chio-labs/sqlbuild/commit/d69535ed71738dc9150e60d8e3df089e43d6bf81))
* add sqlglot column inference for schema change detection with source attribution ([c68ef74](https://github.com/chio-labs/sqlbuild/commit/c68ef743c843ac144bd4494012d36043a530229d))
* add table executor with staged/direct lifecycle, type enforcement, audit execution, hooks, and fingerprint write ([36de9f2](https://github.com/chio-labs/sqlbuild/commit/36de9f27361b75138e7f294a65d9abc3de6bc1ca))
* add template expression helpers ([23b3a2b](https://github.com/chio-labs/sqlbuild/commit/23b3a2be6b4ffa18f1d71c18494f9c8042b33496))
* add transaction() context manager and transactional atomicity tests ([497b613](https://github.com/chio-labs/sqlbuild/commit/497b613d5349038b1079da919095cbc895c59af2))
* add type_enforcement flag to schema change detection for yml vs inferred precedence ([c97e8d3](https://github.com/chio-labs/sqlbuild/commit/c97e8d36638024694c15d5d57cebb17e8c7bc45e))
* add view executor, SQL unit test executor, and planner test ordering for Stage 8 ([38890d8](https://github.com/chio-labs/sqlbuild/commit/38890d89fce9b06baeff2087e09ec33815d353b3))
* add warehouse cursor type consistency check with heuristic and sqlglot classification ([0726216](https://github.com/chio-labs/sqlbuild/commit/0726216864c2707c61ced52ad077afaacb35896c))
* add warehouse snapshot gathering with bulk relation, column, and fingerprint reads ([c09e764](https://github.com/chio-labs/sqlbuild/commit/c09e764ae609e6ae7fe9963b02251d7583c16044))
* attach defaults and path defaults to compile model inputs ([8aec92c](https://github.com/chio-labs/sqlbuild/commit/8aec92c7fcbc0cd61e747efea1ce4abb366c0461))
* bootstrap repo and port convention checkers ([4967f63](https://github.com/chio-labs/sqlbuild/commit/4967f63fde8d68b1947f03988b63346ef9a73280))
* compile attached generic audits ([b619e53](https://github.com/chio-labs/sqlbuild/commit/b619e539e061f7d4df5b7a6b5af1dc0176b8f1c2))
* compile test and audit sql surfaces ([9482476](https://github.com/chio-labs/sqlbuild/commit/9482476fd72e54daf06e9fd027ce76e6b4c50272))
* default audits to delta and final ([25099c2](https://github.com/chio-labs/sqlbuild/commit/25099c2669679b683ad5fb1b0cba605504343a79))
* enforce typed source columns by default ([6d80f38](https://github.com/chio-labs/sqlbuild/commit/6d80f3898148b7667e6c388e0194e8fe440aae9e))
* enforce typed source columns by default ([4fd4e9b](https://github.com/chio-labs/sqlbuild/commit/4fd4e9b2d2fe03dc0017b435d9e1bc220ec85be6))
* expand compile templates in config layers ([f21b124](https://github.com/chio-labs/sqlbuild/commit/f21b124a2f1f7c822b4607fc41556180c47af97c))
* expand diff examples and samples ([884693f](https://github.com/chio-labs/sqlbuild/commit/884693f0c35da2ded25542dd797940870a516b71))
* expand discovery parsing and cli error handling ([69f23e2](https://github.com/chio-labs/sqlbuild/commit/69f23e2d34a029fa536e1b21047b8027ea756442))
* expand path selector endpoint syntax ([938ec22](https://github.com/chio-labs/sqlbuild/commit/938ec22979fdb9899b89450ddfc1e00bbb67db49))
* expand SQL UDF compile inputs ([9d6f2df](https://github.com/chio-labs/sqlbuild/commit/9d6f2df51a58c2f3b2209608af1c567548ed9242))
* extract logical sql references during compile ([01517e6](https://github.com/chio-labs/sqlbuild/commit/01517e6e8fee2c43cc735d5889b0b3c5990260d5))
* extract sql test cte semantics during compile ([65b789a](https://github.com/chio-labs/sqlbuild/commit/65b789a3290f842e616f40502c0e62d5eb9c6712))
* filter warehouse metadata by selected scope ([e10cd8a](https://github.com/chio-labs/sqlbuild/commit/e10cd8a3c26aab5c40bc8a32c522e7884a54663d))
* finish lifecycle event logging ([d907eb5](https://github.com/chio-labs/sqlbuild/commit/d907eb551b26b55df768716a9488246552bd97a7))
* guard unit test sql length ([7af7856](https://github.com/chio-labs/sqlbuild/commit/7af7856d0bd02cafc3f1d0cd4278abb6664fcdbc))
* improve chained sql test output ([2732950](https://github.com/chio-labs/sqlbuild/commit/273295085e52d4775bd879e53f5774edf1186625))
* make seed resources explicit ([f635894](https://github.com/chio-labs/sqlbuild/commit/f635894b5ea598b6271c8aaa9dcbabde50033e69))
* move model metadata into headers ([18d5481](https://github.com/chio-labs/sqlbuild/commit/18d54813b09d22c04112a19934041fc6920e619e))
* move model metadata into headers ([dd7f980](https://github.com/chio-labs/sqlbuild/commit/dd7f9808e37feef27585341fb8862ea4aff478dc))
* move statement recording into adapter write methods ([e2c6625](https://github.com/chio-labs/sqlbuild/commit/e2c66250bcc7c1f9665ab79d8eafddce0741e194))
* redesign plan formatter with grouped output, cascade display, microbatch labels, and enum-only domain strings ([08004cc](https://github.com/chio-labs/sqlbuild/commit/08004cc13f8986d9763724586c1b170acf203920))
* refine debug diagnostics output ([306506b](https://github.com/chio-labs/sqlbuild/commit/306506bcfac4635ed8de5d0bafc944395812217c))
* resolve compile environment and vars layering ([67931de](https://github.com/chio-labs/sqlbuild/commit/67931de95e403a175af5f64aedc203f97d6772b0))
* show delta/final audit phases with batch counts and aggregate microbatch audit results ([30bfde1](https://github.com/chio-labs/sqlbuild/commit/30bfde146d30bad70e3b976312d4f485d6e7f33c))
* show executed lifecycle SQL in verbose output ([509cc03](https://github.com/chio-labs/sqlbuild/commit/509cc039bc63d797b46efbcd67a78cd2147e2e7d))
* split early run context from target templates ([9b30f31](https://github.com/chio-labs/sqlbuild/commit/9b30f31572b489e1b08e72aa7e0fa9cae0229fe6))
* support double quoted model header strings ([5212d40](https://github.com/chio-labs/sqlbuild/commit/5212d40b299f26c5834d5dfb4d980fb0e1d714e6))
* support expression-backed sources ([2d8a693](https://github.com/chio-labs/sqlbuild/commit/2d8a69355a18ea48905568704e2af417fcf7f860))
* support local connection settings ([fa900cd](https://github.com/chio-labs/sqlbuild/commit/fa900cdcb08d24d8cf8ba3d34dba494596983c7a))
* support local environment overrides ([fd84b1e](https://github.com/chio-labs/sqlbuild/commit/fd84b1eced7b8f0e8a2dd8fe27b9bad4187a477c))
* support local environment overrides ([deeb521](https://github.com/chio-labs/sqlbuild/commit/deeb521993167a472480dd8a8080e178d4835d67))
* support seed mocks in SQL tests ([ddeea1c](https://github.com/chio-labs/sqlbuild/commit/ddeea1cc327375858604475146e6d1809ea794aa))
* support sql test macro mocks ([d4b7c84](https://github.com/chio-labs/sqlbuild/commit/d4b7c84c0990db71e2ec35b29f0beead1aa70e66))
* UNFINISHED (trying to introduce lifecycle events) ([aafbdba](https://github.com/chio-labs/sqlbuild/commit/aafbdba89b0130cafb3af4f8719c782fda1c08e1))
* use environment range for diff ([7abe590](https://github.com/chio-labs/sqlbuild/commit/7abe5901418580d732b461ac3dc63ea06718c917))
* use environment range for diff ([12b76ef](https://github.com/chio-labs/sqlbuild/commit/12b76ef00f3ff2a33eb4544a072f98410928368a))
* validate discovery conflicts and seed metadata ([9dd6622](https://github.com/chio-labs/sqlbuild/commit/9dd6622abe1ffb79ce7f226066e5b2e0438e3fbf))
* validate seed csv headers in discovery ([e2357a5](https://github.com/chio-labs/sqlbuild/commit/e2357a51d3fd2c80a6e33061445648ffadd808e8))
* wire CLI commands for build, run, test, audit, and seed ([4d82460](https://github.com/chio-labs/sqlbuild/commit/4d82460bfe9a8522c57321f674bda4145b539688))
* wire custom materialization loading through compile pipeline and add waffle shop example with e2e coverage ([dad1f75](https://github.com/chio-labs/sqlbuild/commit/dad1f75d79883e397431c3e786f8de89a8abc25d))
* write runtime SQL artifacts after build and run ([293a29c](https://github.com/chio-labs/sqlbuild/commit/293a29ce3b3d351d7cee60d84416487d6f24da78))


### Bug Fixes

* align query fingerprint tracking ([a3307bc](https://github.com/chio-labs/sqlbuild/commit/a3307bc30885c0e1a43600b140b9d76ba502bb22))
* clarify audit failure messaging ([4c2a090](https://github.com/chio-labs/sqlbuild/commit/4c2a0909540111b9c67827f611b189f7419937e3))
* clarify CLI errors and DuckDB UDF fingerprints ([82921f9](https://github.com/chio-labs/sqlbuild/commit/82921f9423f975b0fd34d20601ee1a2648052d35))
* clarify CLI errors and DuckDB UDF fingerprints ([4d5801e](https://github.com/chio-labs/sqlbuild/commit/4d5801eeb1c99dc97044268dafef78b43959512a))
* coarsen mixed timestamp cursor replay ([a53901e](https://github.com/chio-labs/sqlbuild/commit/a53901eed994d2dc030c67e11b6c1848fe78b0e9))
* consolidate duplicate _get_materialization_type and add advanced custom materialization tests ([b9d3f9d](https://github.com/chio-labs/sqlbuild/commit/b9d3f9d987122484f26782060e167d0f999a1454))
* correct SKIP semantics, enforce incremental strategy, fix deferred cursor resolution ([bc7c344](https://github.com/chio-labs/sqlbuild/commit/bc7c3448b94618082ab0625bd4bd0eccd6a5de19))
* enforce cascade backfill planning ([fa18cbb](https://github.com/chio-labs/sqlbuild/commit/fa18cbbc9e03ac7ed33502d94dbb32f7e5050653))
* enforce cascade backfill planning ([214e294](https://github.com/chio-labs/sqlbuild/commit/214e29441ab972ab146475b8112de789f4eb7a29))
* handle runtime-owned cursor filters on first build ([e7b9202](https://github.com/chio-labs/sqlbuild/commit/e7b9202175cc0145735a9306237b8c041cfb4ad6))
* harden microbatch full-refresh execution ([43aa3fc](https://github.com/chio-labs/sqlbuild/commit/43aa3fca70b26fc6878f03040a5ccad52d7e4ef9))
* improve CLI output formatting and add Execution section header ([f3a67f3](https://github.com/chio-labs/sqlbuild/commit/f3a67f33864cddb4673a02b35cde9ee4a645cb49))
* improve plan/execution CLI output visual hierarchy and spacing ([eb1b0ed](https://github.com/chio-labs/sqlbuild/commit/eb1b0edcff08543328b22a7a135460870900d87f))
* improve standalone audit and test output with model grouping and consistent header styling ([c0475ae](https://github.com/chio-labs/sqlbuild/commit/c0475aec81f58e29592e5254e7c28f179512dc16))
* improve warehouse planning feedback ([70fc2a4](https://github.com/chio-labs/sqlbuild/commit/70fc2a46a0f3d2ebdd62611a87741142a1ef43a5))
* improve warehouse planning feedback ([17dd900](https://github.com/chio-labs/sqlbuild/commit/17dd90029a297d4d5df5e7518e7f22ffeb9897fb))
* indent audit and test sub-lines for clearer visual nesting under parent model ([bf2a2c7](https://github.com/chio-labs/sqlbuild/commit/bf2a2c7814fae9aa2e7a10b83964a81412e81db7))
* polish plan selectors and runtime artifacts ([f2f0633](https://github.com/chio-labs/sqlbuild/commit/f2f0633da597634a6a33863635adcc1a2f76345e))
* propagate UDF definition changes ([5e87a9c](https://github.com/chio-labs/sqlbuild/commit/5e87a9c08d105d47f44bf226f81497d7c3d77470))
* propagate UDF definition changes ([12e2125](https://github.com/chio-labs/sqlbuild/commit/12e2125296dff4cf009a1e3af86424f7e194b591))
* refine query change warning behavior ([6c2a193](https://github.com/chio-labs/sqlbuild/commit/6c2a19383ae04847fad1586cf6797d6eb4f5f736))
* refresh latest tracked partition ([9ff283e](https://github.com/chio-labs/sqlbuild/commit/9ff283e60faff82878098f690c4c7d46b883a630))
* reject unsupported dbt refs ([4717d3b](https://github.com/chio-labs/sqlbuild/commit/4717d3ba301af8079b08e04bf4b863a412b1789b))
* remove dead test helpers, deduplicate write_repo_files into shared fixture, and strengthen ast hash test ([e5daeb2](https://github.com/chio-labs/sqlbuild/commit/e5daeb245b0f1233270153cf1e0a035389940f87))
* resolve model-backed cursor inputs at runtime ([423f9ff](https://github.com/chio-labs/sqlbuild/commit/423f9ff442457e51095dbec98f1580b9c2f7cbb7))
* scope snowflake test markers ([9ee43c2](https://github.com/chio-labs/sqlbuild/commit/9ee43c2ff0b71b74df887337ff6853c3c64d3ce5))
* show incremental strategy annotation on full-refresh build output ([288d8c2](https://github.com/chio-labs/sqlbuild/commit/288d8c2788a1007fb64397cd1113f0c98b38f9f2))
* show model count in audit and test command headers ([f528515](https://github.com/chio-labs/sqlbuild/commit/f528515ba1fe0562e2ef0a662594fb2d68d4b927))
* show seed failures and default quotechar ([cabd72c](https://github.com/chio-labs/sqlbuild/commit/cabd72c6dc78320d1dfe8546633d9c1f55704bcd))
* stabilize SQL tests with Snowflake UDFs ([239fa16](https://github.com/chio-labs/sqlbuild/commit/239fa16786dc45737b15e637e824d2d74e0cbf87))
* tighten config validation ([6adc4c2](https://github.com/chio-labs/sqlbuild/commit/6adc4c23ade8cc1613701866badbb41e0c26ab6b))
* use BigQuery copy promotion ([a4ccff9](https://github.com/chio-labs/sqlbuild/commit/a4ccff94258dbd7503ecb1b4039045e74e3d8c50))
* use BigQuery copy promotion ([386deb4](https://github.com/chio-labs/sqlbuild/commit/386deb49ebbbf625f55847dac0ca63b48240ae3f))
* use BigQuery MERGE for delete insert ([a0d1283](https://github.com/chio-labs/sqlbuild/commit/a0d12838dfc8165279b83e4103939e4e648c23dd))
* use BigQuery MERGE for delete insert ([da62a51](https://github.com/chio-labs/sqlbuild/commit/da62a51af74dc93ab8c5679572b663b56039a94b))
* use dynamic column widths in build progress output for better terminal usage ([0c89ad0](https://github.com/chio-labs/sqlbuild/commit/0c89ad002a596d174c3125b66296f277e326dee0))
* validate cursor input names ([ec1a97a](https://github.com/chio-labs/sqlbuild/commit/ec1a97ab2d284eba42dbce91a6418970d9e31e48))
* validate declared inline source columns ([6969e03](https://github.com/chio-labs/sqlbuild/commit/6969e039d202bbab76fd6ed2267bf94f55bcb528))
* validate declared inline source columns ([da58065](https://github.com/chio-labs/sqlbuild/commit/da58065da0bbb6b6de5fa225772fca1f9978246e))
* validate hook sql at compile time ([6c370d1](https://github.com/chio-labs/sqlbuild/commit/6c370d1f51a5702295c5be7a3f7e59c877baee09))
* validate relation source metadata columns ([a0ed2c0](https://github.com/chio-labs/sqlbuild/commit/a0ed2c015421db8641143cb6761b70ab5c7941d2))
* wrap multi-statement DML in transactions for atomicity ([884bfbc](https://github.com/chio-labs/sqlbuild/commit/884bfbcfbf2cc7e639b1e8dc58cb960baefc567e))


### Documentation

* add project logo ([5e629c7](https://github.com/chio-labs/sqlbuild/commit/5e629c726fd637937800a0533f3959a04471e724))
* capture runtime SQL recording plan ([935e2d3](https://github.com/chio-labs/sqlbuild/commit/935e2d3cd557b9bd68fd0830d3c1fb5a802b8c36))
* enlarge README logo ([e6daafb](https://github.com/chio-labs/sqlbuild/commit/e6daafbc78cfb4306514760bd99cd0f50dfba27b))
* expand project README ([968b5f9](https://github.com/chio-labs/sqlbuild/commit/968b5f9bb905c9554af777d8a2312b67f5f00b2c))
* update feature overview ([607da90](https://github.com/chio-labs/sqlbuild/commit/607da901837f0dccbb5060f294b14d487da2408b))
* update README logo asset ([cce1d12](https://github.com/chio-labs/sqlbuild/commit/cce1d12df7289fcde8a7da42ae1f606ea206a0cd))
* update README positioning ([89fea65](https://github.com/chio-labs/sqlbuild/commit/89fea658c15dbd84450b554fed6bea163dfbbbfc))
* update README positioning ([832153e](https://github.com/chio-labs/sqlbuild/commit/832153e0bebbdbcade455c8d45a628a3f9bee923))

## Changelog

Release notes are managed by release-please.

# Event exporter runtime

Project exporters receive only validated, immutable `LifecycleEvent` values. The CLI registers the
exporter observer after canonical history and native projections, and diagnostics remain on the
separate diagnostic channel.

The dispatcher, queue, accounting and command scope are native (`sqlbuild._native`); Python keeps
only the public sink declarations and the two calls into user sink functions.

Each command owns one bounded event queue and one daemon dispatcher. Exporters run in deterministic
declaration order. An invocation runs on a daemon isolation thread so an exporter blocked in
destination code cannot keep the command's non-daemon teardown alive. The dispatcher waits at most
the invocation timeout; after a timeout that exporter is not invoked again and its later attempts
are dropped. This bounds concurrently hung threads to the number of exporters, but Python cannot
terminate those daemon threads or undo destination side effects if they eventually return.

Shutdown stops acceptance, drains only until the shared deadline (CLI commands use
`event_sinks.lifecycle.shutdown_timeout`, default two seconds), accounts queued and unattempted
deliveries as dropped, terminates the event dispatcher, and returns without waiting for timed-out
invocation threads. Concurrent shutdown callers receive the same cached final accounting snapshot.
The shared provider session is retained while any invocation remains live. Teardown runs exactly
once after the last invocation returns; a forever-hung daemon therefore retains its providers until
process exit. A provider is never torn down while live exporter code can still access it. Runtime
failures and the aggregate summary use sanitized diagnostic logging and never publish lifecycle
events.
Failure and summary callbacks run on one daemon notification worker, never on the event dispatcher
or command thread. Failures use a bounded queue and may be dropped when it is full. Periodic health
uses one replaceable slot, and the final summary uses a separate write-once retained slot, so neither
can be lost to failure-queue pressure. Shutdown returns the cached final summary without waiting for
notification callbacks; a blocked notifier remains daemonized and delivers the retained final after
it returns.

## Filters and lifecycle policy

`@lifecycle_event_sink(event_kinds=..., min_severity=...)` accepts the canonical kinds `invocation`,
`run`, `resource`, `operation`, `statement`, and `retry`, and severities `debug`, `info`, `warning`,
and `error`. Omitted declaration filters opt into all kinds from `debug` upward. Options are
validated and frozen when the function is decorated.

Project config can only narrow that opt-in. `[event_sinks.lifecycle]` supplies global `event_kinds` and
`min_severity`; `[event_sinks.lifecycle.named.<exporter_name>]` can narrow one exporter further. Effective
kinds are the intersection of declaration, global, and named sets. Effective minimum severity is
the strictest of those three values. Unknown names, kinds, and severities fail before execution.

The single authoritative lifecycle mapping is the native `lifecycle_export_policy`
(`crates/sqlbuild-core/src/event_export`), derived from the lifecycle event catalogue: failed terminal event
types are `error`; completed and skipped terminals are `info`; `retry_scheduled` is `warning`; and
starts plus `statement_submitted` are `debug`. Diagnostics are not lifecycle events and never enter
this queue.

Queue priority is finite and independent of destinations: failed terminals (3), other terminal
evidence (2), retry scheduling (1), then starts/submission (0). Dispatch takes the highest priority
and preserves FIFO among equal priorities. On overflow, a higher-priority arrival displaces the
oldest queued item at the lowest lower priority; otherwise the arrival is dropped. The queued item
carries its eligible exporter indices, so filtering, displacement, and fanout cannot create a
duplicate attempt.

## Accounting and health

For each event/exporter pair, filtering increments `filtered` exactly once. An eligible pair
increments `accepted` exactly once, then ends in exactly one of `delivered`, `failed`, or `dropped`.
Thus every final per-exporter and aggregate snapshot satisfies
`accepted == delivered + failed + dropped`; `filtered` is separate. `delivered` means only that the
user exporter callable returned normally and does not claim a remote acknowledgement or durability.

Periodic health defaults to 30 seconds and emits only when counters or queue depth changed since the
last published snapshot. Its single pending slot is overwritten with the newest snapshot while the
notifier is blocked, preventing an interval backlog. Non-final snapshots always report
`flush_complete=false`. Final diagnostic summaries include frozen aggregate and per-exporter counts,
queue depth/capacity, and `flush_complete`. A timed-out live invocation makes the bounded flush
incomplete even though its attempt is counted failed and queued/later blocked attempts are counted
dropped. Failure and health diagnostics use only exporter name, catalogued kind/severity, exception
type, counts, and queue dimensions. They bypass lifecycle publication and exporter enqueue, contain
no event payload or destination/provider details, and reporter failures remain isolated.

Startup first imports public Python modules under `python/event_sinks/` and collects decorated
declarations without discovering providers. If those modules contain only helpers, SQLBuild does
not import project providers or construct exporter queue, dispatcher, or notification threads.
Normal command-owned project discovery remains responsible for providers needed by nodes and hooks.

The project declaration, configuration, delivery, security, and failure contract is documented at
[sqlbuild.com/docs/concepts/observability/event-sinks](https://sqlbuild.com/docs/concepts/observability/event-sinks/).

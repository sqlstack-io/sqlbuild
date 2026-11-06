<script lang="ts">
	import ProblemList from '$lib/components/platform/problem-list.svelte';
	import { formatAgo, formatDuration, formatIn, formatStamp } from '$lib/format/time';
	import { app } from '$lib/state/app.svelte';
	import { createResource } from '$lib/state/resource.svelte';
	import { heartbeatLate, replicaLive, schedulerWords, sinkWords } from './utils';

	const health = createResource((client) => client.platformHealth(), ['platform_events']);
	const serverEvents = createResource((client) => client.listPlatformEventHistory({ locationId: null }), ['platform_events']);

	// Scheduler, replicas and forwarding are liveness signals; poll them while the page is open.
	$effect(() => {
		const timer = setInterval(() => health.refresh(), 15_000);
		return () => clearInterval(timer);
	});

	const h = $derived(health.data ?? null);
	const scheduler = $derived(h ? schedulerWords(h.scheduler, app.now) : null);
	const sink = $derived(h ? sinkWords(h.sink, app.now) : null);
	const late = $derived(!!h && heartbeatLate(h.heartbeat, app.now));
	const openServerProblems = $derived((serverEvents.data ?? []).filter((e) => e.active).length);
	let open = $state(false);

	export function refresh() {
		health.refresh();
		serverEvents.refresh();
	}
</script>

<section id="server" class="strip" aria-label="Server status">
	{#if health.error}
		<span class="err">Couldn't load the server status: {health.error}</span>
	{:else if !h || !scheduler || !sink}
		<span class="muted">Loading server status…</span>
	{:else}
		<span class="item" title={scheduler.title}>
			Scheduler <i style:background={scheduler.tone}></i><b>{scheduler.text}</b>
			{#if scheduler.checkIn}<span class="muted">· last check-in {scheduler.checkIn}</span>{/if}
		</span>
		<span class="sep">·</span>
		<span class="item" title={sink.title}>Event forwarding <i style:background={sink.tone}></i><b>{sink.text}</b></span>
		{#if openServerProblems}<span class="sep">·</span><span class="warn">{openServerProblems} open server problem{openServerProblems === 1 ? '' : 's'}</span>{/if}
	{/if}
	<button class="toggle" onclick={() => (open = !open)} aria-expanded={open}>{open ? 'Hide details' : 'Details'}</button>
</section>

{#if open && h}
	<div class="details">
		<div class="cols">
			<div>
				<h4>Servers</h4>
				<ul>
					{#each h.scheduler.instances as i (i.instanceId)}
						<li title="Started {formatStamp(i.startedAt)} · version {i.version}">
							<i style:background={replicaLive(i.lastSeenAt, app.now) ? 'var(--sb-success)' : 'var(--sb-error)'}></i>
							<code>{i.instanceId}</code>
							<span class="muted">
								{!i.schedulerEnabled ? 'web and API only' : h.scheduler.lease?.holder === i.instanceId ? 'running the scheduler' : 'standing by to take over'}
							</span>
							<span class="faint">seen {formatAgo(i.lastSeenAt, app.now)}</span>
						</li>
					{/each}
				</ul>
				{#if h.scheduler.lease}
					<p class="faint">
						Leader lease held by <code>{h.scheduler.lease.holder}</code> for {formatDuration(h.scheduler.lease.acquiredAt, null, app.now)}; renewed {formatAgo(h.scheduler.lease.renewedAt, app.now)}, expires {formatIn(h.scheduler.lease.expiresAt, app.now)}.
					</p>
				{/if}
			</div>
			<div>
				<h4>Heartbeat</h4>
				<p class:warn={late}>
					{h.heartbeat.lastEmittedAt ? `Sent ${formatAgo(h.heartbeat.lastEmittedAt, app.now)}` : 'Never sent'}{late ? ' — late' : ''}
				</p>
				<p class="faint">The server sends a heartbeat event every {h.heartbeat.intervalSeconds}s. If it stops arriving where you forward events, the server is down.</p>
			</div>
			<div>
				<h4>Event forwarding</h4>
				{#if h.sink}
					<p>Sends problems as they happen to a {h.sink.kind} set in the server config (<code>{h.sink.destinationSource}</code>).</p>
					<p class="faint">
						{h.sink.eventKinds.length ? h.sink.eventKinds.join(', ') : 'Every kind of event'} · {h.sink.pendingEvents} waiting · last tried {formatAgo(h.sink.lastAttemptAt, app.now)}
					</p>
					{#if h.sink.lastError}<p class="err">{h.sink.lastError}</p>{/if}
				{:else}
					<p class="faint">Not set up. Sensors still see every problem; forwarding pushes them out without code.</p>
				{/if}
			</div>
		</div>
		<h4 class="mt">Server problems</h4>
		<ProblemList events={serverEvents.data ?? []} resource={serverEvents} empty="No server problems recorded." />
	</div>
{/if}

<style>
	.strip {
		display: flex;
		flex: 0 0 auto;
		align-items: center;
		gap: 10px;
		padding: 10px 20px;
		border-bottom: 1px solid var(--border);
		background: var(--sb-surface-low);
		font-size: 13px;
	}
	.item {
		display: inline-flex;
		align-items: center;
		gap: 6px;
	}
	.strip i,
	li i {
		display: inline-block;
		width: 8px;
		height: 8px;
		border-radius: 50%;
	}
	b {
		font-weight: 500;
	}
	.sep {
		color: var(--sb-text-faint);
	}
	.toggle {
		margin-left: auto;
		color: var(--primary);
		font-size: 12.5px;
	}
	.details {
		flex: 0 0 auto;
		max-height: 55vh;
		overflow: auto;
		padding: 12px 20px;
		border-bottom: 1px solid var(--border);
		font-size: 12.5px;
	}
	.cols {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
		gap: 20px;
	}
	h4 {
		margin-bottom: 6px;
		color: var(--sb-text-faint);
		font-size: 12px;
		font-weight: 500;
	}
	h4.mt {
		margin-top: 16px;
	}
	ul {
		display: flex;
		flex-direction: column;
		gap: 3px;
	}
	li {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	li .faint {
		margin-left: auto;
	}
	p {
		margin-bottom: 4px;
	}
	code {
		font-family: var(--font-mono);
		font-size: 12px;
	}
	.muted {
		color: var(--muted-foreground);
	}
	.faint {
		color: var(--sb-text-faint);
	}
	.warn {
		color: var(--sb-warning);
	}
	.err {
		color: var(--sb-error);
	}
</style>

<script lang="ts">
	import type { NodeBuild } from '$lib/api';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { app } from '$lib/state/app.svelte';
	import { shortRunId } from '$lib/build-area/labels';

	// Where a build came from: a link to the scheduler run, or, for builds made
	// outside the scheduler (CLI, CI), the fingerprint's run ID and time.
	let { build, time = true }: { build: Pick<NodeBuild, 'runId' | 'fingerprintRunId' | 'builtAt'> | null; time?: boolean } = $props();
</script>

{#if !build}
	<span class="muted">Never built in this target</span>
{:else if build.runId}
	<a class="run" href="/runs/{build.runId}">{build.runId}</a>{#if time}<span class="muted" title={formatStamp(build.builtAt)}>{` · ${formatAgo(build.builtAt, app.now)}`}</span>{/if}
{:else}
	<span class="muted" title="Built by sqb outside this server, e.g. from a laptop or CI · fingerprint run {build.fingerprintRunId} · {formatStamp(build.builtAt)}">From the command line · <code>{shortRunId(build.fingerprintRunId)}</code>{#if time} · {formatAgo(build.builtAt, app.now)}{/if}</span>
{/if}

<style>
	.run {
		color: var(--primary);
		font: 12px var(--font-mono);
	}
	.muted {
		color: var(--muted-foreground);
	}
	code {
		font: 12px var(--font-mono);
	}
</style>

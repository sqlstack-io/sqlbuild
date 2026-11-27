<script lang="ts">
	import type { ConnectionTest, Scope } from '$lib/api';
	import StatusLabel from '$lib/components/list/status-label.svelte';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { actionTone, errorMessage, testOrigin, testWords, type ActionWords } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import ActionStatus from './action-status.svelte';
	import GatedButton from './gated-button.svelte';
	import ViewError from './view-error.svelte';

	/**
	 * `compact` shows one line, e.g. "Warehouse ● connected 54s ago [Test connection]", with the message in the tooltip.
	 * `showLast` false shows only a test run here, for a page that shows the last recorded check itself.
	 * `deployedAt` makes a test older than the latest deploy read "not tested since deploy".
	 */
	type Props = { scope: Scope; connection: string; last: ConnectionTest | null; compact?: boolean; showLast?: boolean; deployedAt?: string | null; problemsHref?: string | null; ontested?: () => void };
	let { scope, connection, last, compact = false, showLast = true, deployedAt = null, problemsHref = null, ontested }: Props = $props();

	let testing = $state<number | null>(null);
	let result = $state<ConnectionTest | null>(null);
	let failure = $state<{ at: number; cause: string } | null>(null);
	const shown = $derived(result ?? (showLast ? last : null));
	const words = $derived.by((): ActionWords => {
		if (testing !== null) return { tone: 'running', label: `testing · ${Math.max(0, Math.round((app.now - testing) / 1000))}s`, title: 'Runs the same check as sqb debug.' };
		if (failure) return { tone: 'failed', label: `couldn’t test · ${formatAgo(new Date(failure.at).toISOString(), Math.max(app.now, failure.at))}`, title: failure.cause };
		return testWords(shown, deployedAt, Math.max(app.now, shown ? Date.parse(shown.at) : 0));
	});
	// A failed click shows its error behind View error; a failed test is explained in full where `problemsHref` points.
	const cause = $derived(testing === null ? (failure?.cause ?? (shown?.outcome === 'failed' && !problemsHref ? shown.message : null)) : null);
	const href = $derived(testing === null && !failure && shown?.outcome === 'failed' ? problemsHref : null);

	async function test() {
		testing = Date.now();
		failure = null;
		try {
			result = await app.client.testConnection(scope, connection);
			ontested?.();
		} catch (error) {
			failure = { at: Date.now(), cause: errorMessage(error) };
		} finally {
			testing = null;
		}
	}
</script>

{#if compact}
	<ActionStatus
		lead="Warehouse"
		{words}
		{cause}
		{href}
		testid="connection-test"
		action={{ label: 'Test connection', requires: 'connection.test', onclick: test, busy: testing !== null, busyTitle: testing !== null ? `Testing since ${formatStamp(new Date(testing).toISOString())}` : undefined, title: 'Ask the location to connect to its warehouse, as sqb debug does' }}
	/>
{:else}
	<div class="connection-test">
		<div class="min-w-0 flex-1">
			{#if testing !== null || failure || shown || showLast}
				<span title={words.title}><StatusLabel tone={actionTone[words.tone]} label="{words.label}{shown && testing === null && !failure ? ` ${testOrigin(shown)}` : ''}" muted={words.tone === 'never'} /></span>
			{/if}
			{#if shown && testing === null && !failure}
				{#if shown.outcome === 'failed'}<div class="error-line"><ViewError error={shown.message} /></div>{:else}<div class="message">{shown.message}</div>{/if}
			{/if}
			{#if failure}<div class="error-line"><ViewError error={failure.cause} /></div>{/if}
		</div>
		<span title={testing !== null ? `Testing since ${formatStamp(new Date(testing).toISOString())}` : undefined}>
			<GatedButton requires="connection.test" busy={testing !== null} onclick={test} title="Ask the location to connect to its warehouse, as sqb debug does">Test connection</GatedButton>
		</span>
	</div>
{/if}

<style>
	.connection-test {
		display: flex;
		align-items: center;
		gap: 12px;
		min-width: 0;
	}
	.error-line {
		margin-top: 2px;
	}
	.message {
		margin-top: 2px;
		overflow: hidden;
		color: var(--muted-foreground);
		font-size: 12px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>

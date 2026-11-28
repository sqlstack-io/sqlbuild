<script lang="ts">
	import type { RepublishResult, Scope } from '$lib/api';
	import { formatAgo, formatStamp } from '$lib/format/time';
	import { errorMessage, publishWords, type ActionWords } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';
	import ActionStatus from './action-status.svelte';

	/** "Deploy ● version 7 · 2h ago [Republish]"; publishing runs the discover functions. */
	type Props = { scope: Scope; snapshot?: { generation: number; publishedAt: string } | null; ondone?: () => void };
	let { scope, snapshot = null, ondone }: Props = $props();

	let publishing = $state<number | null>(null);
	let result = $state<RepublishResult | null>(null);
	let failure = $state<{ at: number; cause: string } | null>(null);
	let done = $state<number | null>(null);
	const words = $derived.by((): ActionWords => {
		if (failure && publishing === null) return { tone: 'failed', label: 'couldn’t republish', title: failure.cause };
		if (result?.state === 'published' && done !== null && publishing === null) {
			const version = result.generation ?? snapshot?.generation;
			return { tone: 'ok', label: `published${version ? ` version ${version}` : ''} · ${formatAgo(new Date(done).toISOString(), Math.max(app.now, done))}`, title: 'Compiled and published again by you; the discover functions ran.' };
		}
		return publishWords(result, publishing, snapshot, app.now);
	});
	const cause = $derived(publishing !== null ? null : (failure?.cause ?? (result?.state === 'failed' ? result.error : null)));

	async function republish() {
		publishing = Date.now();
		result = null;
		failure = null;
		try {
			result = await app.client.republish(scope);
			done = Date.now();
			ondone?.();
		} catch (error) {
			failure = { at: Date.now(), cause: errorMessage(error) };
		} finally {
			publishing = null;
		}
	}
</script>

<ActionStatus
	lead="Deploy"
	{words}
	{cause}
	testid="republish"
	action={{
		label: 'Republish',
		requires: 'location.republish',
		onclick: republish,
		busy: publishing !== null,
		busyTitle: publishing !== null ? `Publishing since ${formatStamp(new Date(publishing).toISOString())}` : undefined,
		title: 'Compile and publish again, running the discover functions'
	}}
/>

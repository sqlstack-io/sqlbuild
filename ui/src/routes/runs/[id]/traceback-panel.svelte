<script lang="ts">
	import Button from '$lib/components/ui/button/button.svelte';
	import { to } from '$lib/paths';

	// A Python node's exception, in place (CHI-1591): the exception and the last part of its traceback,
	// with links from the node's own frames to its code. The full context is the output viewer, opened
	// at the error line.
	let {
		detail,
		file,
		runId,
		nodeKey,
		onframe
	}: {
		/** The whole traceback, as the node printed it. */
		detail: string;
		/** The node's own file, such as `models/ml/score_customers_v2.py`: its frames link to its code. */
		file: string | null;
		runId: string;
		nodeKey: string;
		/** A frame in the node's own code: show that line in the drawer. */
		onframe?: (line: number) => void;
	} = $props();

	const TAIL = 15;
	const lines = $derived(detail.split('\n'));
	/** The exception: the last line naming one, such as `KeyError: 'customer_segment'`. */
	const exception = $derived([...lines].reverse().find((l) => /^[A-Za-z_.]+(Error|Exception|Warning)\b/.test(l)) ?? lines[lines.length - 1]);
	const [kind, ...rest] = $derived(exception.split(': '));
	let all = $state(false);
	const shown = $derived(all ? lines : lines.slice(-TAIL));
	/** A frame line in the node's own file, as its line number. */
	function ownLine(text: string): number | null {
		if (!file) return null;
		const m = text.match(/File "(?:\/app\/)?([^"]+)", line (\d+)/);
		return m && m[1] === file ? Number(m[2]) : null;
	}
</script>

<div class="tb">
	<div class="exc"><b>{kind}</b>{#if rest.length}: <span class="mono">{rest.join(': ')}</span>{/if}</div>
	<pre>{#if !all && lines.length > TAIL}<span class="faint">… {lines.length - TAIL} earlier lines</span>
{/if}{#each shown as l, i (i)}{@const own = ownLine(l)}{#if own != null && onframe}<span>{l.slice(0, l.indexOf('File "'))}File "</span><button class="frame" onclick={() => onframe(own)} title="Show line {own} of {file}">{file}", line {own}</button><span>{l.slice(l.indexOf(`line ${own}`) + `line ${own}`.length)}</span>{:else}<span class:last={l === exception}>{l}</span>{/if}
{/each}</pre>
	<div class="acts">
		{#if lines.length > TAIL}<Button variant="link" size="inline" class="text-[12px]" onclick={() => (all = !all)}>{all ? 'Show the last lines' : `Show all ${lines.length} lines`}</Button>{/if}
		<a href={to(`/runs/${runId}/output/${nodeKey}?at=error`)} title="The node's whole output, at the error line">Open in output →</a>
	</div>
</div>

<style>
	.tb {
		display: flex;
		min-width: 0;
		flex-direction: column;
		gap: 6px;
		padding: 6px 10px;
		border-left: 2px solid color-mix(in srgb, var(--sb-error) 60%, transparent);
		font-size: 12px;
	}
	.exc b {
		color: var(--sb-error);
		font-weight: 600;
	}
	.mono,
	pre {
		font-family: var(--font-mono);
		font-size: 12px;
	}
	pre {
		margin: 0;
		color: var(--muted-foreground);
		white-space: pre-wrap;
		word-break: break-word;
	}
	pre .last {
		color: var(--sb-error);
	}
	.frame {
		color: var(--primary);
		text-decoration: underline;
		text-decoration-color: color-mix(in srgb, var(--primary) 40%, transparent);
	}
	.faint {
		color: var(--sb-text-faint);
	}
	.acts {
		display: flex;
		align-items: baseline;
		gap: 16px;
	}
	.acts a {
		color: var(--primary);
	}
</style>

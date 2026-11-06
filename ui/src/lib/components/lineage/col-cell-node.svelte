<script lang="ts">
	import { Handle, Position, type NodeProps } from '@xyflow/svelte';
	import { Table2, Eye, Layers, Database } from '@lucide/svelte';

	type Lens = 'raw' | 'passthrough' | 'rename' | 'transform';
	type Data = {
		model: string;
		column: string;
		kind: string;
		status: 'ok' | 'fail' | 'warn' | 'stale' | 'src';
		lens?: Lens;
		lensLabel?: Record<Lens, string>;
		focal?: boolean;
		active?: boolean;
	};

	let { id, data }: NodeProps = $props();
	const d = $derived(data as Data);

	const statusColor: Record<string, string> = {
		ok: 'var(--sb-success)',
		fail: 'var(--sb-error)',
		warn: 'var(--sb-warning)',
		stale: 'var(--sb-stale)',
		src: 'var(--sb-text-faint)'
	};
	const KindIcon = $derived(d.kind === 'view' ? Eye : d.kind.startsWith('source') ? Database : d.kind.startsWith('incremental') ? Layers : Table2);
	const kindColor = $derived(d.kind === 'view' ? '#5aa9e6' : d.kind === 'table' ? '#2e90ff' : d.kind.startsWith('incremental') ? '#3fb6c9' : '#7c8aa5');
</script>

<div
	class="overflow-hidden rounded-[2px] border bg-[var(--card)]"
	data-colid={id}
	style:width="190px"
	style:border-color={d.focal || d.active ? 'var(--primary)' : 'var(--node-border)'}
	style:box-shadow={d.focal ? '0 0 0 1px var(--primary)' : 'none'}
>
	<Handle type="target" position={Position.Left} style="width:7px;height:7px;background:var(--sb-edge);border:none;" />
	<!-- column (primary) -->
	<div class="flex items-center gap-2 px-3 py-2" style:border-left="3px solid {statusColor[d.status]}">
		<span class="truncate font-mono text-[12.5px] font-medium text-foreground">{d.column}</span>
	</div>
	<!-- model (context) -->
	<div class="flex items-center gap-1.5 border-t border-[var(--border-subtle)] px-3 py-1.5">
		<KindIcon size={11} color={kindColor} />
		<span class="text-muted-foreground truncate text-[12px]">{d.model}</span>
		{#if d.lens && d.lensLabel}
			<span class="ml-auto shrink-0 text-[11.5px]" class:font-semibold={d.lens === 'transform'} style:color={d.lens === 'transform' ? 'var(--foreground)' : 'var(--sb-text-faint)'}>{d.lensLabel[d.lens]}</span>
		{:else}
			<span class="text-[var(--sb-text-faint)] ml-auto text-[11px]">{d.kind}</span>
		{/if}
	</div>
	<Handle type="source" position={Position.Right} style="width:7px;height:7px;background:var(--sb-edge);border:none;" />
</div>

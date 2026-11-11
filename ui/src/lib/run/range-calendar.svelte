<script lang="ts">
	// A calendar beside the typed dates: pick the first and last day, both included. Dates are UTC days.
	import { parseDate, type DateValue } from '@internationalized/date';
	import { RangeCalendar } from 'bits-ui';
	import CalendarIcon from '@lucide/svelte/icons/calendar';
	import ChevronLeftIcon from '@lucide/svelte/icons/chevron-left';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import * as Popover from '$lib/components/ui/popover';

	let { start, end, today, onpick }: { start: string; end: string; today: string; onpick: (start: string, end: string) => void } = $props();

	const day = (text: string): DateValue | undefined => {
		try {
			return text ? parseDate(text) : undefined;
		} catch {
			return undefined;
		}
	};
	let open = $state(false);
	let value = $state<{ start: DateValue | undefined; end: DateValue | undefined }>({ start: undefined, end: undefined });
	// Opening starts from the typed range, so the calendar and the fields never disagree.
	$effect(() => {
		if (open) value = { start: day(start), end: day(end) };
	});
</script>

<Popover.Root bind:open>
	<Popover.Trigger class="range-calendar-trigger" aria-label="Pick dates on a calendar" title="Pick dates on a calendar"><CalendarIcon size={13} /></Popover.Trigger>
	<Popover.Content class="w-auto p-3" align="start">
		<RangeCalendar.Root
			class="calendar"
			bind:value
			weekStartsOn={1}
			weekdayFormat="short"
			fixedWeeks
			maxValue={day(today)}
			placeholder={day(start) ?? day(today)}
			onValueChange={(v) => {
				if (!v.start || !v.end) return;
				onpick(v.start.toString(), v.end.toString());
				open = false;
			}}
		>
			{#snippet children({ months, weekdays })}
				<RangeCalendar.Header class="header">
					<RangeCalendar.PrevButton class="nav"><ChevronLeftIcon size={14} /></RangeCalendar.PrevButton>
					<RangeCalendar.Heading class="heading" />
					<RangeCalendar.NextButton class="nav"><ChevronRightIcon size={14} /></RangeCalendar.NextButton>
				</RangeCalendar.Header>
				{#each months as month (month.value.toString())}
					<RangeCalendar.Grid class="grid">
						<RangeCalendar.GridHead>
							<RangeCalendar.GridRow class="row">
								{#each weekdays as weekday (weekday)}<RangeCalendar.HeadCell class="weekday">{weekday.slice(0, 2)}</RangeCalendar.HeadCell>{/each}
							</RangeCalendar.GridRow>
						</RangeCalendar.GridHead>
						<RangeCalendar.GridBody>
							{#each month.weeks as week (week[0].toString())}
								<RangeCalendar.GridRow class="row">
									{#each week as date (date.toString())}
										<RangeCalendar.Cell {date} month={month.value} class="cell"><RangeCalendar.Day class="day" /></RangeCalendar.Cell>
									{/each}
								</RangeCalendar.GridRow>
							{/each}
						</RangeCalendar.GridBody>
					</RangeCalendar.Grid>
				{/each}
				<p class="hint">{value.start && !value.end ? 'Pick the last day' : 'Pick the first day, then the last'}</p>
			{/snippet}
		</RangeCalendar.Root>
	</Popover.Content>
</Popover.Root>

<style>
	:global(.range-calendar-trigger) {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 22px;
		border: 1px solid var(--border);
		border-radius: 2px;
		color: var(--muted-foreground);
	}
	:global(.range-calendar-trigger:hover) {
		color: var(--foreground);
	}
	:global(.calendar .header) {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 8px;
	}
	:global(.calendar .heading) {
		font-size: 12.5px;
		font-weight: 600;
	}
	:global(.calendar .nav) {
		display: inline-flex;
		padding: 4px;
		border-radius: 2px;
		color: var(--muted-foreground);
	}
	:global(.calendar .nav:hover) {
		background: var(--sb-hover);
		color: var(--foreground);
	}
	:global(.calendar .nav[data-disabled]) {
		opacity: 0.35;
	}
	:global(.calendar .grid) {
		border-collapse: collapse;
	}
	:global(.calendar .weekday) {
		width: 32px;
		padding-bottom: 4px;
		color: var(--sb-text-faint);
		font-size: 10.5px;
		font-weight: 600;
		text-transform: uppercase;
	}
	:global(.calendar .cell) {
		padding: 1px 0;
	}
	:global(.calendar .day) {
		display: flex;
		width: 32px;
		height: 28px;
		align-items: center;
		justify-content: center;
		color: var(--foreground);
		font-size: 12px;
		font-variant-numeric: tabular-nums;
	}
	:global(.calendar .day:hover:not([data-disabled])) {
		background: var(--sb-hover);
	}
	:global(.calendar .day[data-outside-month]) {
		visibility: hidden;
	}
	:global(.calendar .day[data-disabled]) {
		color: var(--sb-text-faint);
		opacity: 0.5;
	}
	:global(.calendar .day[data-today]) {
		text-decoration: underline;
		text-underline-offset: 3px;
	}
	:global(.calendar .day[data-range-middle]),
	:global(.calendar .day[data-highlighted]) {
		background: color-mix(in srgb, var(--primary) 18%, transparent);
	}
	:global(.calendar .day[data-selection-start]),
	:global(.calendar .day[data-selection-end]) {
		background: var(--primary);
		color: var(--primary-foreground);
		font-weight: 600;
	}
	:global(.calendar .hint) {
		margin: 8px 0 0;
		color: var(--sb-text-faint);
		font-size: 11.5px;
	}
</style>

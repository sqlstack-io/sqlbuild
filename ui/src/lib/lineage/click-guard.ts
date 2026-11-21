// PROTOTYPE: a header that both drags its box and toggles it on click. A press that moves further
// than a few pixels is a drag, and the click that follows it is ignored. Pure apart from its own memory.

const SLOP = 4;

/** `down` records where a press starts; `still(event)` says whether the click ending it never moved. */
export function clickGuard() {
	let start: { x: number; y: number } | null = null;
	return {
		down(event: PointerEvent) {
			start = { x: event.clientX, y: event.clientY };
		},
		still(event: MouseEvent): boolean {
			const from = start;
			start = null;
			return !from || Math.hypot(event.clientX - from.x, event.clientY - from.y) <= SLOP;
		}
	};
}

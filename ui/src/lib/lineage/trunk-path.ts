// Edge paths that merge into one trunk shortly before their target: every edge into a card arrives at
// the same point, level, a short way before it, and runs straight from there, so many waits into one
// card read as one line rather than a fan of separate curves. Pure.

/** How far before its target an edge joins the trunk. */
export const TRUNK = 28;

/**
 * An SVG path from the source to the target, curving to the trunk's start (`TRUNK` before the target,
 * at its height, arriving level) and then straight in. When the target is too close or behind the
 * source for a trunk, null: draw an ordinary curve instead.
 */
export function trunkPath(sourceX: number, sourceY: number, targetX: number, targetY: number): string | null {
	const joinX = targetX - TRUNK;
	if (joinX - sourceX < TRUNK) return null;
	const bend = (joinX - sourceX) / 2;
	return `M${sourceX},${sourceY} C${sourceX + bend},${sourceY} ${joinX - bend},${targetY} ${joinX},${targetY} L${targetX},${targetY}`;
}

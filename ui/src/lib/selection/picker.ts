// A chip's value picker. Ticking more values of the chip's type means "any of them": the token is
// repeated with each value, which `sqb` unions, so `+tag:a,path:x` with `b` ticked too becomes
// `+tag:a,path:x +tag:b,path:x`, exactly (a ∪ b) ∩ x. Unticking removes that copy.

import { parsePart, parseToken, partText } from './syntax';

/** Tokens that differ from token `at` only in the value of part `part`, with that value; the token itself included. */
export function siblings(tokens: readonly string[], at: number, part: number): { token: number; value: string }[] {
	const own = parseToken(tokens[at]).parts;
	const p = own[part];
	const out: { token: number; value: string }[] = [];
	tokens.forEach((raw, i) => {
		const parts = parseToken(raw).parts;
		if (parts.length !== own.length) return;
		const q = parts[part];
		if (q.type !== p.type || q.up !== p.up || q.down !== p.down || !!q.between !== !!p.between) return;
		if (parts.every((o, j) => j === part || o.raw === own[j].raw)) out.push({ token: i, value: q.value });
	});
	return out;
}

/** The values ticked in the picker of part `part` of token `at`. */
export const pickedValues = (tokens: readonly string[], at: number, part: number) => siblings(tokens, at, part).map((s) => s.value);

/** Ticks or unticks `value` in that picker: adds a copy of the token after its last sibling, or removes the copy holding it. */
export function toggleValueIn(tokens: readonly string[], at: number, part: number, value: string): string[] {
	const sibs = siblings(tokens, at, part);
	const held = sibs.find((s) => s.value === value);
	if (held) return tokens.filter((_, i) => i !== held.token);
	const parts = tokens[at].split(',');
	parts[part] = partText({ ...parsePart(parts[part]), value });
	const after = Math.max(...sibs.map((s) => s.token));
	return [...tokens.slice(0, after + 1), parts.join(','), ...tokens.slice(after + 1)];
}

/** The picker of part `part` of token `at` set to exactly `values`: copies added for new values, removed for dropped ones. Every copy gone removes the chip. */
export function setValuesIn(tokens: readonly string[], at: number, part: number, values: readonly string[]): string[] {
	let out = [...tokens];
	let anchor = at;
	for (const v of values) if (!pickedValues(out, anchor, part).includes(v)) out = toggleValueIn(out, anchor, part, v);
	for (const v of pickedValues(out, anchor, part)) {
		if (values.includes(v)) continue;
		const sibs = siblings(out, anchor, part);
		const gone = sibs.find((s) => s.value === v)!.token;
		const next = sibs.find((s) => s.value !== v)?.token;
		out = out.filter((_, i) => i !== gone);
		if (next === undefined) break;
		anchor = next > gone ? next - 1 : next;
	}
	return out;
}

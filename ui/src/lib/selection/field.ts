// The selection field's editing model. The text is the source of truth: finished tokens are drawn
// as chips, and the token being typed is its finished comma parts (chips in an "and" group) plus
// the text in the input. Every edit is a pure function from one state to the next.

import { joinTokens, parsePart, partText } from './syntax';

export type FieldState = {
	/** Finished tokens, drawn as chips. */
	tokens: string[];
	/** Finished comma parts of the token being typed. */
	parts: string[];
	/** The text in the input: the part being typed. */
	text: string;
};

export const EMPTY: FieldState = { tokens: [], parts: [], text: '' };

/** A pasted or linked selection: every token finished. */
export function fieldOf(selection: string): FieldState {
	return { tokens: selection.split(/\s+/).filter(Boolean), parts: [], text: '' };
}

/** The token being typed, or `''`. */
export const draftToken = (s: FieldState) => [...s.parts, s.text.trim()].filter(Boolean).join(',');

/** The finished selection: what is evaluated, copied and built. */
export const finished = (s: FieldState) => joinTokens(s.tokens);

/** Everything in the field, the token being typed included. */
export const fieldText = (s: FieldState) => joinTokens([...s.tokens, draftToken(s)]);

/** Finishes the token being typed (space, Tab, Enter or leaving the field). Text holding spaces, such as a paste, becomes several tokens. */
export function commit(s: FieldState): FieldState {
	const draft = draftToken(s);
	return draft ? { tokens: [...s.tokens, ...draft.split(/\s+/).filter(Boolean)], parts: [], text: '' } : { ...s, parts: [], text: '' };
}

/** A comma: finishes the part being typed, or with nothing typed reopens the last chip so the next part joins it. */
export function comma(s: FieldState): FieldState {
	if (s.text.trim()) return { ...s, parts: [...s.parts, s.text.trim()], text: '' };
	if (!s.parts.length && s.tokens.length) return { tokens: s.tokens.slice(0, -1), parts: s.tokens[s.tokens.length - 1].split(','), text: '' };
	return s;
}

/** Backspace with the input empty: the chip before the caret turns back into its exact text. */
export function backspace(s: FieldState): FieldState {
	if (s.text) return s;
	if (s.parts.length) return { ...s, parts: s.parts.slice(0, -1), text: s.parts[s.parts.length - 1] };
	if (!s.tokens.length) return s;
	const parts = s.tokens[s.tokens.length - 1].split(',');
	return { tokens: s.tokens.slice(0, -1), parts: parts.slice(0, -1), text: parts[parts.length - 1] };
}

/** A chosen suggestion: `complete` finishes its token as a chip; otherwise it stays text to go on typing (such as `tag:`). */
export function choose(s: FieldState, part: string, complete: boolean): FieldState {
	const next = { ...s, text: part };
	return complete ? commit(next) : next;
}

/** The finished tokens with one part replaced; an empty replacement removes the part, and a token left with no part. */
export function replacePart(s: FieldState, token: number, part: number, text: string): FieldState {
	const parts = s.tokens[token].split(',');
	if (text) parts[part] = text;
	else parts.splice(part, 1);
	const tokens = [...s.tokens];
	if (parts.length) tokens[token] = parts.join(',');
	else tokens.splice(token, 1);
	return { ...s, tokens };
}

/** A finished token replaced whole, such as by a "Did you mean" fix. */
export function replaceToken(s: FieldState, token: number, text: string): FieldState {
	const tokens = [...s.tokens];
	tokens.splice(token, 1, ...text.split(/\s+/).filter(Boolean));
	return { ...s, tokens };
}

/** Turns upstream or downstream on or off for one part. */
export function toggleExpansion(s: FieldState, token: number, part: number, side: 'up' | 'down'): FieldState {
	const p = parsePart(s.tokens[token].split(',')[part]);
	return replacePart(s, token, part, partText({ ...p, [side]: !p[side] }));
}

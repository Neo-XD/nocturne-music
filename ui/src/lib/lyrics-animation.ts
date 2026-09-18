export const APPLE_V2_FINAL_LINE_FALLBACK_MS = 4_000;

interface TimedWordLike {
	text: string;
	start_ms: number;
	end_ms: number;
}

interface LyricLineLike {
	time_ms?: number;
	end_time_ms?: number;
	text: string;
	words?: TimedWordLike[];
}

export interface AppleV2Grapheme {
	text: string;
	startMs: number;
	endMs: number;
	timed: boolean;
}

export interface AppleV2LineTiming {
	text: string;
	startMs: number;
	endMs: number;
	source: 'real' | 'estimated';
	graphemes: AppleV2Grapheme[];
}

interface GraphemePart {
	text: string;
	index: number;
}

interface SegmenterLike {
	segment(input: string): Iterable<{ segment: string; index: number }>;
}

type SegmenterConstructor = new (
	locales?: string | string[],
	options?: { granularity: 'grapheme' }
) => SegmenterLike;

const Segmenter = (Intl as typeof Intl & { Segmenter?: SegmenterConstructor }).Segmenter;
const graphemeSegmenter = Segmenter
	? new Segmenter(undefined, { granularity: 'grapheme' })
	: undefined;

function graphemeParts(text: string): GraphemePart[] {
	if (graphemeSegmenter) {
		return Array.from(graphemeSegmenter.segment(text), ({ segment, index }) => ({
			text: segment,
			index
		}));
	}

	let index = 0;
	return Array.from(text, (part) => {
		const result = { text: part, index };
		index += part.length;
		return result;
	});
}

export function segmentGraphemes(text: string): string[] {
	return graphemeParts(text).map((part) => part.text);
}

function meaningfulText(text: string): boolean {
	return /\S/u.test(text);
}

function hasUsableInterval(word: TimedWordLike): boolean {
	return meaningfulText(word.text) && word.end_ms > word.start_ms;
}

function nextTimedLineStart(lines: LyricLineLike[], index: number, startMs: number): number | undefined {
	for (let nextIndex = index + 1; nextIndex < lines.length; nextIndex += 1) {
		const nextStart = lines[nextIndex].time_ms;
		if (nextStart !== undefined && nextStart > startMs) return nextStart;
	}
	return undefined;
}

function resolveLineInterval(
	lines: LyricLineLike[],
	index: number
): { startMs: number; endMs: number } | undefined {
	const line = lines[index];
	const startMs = line.time_ms;
	if (startMs === undefined) return undefined;

	if (line.end_time_ms !== undefined && line.end_time_ms > startMs) {
		return { startMs, endMs: line.end_time_ms };
	}

	const nextStart = nextTimedLineStart(lines, index, startMs);
	if (nextStart !== undefined) return { startMs, endMs: nextStart };

	// This is the final timed line. Keep its visual-only tail short and deterministic.
	return { startMs, endMs: startMs + APPLE_V2_FINAL_LINE_FALLBACK_MS };
}

function estimatedGraphemes(
	text: string,
	startMs: number,
	endMs: number
): AppleV2Grapheme[] {
	const parts = graphemeParts(text);
	if (parts.length === 0) return [];
	const duration = Math.max(0, endMs - startMs);

	return parts.map((part, index) => ({
		text: part.text,
		startMs: startMs + (duration * index) / parts.length,
		endMs: startMs + (duration * (index + 1)) / parts.length,
		timed: true
	}));
}

function findWordRange(text: string, wordText: string, fromIndex: number) {
	let matchedText = wordText;
	let index = text.indexOf(matchedText, fromIndex);
	if (index < 0) {
		matchedText = wordText.trim();
		if (!matchedText) return undefined;
		index = text.indexOf(matchedText, fromIndex);
	}
	return index >= 0 ? { start: index, end: index + matchedText.length } : undefined;
}

function realGraphemes(
	text: string,
	words: TimedWordLike[],
	lineEndMs: number
): AppleV2Grapheme[] {
	const parts = graphemeParts(text);
	const graphemes = parts.map((part) => ({
		text: part.text,
		startMs: lineEndMs,
		endMs: lineEndMs,
		timed: false
	}));
	let searchFrom = 0;

	for (const word of words) {
		if (!meaningfulText(word.text)) continue;
		const range = findWordRange(text, word.text, searchFrom);
		if (!range) continue;
		searchFrom = range.end;
		const matched = parts
			.map((part, index) => ({ part, index }))
			.filter(({ part }) => part.index >= range.start && part.index < range.end);
		if (matched.length === 0) continue;

		if (hasUsableInterval(word)) {
			const duration = word.end_ms - word.start_ms;
			for (let index = 0; index < matched.length; index += 1) {
				const target = graphemes[matched[index].index];
				target.startMs = word.start_ms + (duration * index) / matched.length;
				target.endMs = word.start_ms + (duration * (index + 1)) / matched.length;
				target.timed = true;
			}
		} else {
			// Preserve an unclosed provider timestamp as an instantaneous boundary; do not estimate it.
			for (const { index } of matched) {
				graphemes[index].startMs = word.start_ms;
				graphemes[index].endMs = word.start_ms;
			}
		}
	}

	return graphemes;
}

export function buildAppleV2LineTiming(
	lines: LyricLineLike[],
	index: number,
	synced: boolean
): AppleV2LineTiming | undefined {
	if (!synced) return undefined;
	const line = lines[index];
	if (!line) return undefined;
	const realWords = line.words?.filter(hasUsableInterval) ?? [];
	const interval = resolveLineInterval(lines, index);

	if (realWords.length > 0) {
		const text = line.text || line.words?.map((word) => word.text).join('') || '';
		if (!text) return undefined;
		const startMs = line.time_ms ?? Math.min(...realWords.map((word) => word.start_ms));
		const endMs =
			interval?.endMs ?? Math.max(...realWords.map((word) => word.end_ms), startMs);
		return {
			text,
			startMs,
			endMs,
			source: 'real',
			graphemes: realGraphemes(text, line.words ?? realWords, endMs)
		};
	}

	if (!interval || !line.text) return undefined;
	return {
		text: line.text,
		...interval,
		source: 'estimated',
		graphemes: estimatedGraphemes(line.text, interval.startMs, interval.endMs)
	};
}

export function appleV2GraphemeProgress(
	grapheme: AppleV2Grapheme,
	currentMs: number
): number {
	if (currentMs <= grapheme.startMs) return 0;
	if (currentMs >= grapheme.endMs) return 1;
	const duration = grapheme.endMs - grapheme.startMs;
	return duration > 0 ? (currentMs - grapheme.startMs) / duration : 1;
}

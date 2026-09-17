export interface LyricSelectorRequest {
	videoId: string;
	initialTitle: string;
	initialArtist: string;
	duration?: number;
}

export const lyricSelector = $state<{
	open: boolean;
	request: LyricSelectorRequest | null;
}>({
	open: false,
	request: null
});

export function openLyricSelector(request: LyricSelectorRequest): void {
	lyricSelector.request = { ...request };
	lyricSelector.open = true;
}

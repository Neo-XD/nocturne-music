// Temporary diagnostic: surface the real startup error on the error page instead of "Internal Error".
import type { HandleClientError } from '@sveltejs/kit';

export const handleError: HandleClientError = ({ error }) => {
	console.error(error);
	const e = error as Error;
	return { message: e?.stack || e?.message || String(error) };
};

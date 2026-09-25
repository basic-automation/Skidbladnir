// The app runs in two places: inside the Tauri window, and in a plain browser during
// `nuxt dev` without the shell. Everything that crosses the IPC boundary goes through
// here so a missing host degrades to a clear message instead of an unhandled rejection.

/** True when running inside the Tauri window rather than a plain browser. */
export function isTauri(): boolean {
	return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

/** Call a Rust command. Throws with a readable message when there is no Tauri host. */
export async function invokeCommand<T>(command: string, args?: Record<string, unknown>): Promise<T> {
	if (!isTauri()) {
		throw new Error('Not running inside the Skidbladnir window, so the encoder is unavailable. Launch with `cargo tauri dev`.')
	}
	const { invoke } = await import('@tauri-apps/api/core')
	return invoke<T>(command, args)
}

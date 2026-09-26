// The settings model, mirroring the Rust `EncodeJob` and `WebpSettings` exactly.
//
// The DEFAULTS are deliberately not written here: they are fetched from the Rust core on
// mount. Duplicating them in TypeScript is precisely how the two drift apart, and they
// are pinned by test on the Rust side against the Electron UI's original numbers.

export type Mode = 'lossy' | 'lossless' | 'nearLossless' | 'jpegLike' | 'preset'
export type Preset = 'default' | 'photo' | 'picture' | 'drawing' | 'icon' | 'text'
export type FilterType = 'auto' | 'simple' | 'strong'
export type AlphaFiltering = 'off' | 'fast' | 'best'
export type TargetMetric = { kind: 'size', value: number } | { kind: 'psnr', value: number } | null

export type OutputFormat = 'webp'

export interface Resize { width: number, height: number }

/** What the window sends, and what presets and the preferences file store. */
export interface EncodeJob {
	format: OutputFormat
	/** Shared by every output format. */
	resize: Resize
	webp: WebpSettings
}

/** The WebP controls: the `cwebp` surface less the resize, which belongs to the job. */
export interface WebpSettings {
	mode: Mode
	preset: Preset | null
	quality: number
	alphaQuality: number
	alphaFiltering: AlphaFiltering | null
	method: number
	segments: number
	partitionLimit: number
	sns: number
	passes: number
	filter: FilterType
	filterStrength: number
	filterSharpness: number
	target: TargetMetric
	sharpYuv: boolean
	lowMemory: boolean
	multiThreading: boolean
}

/** Sizes and dimensions a completed conversion reports back. */
export interface ConversionReport {
	inputPath: string
	outputPath: string
	sourceBytes: number
	outputBytes: number
	width: number
	height: number
	/** The core's saving as a percentage of the original, negative if the output grew; `null` for a zero-byte source. */
	savingPercent: number | null
}

/**
 * Whether a mode reaches the advanced lossy controls.
 *
 * This mirrors `Mode::uses_lossy_options` in the Rust core. It is real encoder behaviour,
 * not decoration: the Electron UI sends the advanced values for the lossy mode alone, so
 * showing them in another mode would promise an effect the encoder will not deliver.
 */
export function usesLossyOptions(mode: Mode): boolean {
	return mode === 'lossy'
}

/** Whether the manual filter strength and sharpness apply. */
export function usesManualFilter(filter: FilterType): boolean {
	return filter === 'simple' || filter === 'strong'
}

/** Format a byte count the way a person reads it. */
export function formatBytes(bytes: number): string {
	if (bytes < 1024) return `${bytes} B`
	if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
	return `${(bytes / (1024 * 1024)).toFixed(2)} MB`
}

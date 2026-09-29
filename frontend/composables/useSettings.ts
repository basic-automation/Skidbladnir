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

export type OutputFormat = 'webp' | 'avif' | 'jxl' | 'heic'

/** `noEnlarge` leaves an image smaller than the target at its own size. */
export interface Resize { width: number, height: number, noEnlarge: boolean }

/** What the window sends, and what presets and the preferences file store. */
export interface EncodeJob {
	format: OutputFormat
	/** Shared by every output format. */
	resize: Resize
	webp: WebpSettings
	avif: AvifSettings
	jxl: JxlSettings
	heic: HeicSettings
}

/**
 * The HEIC controls. The standard edition's Kvazaar reads `quality` alone; the rest are
 * x265's, in the GPL edition. Defaults come from the Rust core, and are what libheif sets
 * for x265 itself. The fractional x265 options are held in tenths.
 */
export interface HeicSettings {
	quality: number
	lossless: boolean
	chroma: HeicChroma
	bitDepth: 'eight' | 'ten'
	preset: HeicPreset
	tune: HeicTune
	tuIntraDepth: number
	aqMode: HeicAqMode
	aqStrength: number
	psyRd: number
	psyRdoq: number
	deblock: boolean
	deblockStrength: number
	deblockThreshold: number
	sao: boolean
}

export type HeicChroma = '420' | '422' | '444'
export type HeicPreset = 'ultrafast' | 'superfast' | 'veryfast' | 'faster' | 'fast' | 'medium' | 'slow' | 'slower' | 'veryslow' | 'placebo'
export type HeicTune = 'psnr' | 'ssim' | 'grain' | 'fastdecode'
export type HeicAqMode = 'off' | 'variance' | 'autoVariance' | 'autoVarianceDark' | 'autoVarianceEdge'

/**
 * Which edition this build is: standard (Kvazaar, ISC) or GPL (x265, GPL-3.0-or-later).
 * Only the GPL edition has x265's HEIC controls.
 */
export interface Edition {
	name: 'standard' | 'gpl'
	label: string
	license: string
	x265: boolean
}

/** The JPEG XL controls, as libjxl exposes them. Defaults come from the Rust core. */
export interface JxlSettings {
	quality: number
	effort: number
	lossless: boolean
	losslessJpeg: boolean
	multiThreading: boolean
}

/** The AVIF controls, as `ravif` exposes them. Defaults come from the Rust core. */
export interface AvifSettings {
	quality: number
	alphaQuality: number
	speed: number
	bitDepth: 'eight' | 'ten'
	colorModel: 'ycbcr' | 'rgb'
	alphaMode: 'clean' | 'dirty' | 'premultiplied'
	multiThreading: boolean
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

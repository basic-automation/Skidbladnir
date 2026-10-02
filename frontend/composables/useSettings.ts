// The settings model, mirroring the Rust `EncodeJob` and each format's settings exactly:
// the same fields, in the JSON shape serde writes them.
//
// Each format's settings are its reference encoder's command line, one field per option
// (`cwebp`, `avifenc`, `cjxl`, `heif-enc -e kvazaar`); see the Rust `settings` module.
// `null` is "the option is not given", which for several options means something other
// than any value it could be given.
//
// The DEFAULTS are deliberately not written here: they come from the Rust core. Duplicating
// them in TypeScript is precisely how the two drift apart.

export type OutputFormat = 'webp' | 'avif' | 'jxl' | 'heic'

/** `cwebp -resize_mode`. */
export type ResizeMode = 'always' | 'downOnly' | 'upOnly'

/** `0` derives a dimension from the other; both `0` is no resize. */
export interface Resize { width: number, height: number, mode: ResizeMode }

/** A crop in source pixels, applied before the resize. */
export interface Crop { x: number, y: number, width: number, height: number }

/** What the window sends, and what presets and the preferences file store. */
export interface EncodeJob {
	format: OutputFormat
	/** Shared by every output format. */
	crop: Crop | null
	/** Shared by every output format. */
	resize: Resize
	webp: WebpSettings
	avif: AvifSettings
	jxl: JxlSettings
	heic: HeicSettings
	/** Read a TIFF's alpha as cwebp (WebP) or heif-enc (HEIC) does, rather than correctly. */
	tiffAlphaLikeReference: boolean
}

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

// ---- WebP: cwebp ------------------------------------------------------------------------

export type Preset = 'default' | 'photo' | 'picture' | 'drawing' | 'icon' | 'text'
export type FilterType = 'simple' | 'strong'
export type AlphaFiltering = 'off' | 'fast' | 'best'
export type ImageHint = 'default' | 'picture' | 'photo' | 'graph'
export type TargetMetric = { kind: 'size', value: number } | { kind: 'psnr', value: number }

export interface WebpSettings {
	lossless: boolean
	nearLossless: number
	exact: boolean
	quality: number
	alphaQuality: number
	alphaCompression: boolean
	alphaFiltering: AlphaFiltering
	method: number
	imageHint: ImageHint
	target: TargetMetric | null
	segments: number
	sns: number
	filterStrength: number
	filterSharpness: number
	filterType: FilterType
	autofilter: boolean
	passes: number
	qmin: number
	qmax: number
	preprocessing: number
	partitionLimit: number
	jpegLike: boolean
	sharpYuv: boolean
	lowMemory: boolean
	multiThreading: boolean
	keepAlpha: boolean
	/** `0xRRGGBB`. */
	blendAlpha: number | null
	metadata: { exif: boolean, icc: boolean, xmp: boolean }
	animation: WebpAnimation
}

/** The animation encoder's options (img2webp, gif2webp); `null` is the tool's default. */
export interface WebpAnimation {
	minimizeSize: boolean
	allowMixed: boolean
	kmin: number | null
	kmax: number | null
	loopCount: number | null
	loopCompatibility: boolean
	frameDuration: number | null
}

// ---- AVIF: avifenc ----------------------------------------------------------------------

export type YuvFormat = 'auto' | 'yuv444' | 'yuv422' | 'yuv420' | 'yuv400'
export interface Cicp { primaries: number, transfer: number, matrix: number }
export interface QuantizerRange { min: number, max: number }
export type Tiling = { kind: 'automatic' } | { kind: 'manual', rowsLog2: number, colsLog2: number }
export interface Fraction { numerator: number, denominator: number }
export interface Grid { columns: number, rows: number }
export type CleanAperture = { kind: 'crop', values: [number, number, number, number] } | { kind: 'raw', values: [number, number, number, number, number, number, number, number] }
export interface CodecOption { key: string, value: string }
/** Where a piece of metadata comes from: the input, nowhere, or a file. */
export type MetadataSource = { kind: 'keep' } | { kind: 'strip' } | { kind: 'file', path: string }

export interface AvifSettings {
	quality: number | null
	qualityAlpha: number | null
	speed: number | null
	lossless: boolean
	depth: number | null
	depthExtension: number | null
	yuv: YuvFormat
	premultiply: boolean
	sharpYuv: boolean
	cicp: Cicp | null
	limitedRange: boolean
	targetSize: number | null
	progressive: boolean
	grid: Grid | null
	quantizer: QuantizerRange | null
	alphaQuantizer: QuantizerRange | null
	tiling: Tiling
	scalingMode: Fraction | null
	codecOptions: CodecOption[]
	pasp: [number, number] | null
	cleanAperture: CleanAperture | null
	irot: number | null
	imir: number | null
	clli: [number, number] | null
	icc: MetadataSource
	exif: MetadataSource
	xmp: MetadataSource
	jobs: number | null
}

// ---- JPEG XL: cjxl ----------------------------------------------------------------------

/** `cjxl`'s `Override`: the flag not given, `=0`, or `=1`. */
export type Tristate = 'default' | 'off' | 'on'
export type JxlTarget = { kind: 'default' } | { kind: 'distance', value: number } | { kind: 'quality', value: number }
export type WhitePoint = { kind: 'd65' | 'e' | 'dci' | 'd50' } | { kind: 'custom', xy: [number, number] }
export type Primaries = { kind: 'srgb' | 'rec2100' | 'p3' | 'adobe' | 'proPhoto' } | { kind: 'custom', xy: [number, number, number, number, number, number] }
export type RenderingIntent = 'perceptual' | 'relative' | 'saturation' | 'absolute'
export type TransferFunction = { kind: 'srgb' | 'bt709' | 'linear' | 'pq' | 'hlg' | 'dci' | 'adobe' | 'proPhoto' } | { kind: 'gamma', gamma: number }
export interface JxlColorSpace { gray: boolean, whitePoint: WhitePoint, primaries: Primaries, renderingIntent: RenderingIntent, transferFunction: TransferFunction }

export interface JxlSettings {
	target: JxlTarget
	alphaDistance: number | null
	effort: number
	allowExpertOptions: boolean
	brotliEffort: number
	progressive: boolean
	groupOrder: Tristate
	container: Tristate
	compressBoxes: Tristate
	modular: Tristate
	losslessJpeg: boolean
	photonNoiseIso: number
	intensityTarget: number
	allowJpegReconstruction: boolean
	codestreamLevel: number
	buffering: number
	fasterDecoding: number
	premultiply: number
	keepInvisible: Tristate
	centerX: number
	centerY: number
	progressiveAc: boolean
	qprogressiveAc: boolean
	progressiveDc: number
	resampling: number
	ecResampling: number
	alreadyDownsampled: boolean
	upsamplingMode: number
	epf: number
	gaborish: Tristate
	overrideBitdepth: number
	noise: Tristate
	jpegReconstructionCfl: Tristate
	dots: Tristate
	patches: Tristate
	frameIndexBox: boolean
	disablePerceptualOptimizations: boolean
	outputMode: number
	streamingOutput: boolean
	iterations: number
	modularColorspace: number
	modularGroupSize: number
	modularPredictor: number
	modularNbPrevChannels: number
	modularPaletteColors: number
	modularLossyPalette: boolean
	preCompact: number
	postCompact: number
	responsive: boolean | null
	multiThreading: boolean
	colorSpace: JxlColorSpace | null
	iccFile: string | null
	exif: MetadataSource
	xmp: MetadataSource
	jumbf: MetadataSource
}

// ---- HEIC: heif-enc -e kvazaar (standard edition) or -e x265 (GPL edition) --------------

export type HeicChroma = '420' | '422' | '444'
export type HeicPreset = 'ultrafast' | 'superfast' | 'veryfast' | 'faster' | 'fast' | 'medium' | 'slow' | 'slower' | 'veryslow' | 'placebo'
export type HeicTune = 'psnr' | 'ssim' | 'grain' | 'fastdecode'
export type HeicAqMode = 'off' | 'variance' | 'autoVariance' | 'autoVarianceDark' | 'autoVarianceEdge'

export type ChromaDownsampling = 'nearestNeighbor' | 'average' | 'sharpYuv'
export type ColorProfile =
	| { preset: 'custom', matrixCoefficients: number, colourPrimaries: number, transferCharacteristics: number, fullRange: boolean }
	| { preset: 'auto' | 'bt601' | 'bt709' | 'compatible' | 'bt2020' }
export type Orientation = 'normal' | 'flipHorizontally' | 'rotate180' | 'flipVertically' | 'rotate90CwThenFlipHorizontally' | 'rotate90Cw' | 'rotate90CwThenFlipVertically' | 'rotate270Cw'
export type OmafProjection = 'equirectangular' | 'cubeMap'

export interface HeicSettings {
	quality: number
	lossless: boolean
	alpha: boolean
	premultipliedAlpha: boolean
	thumbnail: number | null
	thumbnailAlpha: boolean
	chromaDownsampling: ChromaDownsampling | null
	colorProfile: ColorProfile
	twoColrBoxes: boolean
	clli: [number, number] | null
	pasp: [number, number] | null
	orientation: Orientation
	cutTiles: number | null
	omafProjection: OmafProjection | null
	description: string
	compatibleBrands: string[]
	unif: boolean
	mini: boolean
	/** Which of the input's metadata goes into the file; `heif-enc` copies all three. */
	metadata: { icc: boolean, exif: boolean, xmp: boolean }
	// x265's controls, the GPL edition's alone: `heif-enc -p` parameters. Kvazaar has no
	// equivalent, so the standard edition neither shows nor sends anything but the defaults.
	// The fractional ones are held in tenths.
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
	/** GPL edition: `-p x265:KEY=VALUE`, applied after every control above. */
	x265Parameters: CodecOption[]
}

// ---- Results ----------------------------------------------------------------------------

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

/** Format a byte count the way a person reads it. */
export function formatBytes(bytes: number): string {
	if (bytes < 1024) return `${bytes} B`
	if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
	return `${(bytes / (1024 * 1024)).toFixed(2)} MB`
}

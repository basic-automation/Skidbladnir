<script setup lang="ts">
// Every cjxl option that changes the file it writes for a still image. Help text is
// cjxl's own (`cjxl -h -v -v -v -v`), with the flag it sets. -1 is cjxl's "the encoder
// chooses" for its numeric options, and each three-way switch has "encoder decides" for
// the flag left out.
import { computed } from 'vue'
import type { JxlColorSpace, JxlSettings, Primaries, RenderingIntent, TransferFunction, WhitePoint } from '~/composables/useSettings'

const model = defineModel<JxlSettings>({ required: true })
const s = computed(() => model.value)

const choose = (label = 'Encoder chooses') => ({ label, value: -1 })
const numbers = (values: number[]) => values.map(value => ({ label: String(value), value }))

const RESAMPLING = [choose('Encoder chooses (only at very low quality)'), ...numbers([1, 2, 4, 8]).map(item => ({ ...item, label: `${item.label}×${item.label}` }))]
const CODESTREAM_LEVELS = [choose(), ...numbers([5, 10])]
const BUFFERING = [choose(), { label: '0: buffer the entire image', value: 0 }, { label: '1: stream input for large images', value: 1 }, { label: '2: stream with a lower threshold', value: 2 }, { label: '3: deprecated', value: 3 }]
const OUTPUT_MODES = [choose('Encoder decides'), { label: '0: buffer output internally', value: 0 }, { label: '1: streaming with seeking', value: 1 }, { label: '2: out-of-order jxlp', value: 2 }]
const PREMULTIPLY = [{ label: 'As the input is', value: -1 }, { label: 'Not premultiplied', value: 0 }, { label: 'Premultiplied', value: 1 }]
const PROGRESSIVE_DC = [choose(), { label: '0: disable', value: 0 }, { label: '1: extra 64×64 pass', value: 1 }, { label: '2: extra 512×512 and 64×64 passes', value: 2 }]
const UPSAMPLING = [{ label: 'Non-separable (default)', value: -1 }, { label: 'Nearest neighbour', value: 0 }, { label: '1', value: 1 }]
const EPF = [choose(), ...numbers([0, 1, 2, 3])]
const GROUP_SIZES = [choose(), { label: '128×128', value: 0 }, { label: '256×256', value: 1 }, { label: '512×512', value: 2 }, { label: '1024×1024', value: 3 }]
const PREDICTORS = [choose('Encoder chooses (14, or 15 at effort 10)'), ...['zero', 'left', 'top', 'avg0', 'select', 'gradient', 'weighted', 'topright', 'topleft', 'leftleft', 'avg1', 'avg2', 'avg3', 'toptop predictive average', 'mix 5 and 6', 'mix everything'].map((name, value) => ({ label: `${value}: ${name}`, value }))]
const TARGETS = [{ label: 'cjxl default', value: 'default' }, { label: 'Distance', value: 'distance' }, { label: 'Quality', value: 'quality' }]
const RESPONSIVE = [{ label: 'Not given', value: 'unset' }, { label: 'Off (0)', value: 'off' }, { label: 'On (1)', value: 'on' }]

const target = computed({
	get: () => s.value.target.kind,
	set: (kind: string) => {
		s.value.target = kind === 'distance' ? { kind: 'distance', value: 1 } : kind === 'quality' ? { kind: 'quality', value: 90 } : { kind: 'default' }
	},
})
const responsive = computed({
	get: () => (s.value.responsive === null ? 'unset' : s.value.responsive ? 'on' : 'off'),
	set: (value: string) => { s.value.responsive = value === 'unset' ? null : value === 'on' },
})

const SRGB_SPACE: JxlColorSpace = { gray: false, whitePoint: { kind: 'd65' }, primaries: { kind: 'srgb' }, renderingIntent: 'relative', transferFunction: { kind: 'srgb' } }
const WHITE_POINTS = [{ label: 'D65', value: 'd65' }, { label: 'E', value: 'e' }, { label: 'DCI', value: 'dci' }, { label: 'D50', value: 'd50' }, { label: 'Custom xy', value: 'custom' }]
const PRIMARIES = [{ label: 'sRGB / BT.709', value: 'srgb' }, { label: 'BT.2020 / BT.2100', value: 'rec2100' }, { label: 'DCI-P3', value: 'p3' }, { label: 'Adobe RGB (1998)', value: 'adobe' }, { label: 'ProPhoto RGB', value: 'proPhoto' }, { label: 'Custom xy', value: 'custom' }]
const INTENTS: { label: string, value: RenderingIntent }[] = [{ label: 'Perceptual', value: 'perceptual' }, { label: 'Relative', value: 'relative' }, { label: 'Saturation', value: 'saturation' }, { label: 'Absolute', value: 'absolute' }]
const TRANSFERS = [{ label: 'sRGB', value: 'srgb' }, { label: 'BT.709', value: 'bt709' }, { label: 'Linear', value: 'linear' }, { label: 'PQ', value: 'pq' }, { label: 'HLG', value: 'hlg' }, { label: 'DCI', value: 'dci' }, { label: 'Adobe', value: 'adobe' }, { label: 'ProPhoto', value: 'proPhoto' }, { label: 'Gamma', value: 'gamma' }]

function setWhitePoint(space: JxlColorSpace, kind: string) {
	space.whitePoint = (kind === 'custom' ? { kind: 'custom', xy: [0.3127, 0.329] } : { kind }) as WhitePoint
}
function setPrimaries(space: JxlColorSpace, kind: string) {
	space.primaries = (kind === 'custom' ? { kind: 'custom', xy: [0.64, 0.33, 0.3, 0.6, 0.15, 0.06] } : { kind }) as Primaries
}
function setTransfer(space: JxlColorSpace, kind: string) {
	space.transferFunction = (kind === 'gamma' ? { kind: 'gamma', gamma: 0.45455 } : { kind }) as TransferFunction
}
const PRIMARY_LABELS = ['Red x', 'Red y', 'Green x', 'Green y', 'Blue x', 'Blue y']
</script>

<template>
	<div class="flex flex-col gap-8">
		<ControlPanel title="Quality" class="pt-8">
			<div class="grid grid-cols-3 gap-4">
				<ControlChoice v-model="target" label="Target" :items="TARGETS" help="-d / -q · mutually exclusive; cjxl's default is distance 1.0, or lossless for JPEG and GIF input" />
				<ControlSlider v-if="s.target.kind === 'distance'" v-model="s.target.value" label="Distance" :min="0" :max="25" :step="0.1" :decimals="3" help="-d · target visual distance in JND units; 0.0 = mathematically lossless, 1.0 = visually lossless" />
				<ControlSlider v-if="s.target.kind === 'quality'" v-model="s.target.value" label="Quality" :min="0" :max="100" :decimals="3" help="-q · 100 = mathematically lossless, 90 = visually lossless" />
				<ControlOptional v-model="s.alphaDistance" label="Alpha distance" :fallback="1" unset="-a · not given: 0.0, lossless alpha">
					<ControlSlider v-model="s.alphaDistance" label="Alpha distance" :min="0" :max="25" :step="0.1" :decimals="3" help="-a · target visual distance for the alpha channel" />
				</ControlOptional>
			</div>
			<div class="grid grid-cols-3 gap-4">
				<ControlSlider v-model="s.effort" label="Effort" :min="1" :max="s.allowExpertOptions ? 11 : 10" help="-e · encoder effort; higher values allow more computation" />
				<ControlToggle v-model="s.allowExpertOptions" label="Allow effort 11" help="--allow_expert_options · somewhat denser lossless compression at an extreme compute cost" />
			</div>
			<div class="grid grid-cols-3 gap-4">
				<ControlToggle v-model="s.losslessJpeg" label="Transcode JPEG losslessly" help="-j 1 · losslessly transcode JPEG data; off decodes it to pixels and reencodes. Not with a crop or resize." />
				<ControlToggle v-model="s.allowJpegReconstruction" label="Allow JPEG reconstruction" help="--allow_jpeg_reconstruction · store what is needed to rebuild the JPEG bit for bit" />
			</div>
		</ControlPanel>

		<ControlPanel title="Mode and progression">
			<div class="grid grid-cols-3 gap-4">
				<ControlTristate v-model="s.modular" label="Modular mode" help="-m · off = VarDCT, on = modular" />
				<ControlToggle v-model="s.progressive" label="Progressive" help="-p · more progressive/responsive decoding" />
				<ControlChoice v-model="responsive" label="Squeeze transform" :items="RESPONSIVE" help="-R · the default is off for lossless output, on for lossy" />
			</div>
			<div class="grid grid-cols-3 gap-4">
				<ControlTristate v-model="s.groupOrder" label="Centre-first group order" help="--group_order · 0 = scanline order, 1 = center-first order" />
				<ControlNumber v-model="s.centerX" label="Centre x" :min="-1" help="--center_x · -1 = middle of the image" />
				<ControlNumber v-model="s.centerY" label="Centre y" :min="-1" help="--center_y · -1 = middle of the image" />
			</div>
			<div class="grid grid-cols-3 gap-4">
				<ControlToggle v-model="s.progressiveAc" label="Progressive AC" help="--progressive_ac · use the progressive mode for AC" />
				<ControlToggle v-model="s.qprogressiveAc" label="Quantised progressive AC" help="--qprogressive_ac · progressive mode for AC with shift quantization" />
				<ControlSelect v-model="s.progressiveDc" label="Progressive DC" :items="PROGRESSIVE_DC" help="--progressive_dc · number of progressive-DC frames" />
			</div>
		</ControlPanel>

		<ControlPanel title="Colour and metadata">
			<div class="grid grid-cols-3 gap-4">
				<ControlOptional v-model="s.colorSpace" label="Colour space of untagged input" :fallback="SRGB_SPACE" help="-x color_space · used only when the input says nothing about its colour" unset="-x color_space · not given">
					<template #default="{ value }">
						<ControlToggle v-model="value.gray" label="Grayscale" help="must match the image" />
						<ControlChoice :model-value="value.whitePoint.kind" label="White point" :items="WHITE_POINTS" @update:model-value="(k: string) => setWhitePoint(value, k)" />
						<div v-if="value.whitePoint.kind === 'custom'" class="grid grid-cols-2 gap-2">
							<ControlNumber v-model="value.whitePoint.xy[0]" label="White x" :step="0.0001" :decimals="6" />
							<ControlNumber v-model="value.whitePoint.xy[1]" label="White y" :step="0.0001" :decimals="6" />
						</div>
						<template v-if="!value.gray">
							<ControlSelect :model-value="value.primaries.kind" label="Primaries" :items="PRIMARIES" @update:model-value="(k: string) => setPrimaries(value, k)" />
							<div v-if="value.primaries.kind === 'custom'" class="grid grid-cols-2 gap-2">
								<ControlNumber v-for="(_, index) in value.primaries.xy" :key="index" v-model="value.primaries.xy[index]" :label="PRIMARY_LABELS[index]!" :step="0.0001" :decimals="6" />
							</div>
						</template>
						<ControlChoice v-model="value.renderingIntent" label="Rendering intent" :items="INTENTS" />
						<ControlSelect :model-value="value.transferFunction.kind" label="Transfer function" :items="TRANSFERS" @update:model-value="(k: string) => setTransfer(value, k)" />
						<ControlNumber v-if="value.transferFunction.kind === 'gamma'" v-model="value.transferFunction.gamma" label="Gamma" :min="0.00001" :max="1" :step="0.00001" :decimals="6" help="the encoding exponent: 0.45455 is 2.2" />
					</template>
				</ControlOptional>
				<ControlOptional v-model="s.iccFile" label="ICC profile for untagged input" fallback="" unset="-x icc_pathname · not given">
					<ControlFile v-model="s.iccFile" label="ICC profile" help="-x icc_pathname · a binary file containing an ICC profile" />
				</ControlOptional>
				<div class="flex flex-col">
					<ControlNumber v-model="s.intensityTarget" label="Intensity target" :min="0" :decimals="3" unit="nits" help="--intensity_target · 0 = choose a sensible value based on the color encoding" />
					<ControlNumber v-model="s.overrideBitdepth" label="Bit depth" :min="0" :max="32" help="--override_bitdepth · 0 = use the input image bit depth" />
				</div>
			</div>
			<div class="grid grid-cols-3 gap-4">
				<ControlSelect v-model="s.premultiply" label="Premultiplied alpha" :items="PREMULTIPLY" help="--premultiply · force premultiplied (associated) alpha" />
				<ControlTristate v-model="s.keepInvisible" label="Keep invisible pixels" help="--keep_invisible · preserve colors of invisible pixels; on by default for lossless" />
			</div>
			<div class="grid grid-cols-3 gap-4">
				<ControlMetadata v-model="s.exif" label="Exif" help="-x exif=FILE / -x strip=exif" />
				<ControlMetadata v-model="s.xmp" label="XMP" help="-x xmp=FILE / -x strip=xmp" />
				<ControlMetadata v-model="s.jumbf" label="JUMBF" help="-x jumbf=FILE / -x strip=jumbf" />
			</div>
			<div class="grid grid-cols-3 gap-4">
				<ControlTristate v-model="s.container" label="Container" help="--container · 0 = only when needed, 1 = always" />
				<ControlTristate v-model="s.compressBoxes" label="Compress metadata boxes" help="--compress_boxes · Brotli compression for metadata boxes" />
				<ControlSlider v-model="s.brotliEffort" label="Brotli effort" :min="0" :max="11" help="--brotli_effort · higher values target higher density" />
			</div>
		</ControlPanel>

		<ControlDisclosure title="Filters and tools">
			<div class="grid grid-cols-3 gap-4">
				<ControlSelect v-model="s.epf" label="Edge-preserving filter" :items="EPF" help="--epf · edge preserving filter strength" />
				<ControlTristate v-model="s.gaborish" label="Gaborish filter" help="--gaborish" />
				<ControlSelect v-model="s.fasterDecoding" label="Faster decoding" :items="numbers([0, 1, 2, 3, 4])" help="--faster_decoding · decode speed at the expense of quality or density" />
			</div>
			<div class="grid grid-cols-3 gap-4">
				<ControlNumber v-model="s.photonNoiseIso" label="Photon noise" :min="0" :decimals="3" unit="ISO" help="--photon_noise_iso · emulate film or sensor noise; 100 is low, 3200 a lot" />
				<ControlTristate v-model="s.noise" label="Adaptive noise" help="--noise · --photon_noise_iso is recommended instead" />
				<ControlTristate v-model="s.dots" label="Dots" help="--dots · dots generation" />
			</div>
			<div class="grid grid-cols-3 gap-4">
				<ControlTristate v-model="s.patches" label="Patches" help="--patches · patches generation" />
				<ControlTristate v-model="s.jpegReconstructionCfl" label="Chroma-from-luma for JPEG" help="--jpeg_reconstruction_cfl · CFL for lossless JPEG reconstruction" />
				<ControlToggle v-model="s.disablePerceptualOptimizations" label="No perceptual optimisations" help="--disable_perceptual_optimizations" />
			</div>
			<div class="grid grid-cols-3 gap-4">
				<ControlSelect v-model="s.resampling" label="Resampling" :items="RESAMPLING" help="--resampling · for color channels" />
				<ControlSelect v-model="s.ecResampling" label="Extra channel resampling" :items="RESAMPLING" help="--ec_resampling · for extra channels like alpha" />
				<ControlSelect v-model="s.upsamplingMode" label="Upsampling" :items="UPSAMPLING" help="--upsampling_mode · decoder upsampling; nearest neighbour for pixel art" />
			</div>
			<div class="grid grid-cols-3 gap-4">
				<ControlToggle v-model="s.alreadyDownsampled" label="Already downsampled" help="--already_downsampled · signal upsampling without downsampling first" />
			</div>
		</ControlDisclosure>

		<ControlDisclosure title="Modular mode">
			<div class="grid grid-cols-3 gap-4">
				<ControlNumber v-model="s.iterations" label="MA tree learning" :min="-1" :max="100" :decimals="3" unit="%" help="-I · percentage of pixels used to learn MA trees; -1 = encoder chooses, 0 = no MA trees" />
				<ControlNumber v-model="s.modularColorspace" label="Colour transform" :min="-1" :max="41" help="-C · -1 = try several, 0 = none, 1..41 = fixed RCT, 6 = YCoCg" />
				<ControlSelect v-model="s.modularGroupSize" label="Group size" :items="GROUP_SIZES" help="-g · modular group size" />
			</div>
			<div class="grid grid-cols-3 gap-4">
				<ControlSelect v-model="s.modularPredictor" label="Predictor" :items="PREDICTORS" help="-P · predictor(s) to use" />
				<ControlNumber v-model="s.modularNbPrevChannels" label="Previous-channel properties" :min="-1" :max="11" help="-E · maximum previous-channel MA tree properties; -1 = encoder chooses" />
				<ControlNumber v-model="s.modularPaletteColors" label="Palette colours" :min="-1" help="--modular_palette_colors · use a palette at or below this many colours; -1 = encoder chooses" />
			</div>
			<div class="grid grid-cols-3 gap-4">
				<ControlToggle v-model="s.modularLossyPalette" label="Lossy palette" help="--modular_lossy_palette · use delta palette in a lossy way" />
				<ControlNumber v-model="s.preCompact" label="Global channel palette" :min="-1" :max="100" :decimals="3" unit="%" help="-X · global channel palette below this share of the range; -1 = encoder chooses" />
				<ControlNumber v-model="s.postCompact" label="Per-group channel palette" :min="-1" :max="100" :decimals="3" unit="%" help="-Y · local channel palette below this share of the range; -1 = encoder chooses" />
			</div>
		</ControlDisclosure>

		<ControlDisclosure title="Codestream and output">
			<div class="grid grid-cols-3 gap-4">
				<ControlSelect v-model="s.codestreamLevel" label="Codestream level" :items="CODESTREAM_LEVELS" help="--codestream_level" />
				<ControlSelect v-model="s.buffering" label="Buffering" :items="BUFFERING" help="--buffering · how much input buffering libjxl uses" />
				<ControlSelect v-model="s.outputMode" label="Output mode" :items="OUTPUT_MODES" help="--output_mode" />
			</div>
			<div class="grid grid-cols-3 gap-4">
				<ControlToggle v-model="s.streamingOutput" label="Streaming output" help="--streaming_output · incremental writing of the output file" />
				<ControlToggle v-model="s.frameIndexBox" label="Frame index box" help="--frame_indexing=1 · index the frame in a frame index box" />
				<ControlToggle v-model="s.multiThreading" label="Multi-threading" help="--num_threads · off is 0, no multithreading; the output is the same" />
			</div>
		</ControlDisclosure>
	</div>
</template>

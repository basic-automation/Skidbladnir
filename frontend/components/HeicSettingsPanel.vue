<script setup lang="ts">
// Every heif-enc option that changes the file it writes for a still image, with the
// edition's encoder: Kvazaar in the standard edition, x265 in the GPL one. Help text is
// heif-enc's own (`heif-enc --help`), with the flag it sets.
//
// heif-enc's -L cannot work with Kvazaar (it asks for a chroma parameter Kvazaar does not
// have), so lossless there is Kvazaar's own, -p lossless=true: the conversion to 4:2:0
// before it still loses, as it does on the command line. With x265 it is -L.
import { computed } from 'vue'
import type { ChromaDownsampling, HeicSettings, OmafProjection, Orientation } from '~/composables/useSettings'

const model = defineModel<HeicSettings>({ required: true })
const props = defineProps<{
	/** The GPL edition: HEIC through x265, with its controls. */
	x265: boolean
}>()
const s = computed(() => model.value)

const X265_CHROMA = [{ label: '4:2:0', value: '420' }, { label: '4:2:2', value: '422' }, { label: '4:4:4', value: '444' }]
const X265_DEPTH = [{ label: '8-bit', value: 'eight' }, { label: '10-bit', value: 'ten' }]
const X265_PRESETS = [
	{ label: 'Ultrafast', value: 'ultrafast' },
	{ label: 'Superfast', value: 'superfast' },
	{ label: 'Very fast', value: 'veryfast' },
	{ label: 'Faster', value: 'faster' },
	{ label: 'Fast', value: 'fast' },
	{ label: 'Medium', value: 'medium' },
	{ label: 'Slow (libheif\'s default)', value: 'slow' },
	{ label: 'Slower', value: 'slower' },
	{ label: 'Very slow', value: 'veryslow' },
	{ label: 'Placebo', value: 'placebo' },
]
const X265_TUNES = [{ label: 'SSIM', value: 'ssim' }, { label: 'PSNR', value: 'psnr' }, { label: 'Grain', value: 'grain' }, { label: 'Fast decode', value: 'fastdecode' }]
const X265_AQ = [
	{ label: 'Off', value: 'off' },
	{ label: 'Variance', value: 'variance' },
	{ label: 'Auto-variance', value: 'autoVariance' },
	{ label: 'Auto-variance, dark bias', value: 'autoVarianceDark' },
	{ label: 'Auto-variance, edges', value: 'autoVarianceEdge' },
]
const encoderName = computed(() => (props.x265 ? 'x265' : 'Kvazaar'))

const CHROMA = [{ label: 'Encoder decides', value: 'unset' }, { label: 'Nearest neighbour', value: 'nearestNeighbor' }, { label: 'Average', value: 'average' }, { label: 'Sharp YUV', value: 'sharpYuv' }]
const PROFILES = [{ label: 'Custom', value: 'custom' }, { label: 'Auto', value: 'auto' }, { label: 'Compatible', value: 'compatible' }, { label: 'Rec. 601', value: 'bt601' }, { label: 'Rec. 709', value: 'bt709' }, { label: 'Rec. 2020', value: 'bt2020' }]
const ORIENTATIONS: { label: string, value: Orientation }[] = [
	{ label: 'As stored', value: 'normal' },
	{ label: 'Flip horizontally (--flip-h)', value: 'flipHorizontally' },
	{ label: 'Rotate 180° (--rotate-cw 180)', value: 'rotate180' },
	{ label: 'Flip vertically (--flip-v)', value: 'flipVertically' },
	{ label: 'Rotate 90° clockwise, flip horizontally', value: 'rotate90CwThenFlipHorizontally' },
	{ label: 'Rotate 90° clockwise (--rotate-cw 90)', value: 'rotate90Cw' },
	{ label: 'Rotate 90° clockwise, flip vertically', value: 'rotate90CwThenFlipVertically' },
	{ label: 'Rotate 270° clockwise (--rotate-cw 270)', value: 'rotate270Cw' },
]
const PROJECTIONS: { label: string, value: OmafProjection }[] = [{ label: 'Equirectangular', value: 'equirectangular' }, { label: 'Cube map', value: 'cubeMap' }]

const chroma = computed({
	get: () => s.value.chromaDownsampling ?? 'unset',
	set: (value: string) => { s.value.chromaDownsampling = value === 'unset' ? null : value as ChromaDownsampling },
})
const profile = computed({
	get: () => s.value.colorProfile.preset,
	set: (preset: string) => {
		s.value.colorProfile = preset === 'custom' ? { preset: 'custom', matrixCoefficients: 6, colourPrimaries: 1, transferCharacteristics: 13, fullRange: true } : { preset: preset as 'auto' }
	},
})
const brands = computed({
	get: () => s.value.compatibleBrands,
	set: (next: string[]) => { s.value.compatibleBrands = next },
})
</script>

<template>
	<div class="flex flex-col gap-8">
		<ControlPanel title="Quality" class="pt-8">
			<div class="grid grid-cols-3 gap-4">
				<ControlSlider v-model="s.quality" label="Quality" :min="0" :max="100" :help="x265 ? '-q · set output quality (0-100) for lossy compression; x265\'s CRF is (100 − quality) ÷ 2' : '-q · set output quality (0-100) for lossy compression'" :disabled="x265 && s.lossless" />
				<ControlToggle v-if="x265" v-model="s.lossless" label="Lossless" help="-L · generate lossless output: RGB at 4:4:4, every pixel kept (-q and the chroma, depth and rate controls do not apply)" />
				<ControlToggle v-else v-model="s.lossless" label="Lossless coding" help="-p lossless=true · Kvazaar codes the 4:2:0 image without loss" />
				<ControlChoice v-model="chroma" label="Chroma downsampling" :items="CHROMA" help="-C · force chroma downsampling algorithm; left out, libheif picks, which is not the same as average" />
			</div>
			<div v-if="x265 && !s.lossless" class="grid grid-cols-3 gap-4">
				<ControlChoice v-model="s.chroma" label="Chroma" :items="X265_CHROMA" help="-p chroma · 4:2:0 is what phones write and every reader decodes; 4:4:4 keeps colour at full resolution" />
				<ControlChoice v-model="s.bitDepth" label="Bit depth" :items="X265_DEPTH" help="-b · 10-bit is HEVC Main 10, as phones write HDR photos; smooth gradients band less, even from an 8-bit image" />
			</div>
			<p class="px-2.5 text-xs text-paleday-dim">
				HEVC in HEIF, the format iPhones use, encoded by {{ encoderName }}<template v-if="!x265">, which writes 8-bit 4:2:0 colour. The GPL edition encodes with x265 instead, which adds -L lossless, 4:4:4, 10-bit and x265's own controls</template>.
			</p>
		</ControlPanel>

		<ControlPanel v-if="x265" title="x265">
			<div class="grid grid-cols-3 gap-4">
				<ControlSelect v-model="s.preset" label="Preset" :items="X265_PRESETS" help="-p preset · slower presets search harder for a smaller file" />
				<ControlChoice v-model="s.tune" label="Tune" :items="X265_TUNES" help="-p tune · sets x265's remaining decisions for a goal; the controls here apply over it" />
				<ControlSlider v-model="s.tuIntraDepth" label="TU intra depth" :min="1" :max="4" help="-p tu-intra-depth · how far transform units split (at most 3 under 32 px)" />
			</div>
			<template v-if="!s.lossless">
				<div class="grid grid-cols-4 gap-3">
					<ControlSelect v-model="s.aqMode" label="Adaptive quantisation" :items="X265_AQ" help="-p x265:aq-mode · how bits move between flat and detailed areas" />
					<ControlSlider v-model="s.aqStrength" label="AQ strength" :min="0" :max="30" :divisor="10" help="-p x265:aq-strength · 0.0 .. 3.0" :disabled="s.aqMode === 'off'" />
					<ControlSlider v-model="s.psyRd" label="Psy-RD" :min="0" :max="50" :divisor="10" help="-p x265:psy-rd · 0.0 .. 5.0; keeps texture rather than the smoothest match" />
					<ControlSlider v-model="s.psyRdoq" label="Psy-RDOQ" :min="0" :max="500" :divisor="10" help="-p x265:psy-rdoq · 0.0 .. 50.0; keeps detail when quantising" />
				</div>
				<div class="grid grid-cols-4 gap-3">
					<ControlToggle v-model="s.deblock" label="Deblocking" help="-p x265:deblock · the in-loop filter that smooths block edges" />
					<ControlSlider v-model="s.deblockStrength" label="Deblocking strength" :min="-6" :max="6" help="tC offset: higher smooths more" :disabled="!s.deblock" />
					<ControlSlider v-model="s.deblockThreshold" label="Deblocking threshold" :min="-6" :max="6" help="beta offset: higher treats more edges as blocking" :disabled="!s.deblock" />
					<ControlToggle v-model="s.sao" label="SAO" help="-p x265:sao · sample adaptive offset: smooths ringing around edges" />
				</div>
				<p class="px-2.5 text-xs text-paleday-dim">
					These shape the colour image. libheif encodes transparency separately, with its own values for them.
				</p>
			</template>
			<ControlDisclosure title="Other x265 parameters">
				<p class="px-2.5 text-xs text-paleday-dim">
					-p x265:KEY=VALUE · any of x265's own parameters, passed in order after the controls above, so one here overrides them; for example rd=4, rskip=0, limit-tu=2.
				</p>
				<div v-for="(parameter, index) in s.x265Parameters" :key="index" class="flex items-end gap-2">
					<ControlText v-model="parameter.key" :label="`Parameter ${index + 1} key`" placeholder="rd" class="flex-1" />
					<ControlText v-model="parameter.value" :label="`Parameter ${index + 1} value`" placeholder="4" class="flex-1" />
					<UButton color="neutral" variant="ghost" icon="i-lucide-x" :aria-label="`Remove parameter ${index + 1}`" class="mb-2" @click="s.x265Parameters.splice(index, 1)" />
				</div>
				<div class="px-2.5">
					<UButton size="sm" color="neutral" variant="soft" icon="i-material-symbols-add" @click="s.x265Parameters.push({ key: '', value: '' })">
						Add a parameter
					</UButton>
				</div>
			</ControlDisclosure>
		</ControlPanel>

		<ControlPanel title="Transparency and thumbnail">
			<div class="grid grid-cols-3 gap-4">
				<ControlToggle :model-value="!s.alpha" label="Drop alpha" help="--no-alpha · do not save alpha channel" @update:model-value="(drop: boolean) => { s.alpha = !drop }" />
				<ControlToggle v-model="s.premultipliedAlpha" label="Premultiplied alpha" help="--premultiplied-alpha · input image has premultiplied alpha" />
			</div>
			<div class="grid grid-cols-3 gap-4">
				<ControlOptional v-model="s.thumbnail" label="Thumbnail" :fallback="320" unset="-t · not given: no thumbnail">
					<ControlNumber v-model="s.thumbnail" label="Thumbnail size" :min="1" unit="px" help="-t · generate thumbnail with maximum size #; none for an image already inside it" />
				</ControlOptional>
				<ControlToggle v-if="s.thumbnail !== null" :model-value="!s.thumbnailAlpha" label="Drop thumbnail alpha" help="--no-thumb-alpha · do not save alpha channel in thumbnail image" @update:model-value="(drop: boolean) => { s.thumbnailAlpha = !drop }" />
			</div>
		</ControlPanel>

		<ControlPanel title="Colour">
			<div class="grid grid-cols-3 gap-4">
				<ControlChoice v-model="profile" label="Colour profile" :items="PROFILES" help="--color-profile · the NCLX written with the image; custom uses the code points" />
				<template v-if="s.colorProfile.preset === 'custom'">
					<div class="col-span-2 grid grid-cols-2 gap-2">
						<ControlNumber v-model="s.colorProfile.matrixCoefficients" label="Matrix coefficients" :min="0" :max="14" help="--matrix_coefficients · 0, 1, 2 or 4 to 14 (see H.273); default 6" />
						<ControlNumber v-model="s.colorProfile.colourPrimaries" label="Colour primaries" :min="1" :max="22" help="--colour_primaries · 1, 2, 4 to 12 or 22 (see H.273); default 1" />
						<ControlNumber v-model="s.colorProfile.transferCharacteristics" label="Transfer characteristics" :min="1" :max="18" help="--transfer_characteristic · 1, 2 or 4 to 18 (see H.273); default 13" />
						<ControlToggle v-model="s.colorProfile.fullRange" label="Full range" help="--full_range_flag · default: 1" />
					</div>
				</template>
			</div>
			<div class="grid grid-cols-3 gap-4">
				<ControlToggle v-model="s.twoColrBoxes" label="Two colour boxes" help="--enable-two-colr-boxes · write both an ICC and an nclx color profile if both are present" />
				<ControlOptional v-model="s.clli" label="Content light level" :fallback="[1000, 400]" unset="--clli · not given">
					<template #default="{ value }">
						<ControlNumber v-model="value[0]" label="MaxCLL" :min="0" :max="65535" unit="cd/m²" help="--clli MaxCLL,MaxPALL" />
						<ControlNumber v-model="value[1]" label="MaxPALL" :min="0" :max="65535" unit="cd/m²" />
					</template>
				</ControlOptional>
			</div>
		</ControlPanel>

		<ControlPanel title="Metadata">
			<div class="grid grid-cols-3 gap-4">
				<ControlToggle v-model="s.metadata.icc" label="Keep ICC profile" help="heif-enc copies the input's colour profile; off is the same as removing it from the input first" />
				<ControlToggle v-model="s.metadata.exif" label="Keep Exif" help="heif-enc copies the input's Exif; off is the same as removing it from the input first" />
				<ControlToggle v-model="s.metadata.xmp" label="Keep XMP" help="heif-enc copies the input's XMP; off is the same as removing it from the input first" />
			</div>
		</ControlPanel>

		<ControlDisclosure title="Geometry and container">
			<div class="grid grid-cols-3 gap-4">
				<ControlSelect v-model="s.orientation" label="Rotation and mirroring" :items="ORIENTATIONS" help="--rotate-cw / --flip-h / --flip-v · signalled in the file, after a JPEG's own Exif orientation" />
				<ControlOptional v-model="s.pasp" label="Pixel aspect ratio" :fallback="[1, 1]" unset="--pasp · not given">
					<template #default="{ value }">
						<ControlNumber v-model="value[0]" label="Horizontal spacing" :min="0" help="--pasp h,v" />
						<ControlNumber v-model="value[1]" label="Vertical spacing" :min="0" />
					</template>
				</ControlOptional>
				<ControlOptional v-model="s.cutTiles" label="Cut into tiles" :fallback="512" unset="--cut-tiles · not given: one image">
					<ControlNumber v-model="s.cutTiles" label="Tile size" :min="1" unit="px" help="--cut-tiles · cuts the input image into square tiles of the given width" />
				</ControlOptional>
			</div>
			<div class="grid grid-cols-3 gap-4">
				<ControlOptional v-model="s.omafProjection" label="360° projection" fallback="equirectangular" unset="--omaf-image-projection · not given">
					<ControlChoice v-model="s.omafProjection" label="Projection" :items="PROJECTIONS" help="--omaf-image-projection" />
				</ControlOptional>
				<ControlText v-model="s.description" label="Description" placeholder="None" help="--pitm-description · set user description for primary image" />
				<div class="flex min-w-0 flex-col gap-2 px-2.5 py-[7px] text-xs">
					<span class="font-semibold text-paleday-fg">Compatible brands</span>
					<UInputTags v-model="brands" :max-length="4" placeholder="e.g. mif2" variant="soft" size="sm" aria-label="Compatible brands" :ui="{ root: 'bg-paleday-rule' }" />
					<p class="text-paleday-dim">
						--add-compatible-brand · add a compatible brand to the output file (4 characters each)
					</p>
				</div>
			</div>
			<div class="grid grid-cols-3 gap-4">
				<ControlToggle v-model="s.unif" label="Unified IDs" help="--unif · use unified ID namespace (adds 'unif' compatible brand)" />
				<ControlToggle v-model="s.mini" label="Compact format" help="--mini · use compact 'mini' box format, when the image allows it" />
			</div>
		</ControlDisclosure>
	</div>
</template>

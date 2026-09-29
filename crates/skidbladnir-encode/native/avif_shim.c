// avifenc's still-image path, as a function.
//
// Skidbladnir encodes AVIF with libavif and libaom, and promises the same file `avifenc`
// writes for the same image and options. `avifenc` is more than a front end to
// `avifEncoderAddImage`: between reading the file and finishing the encoder it decides the
// pixel format, the depth, the colour signalling, the lossless defaults, tiling and
// quantizer defaults, the transforms, grid splitting, layering and the target-size
// search. This file is that part of apps/avifenc.c (libavif v1.4.2) and of
// apps/shared/avifpng.c and avifutil.c, ported for one still input whose pixels and
// metadata the Rust side has already read the way avifpng.c reads them. Line references
// are to those files. Written in C, beside libavif, so that nothing of libavif's large
// structs has to be mirrored in Rust: the Rust side sees only the two plain structs below.
//
// Portions copyright 2019-2025 Joe Drago and the libavif authors, BSD-2-Clause; see
// libavif's LICENSE in third_party/libavif.

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <stdarg.h>

#include "jpeglib.h"

#include "avif/avif.h"
#include "avifexif.h"
#include "iccmaker.h"

// avifenc's JPEG reader, apps/shared/avifjpeg.c, is compiled into this file as it is, but
// reading from memory: the FILE it is handed is really a Memory, and libjpeg's stdio source
// becomes its memory source. It keeps a JPEG's YCbCr planes when the settings allow, which
// no decode to RGB can reproduce. What it prints goes nowhere, except that its last error
// is kept to report.
#if defined(_MSC_VER)
#define SKID_THREAD_LOCAL __declspec(thread)
#else
#define SKID_THREAD_LOCAL _Thread_local
#endif
typedef struct Memory {
	const unsigned char * data;
	size_t size;
} Memory;
static SKID_THREAD_LOCAL char jpegMessage[256];
static int jpegReport(const char * format, ...)
{
	va_list args;
	va_start(args, format);
	vsnprintf(jpegMessage, sizeof(jpegMessage), format, args);
	va_end(args);
	return 0;
}
static int jpegSilent(const char * format, ...)
{
	(void)format;
	return 0;
}
// avifutil.c's, which cannot come along: it reads every other format too.
void avifImageFixXMP(avifImage * image)
{
	if (image->xmp.size >= 2 && image->xmp.data[image->xmp.size - 1] == '\0' && image->xmp.data[image->xmp.size - 2] != '\0') {
		--image->xmp.size;
	}
}
#define jpeg_stdio_src(cinfo, file) jpeg_mem_src((cinfo), ((const Memory *)(const void *)(file))->data, (unsigned long)((const Memory *)(const void *)(file))->size)
#define fprintf(stream, ...) jpegReport(__VA_ARGS__)
#define printf(...) jpegSilent(__VA_ARGS__)
#include "avifjpeg.c"
#undef jpeg_stdio_src
#undef fprintf
#undef printf
// avifutil.c's too, referenced only by avifJPEGWrite, which comes along with the reader
// and is never called: the app writes no JPEG. MSVC's linker wants it defined anyway.
avifResult avifApplyTransforms(avifRGBImage * dstView, avifRGBImage * srcImage, const avifImage * avif)
{
	(void)dstView;
	(void)srcImage;
	(void)avif;
	return AVIF_RESULT_NOT_IMPLEMENTED;
}

#define INVALID_QUALITY (-1)
#define DEFAULT_QUALITY 60
#define PROGRESSIVE_WORST_QUALITY 10
#define PROGRESSIVE_START_QUALITY 2

typedef struct SkidAvifOption {
	const char * key;
	const char * value;
} SkidAvifOption;

// The image, as avifPNGReadImpl has it after png_read_image, and the colour and metadata
// it found. Anything an --ignore-* option or an override removed is already absent.
typedef struct SkidAvifInput {
	uint32_t width;
	uint32_t height;
	const void * pixels; // Interleaved, rows tight; 16-bit samples in native byte order.
	uint32_t rgb_depth;  // 8 or 16.
	uint32_t channels;   // 1 gray, 2 gray+alpha, 3 RGB, 4 RGBA.
	int raw_gray;        // The source's colour type was gray (avifPNGReadImpl rawColorTypeIsGray).
	int has_cicp;
	uint8_t cicp_primaries;
	uint8_t cicp_transfer;
	const uint8_t * icc;
	size_t icc_size;
	int has_srgb;
	int has_gama;
	double gama; // png_get_gAMA: the file value / 100000.
	int has_chrm;
	double chrm[8]; // png_get_cHRM order: white x, y, red x, y, green x, y, blue x, y.
	const uint8_t * exif;
	size_t exif_size;
	const uint8_t * xmp;
	size_t xmp_size;
	int exif_sets_transforms; // A JPEG's Exif: its orientation becomes irot/imir (avifjpeg.c).
	// A JPEG file to read with avifenc's reader instead of everything above, and which of
	// its metadata --ignore-* (or an override) leaves out.
	const uint8_t * jpeg;
	size_t jpeg_size;
	int ignore_icc;
	int ignore_exif;
	int ignore_xmp;
} SkidAvifInput;

// avifenc's options. -1 (or 0 for the flags) means "not given on the command line".
typedef struct SkidAvifSettings {
	int jobs;
	int speed;
	int quality;
	int quality_alpha;
	int min_quantizer;
	int max_quantizer;
	int min_quantizer_alpha;
	int max_quantizer_alpha;
	int tile_rows_log2;
	int tile_cols_log2;
	int autotiling;
	int scaling_set;
	uint32_t scaling_n;
	uint32_t scaling_d;
	int lossless;
	int depth;
	int depth_extension;
	int yuv_format; // -1 auto, else avifPixelFormat.
	int premultiply;
	int sharpyuv;
	int cicp_set;
	int primaries;
	int transfer;
	int matrix;
	int range_limited;
	int target_size;
	int progressive;
	uint32_t grid_cols; // 0 for no grid.
	uint32_t grid_rows;
	int pasp_set;
	uint32_t pasp[2];
	int crop_set;
	uint32_t crop[4];
	int clap_set;
	uint32_t clap[8];
	int irot; // -1 unset.
	int imir; // -1 unset.
	int clli_set;
	uint32_t clli[2];
	const uint8_t * icc_override;
	size_t icc_override_size;
	const uint8_t * exif_override;
	size_t exif_override_size;
	const uint8_t * xmp_override;
	size_t xmp_override_size;
	const SkidAvifOption * advanced;
	size_t advanced_count;
} SkidAvifSettings;

typedef struct Entry {
	int value;
	int set;
} Entry;

// avifInputFileSettings for the one input.
typedef struct FileSettings {
	Entry quality, qualityAlpha, minQuantizer, maxQuantizer, minQuantizerAlpha, maxQuantizerAlpha, tileRowsLog2, tileColsLog2, autoTiling;
	int scalingSet;
	avifScalingMode scalingMode;
} FileSettings;

static Entry entryOf(int value)
{
	Entry entry = { value, 1 };
	return entry;
}

static void fail(char * error, size_t size, const char * message)
{
	snprintf(error, size, "%s", message);
}

static int quantizerToQuality(int minQuantizer, int maxQuantizer)
{
	const int quantizer = (minQuantizer + maxQuantizer) / 2;
	return ((63 - quantizer) * 100 + 31) / 63;
}

// avifutil.c: avifGetBestCellSize.
static avifBool bestCellSize(uint32_t numPixels, uint32_t numCells, avifBool isSubsampled, uint32_t * cellSize)
{
	*cellSize = (uint32_t)(((uint64_t)numPixels + numCells - 1) / numCells);
	if (*cellSize < 64) {
		*cellSize = 64;
		if ((uint64_t)(numCells - 1) * *cellSize >= (uint64_t)numPixels) {
			return AVIF_FALSE;
		}
	}
	if (*cellSize > 65536) {
		return AVIF_FALSE;
	}
	if (isSubsampled && (*cellSize & 1)) {
		++*cellSize;
		if ((uint64_t)(numCells - 1) * *cellSize >= (uint64_t)numPixels) {
			return AVIF_FALSE;
		}
	}
	return AVIF_TRUE;
}

// avifutil.c: avifImageSplitGrid, without the gain map, which avifenc here never has.
static avifBool splitGrid(const avifImage * image, uint32_t gridCols, uint32_t gridRows, avifImage ** cells)
{
	avifPixelFormatInfo formatInfo;
	avifGetPixelFormatInfo(image->yuvFormat, &formatInfo);
	const avifBool isSubsampledX = !formatInfo.monochrome && formatInfo.chromaShiftX;
	const avifBool isSubsampledY = !formatInfo.monochrome && formatInfo.chromaShiftY;
	uint32_t cellWidth, cellHeight;
	if (!bestCellSize(image->width, gridCols, isSubsampledX, &cellWidth) || !bestCellSize(image->height, gridRows, isSubsampledY, &cellHeight)) {
		return AVIF_FALSE;
	}
	for (uint32_t gridY = 0; gridY < gridRows; ++gridY) {
		for (uint32_t gridX = 0; gridX < gridCols; ++gridX) {
			const uint32_t index = gridX + gridY * gridCols;
			cells[index] = avifImageCreateEmpty();
			if (!cells[index]) {
				return AVIF_FALSE;
			}
			avifCropRect rect = { gridX * cellWidth, gridY * cellHeight, cellWidth, cellHeight };
			if (rect.x + rect.width > image->width) {
				rect.width = image->width - rect.x;
			}
			if (rect.y + rect.height > image->height) {
				rect.height = image->height - rect.y;
			}
			if (avifImageSetViewRect(cells[index], image, &rect) != AVIF_RESULT_OK) {
				return AVIF_FALSE;
			}
		}
	}
	if (image->icc.size > 0 && avifImageSetProfileICC(cells[0], image->icc.data, image->icc.size) != AVIF_RESULT_OK) {
		return AVIF_FALSE;
	}
	if (image->exif.size > 0 && avifRWDataSet(&cells[0]->exif, image->exif.data, image->exif.size) != AVIF_RESULT_OK) {
		return AVIF_FALSE;
	}
	if (image->xmp.size > 0 && avifImageSetMetadataXMP(cells[0], image->xmp.data, image->xmp.size) != AVIF_RESULT_OK) {
		return AVIF_FALSE;
	}
	return AVIF_TRUE;
}

// avifenc.c: avifEncodeImagesFixedQuality and avifEncodeRestOfLayeredImage, for one input.
static avifBool encodeFixedQuality(const SkidAvifSettings * s,
                                   const FileSettings * file,
                                   int layers,
                                   int overrideQuality,
                                   int overrideQualityAlpha,
                                   const avifImage * image,
                                   avifImage * const * gridCells,
                                   avifRWData * encoded,
                                   char * error,
                                   size_t errorSize)
{
	avifBool success = AVIF_FALSE;
	avifRWDataFree(encoded);
	avifEncoder * encoder = avifEncoderCreate();
	if (!encoder) {
		fail(error, errorSize, "out of memory");
		return AVIF_FALSE;
	}
	encoder->maxThreads = s->jobs;
	encoder->codecChoice = AVIF_CODEC_CHOICE_AUTO;
	encoder->speed = s->speed;
	encoder->timescale = 30;
	encoder->keyframeInterval = 0;
	encoder->repetitionCount = AVIF_REPETITION_COUNT_INFINITE;
	encoder->headerFormat = AVIF_HEADER_DEFAULT;
	encoder->creationTime = 0;
	encoder->modificationTime = 0;
	encoder->extraLayerCount = layers - 1;

	// avifEncodeUpdateEncoderSettings.
	if (file->quality.set) encoder->quality = file->quality.value;
	if (file->qualityAlpha.set) encoder->qualityAlpha = file->qualityAlpha.value;
	if (file->minQuantizer.set) encoder->minQuantizer = file->minQuantizer.value;
	if (file->maxQuantizer.set) encoder->maxQuantizer = file->maxQuantizer.value;
	if (file->minQuantizerAlpha.set) encoder->minQuantizerAlpha = file->minQuantizerAlpha.value;
	if (file->maxQuantizerAlpha.set) encoder->maxQuantizerAlpha = file->maxQuantizerAlpha.value;
	if (file->tileRowsLog2.set) encoder->tileRowsLog2 = file->tileRowsLog2.value;
	if (file->tileColsLog2.set) encoder->tileColsLog2 = file->tileColsLog2.value;
	if (file->autoTiling.set) encoder->autoTiling = file->autoTiling.value;
	if (file->scalingSet) encoder->scalingMode = file->scalingMode;
	for (size_t i = 0; i < s->advanced_count; ++i) {
		if (avifEncoderSetCodecSpecificOption(encoder, s->advanced[i].key, s->advanced[i].value) != AVIF_RESULT_OK) {
			snprintf(error, errorSize, "failed to set codec specific option: %s = %s", s->advanced[i].key, s->advanced[i].value);
			goto cleanup;
		}
	}

	if (overrideQuality != INVALID_QUALITY) encoder->quality = overrideQuality;
	if (overrideQualityAlpha != INVALID_QUALITY) encoder->qualityAlpha = overrideQualityAlpha;

	if (s->depth == 8 && s->depth_extension == 8) {
		encoder->sampleTransformRecipe = AVIF_SAMPLE_TRANSFORM_BIT_DEPTH_EXTENSION_8B_8B;
	} else if (s->depth == 12 && s->depth_extension == 4) {
		encoder->sampleTransformRecipe = AVIF_SAMPLE_TRANSFORM_BIT_DEPTH_EXTENSION_12B_4B;
	} else if (s->depth == 12 && s->depth_extension == 8) {
		encoder->sampleTransformRecipe = AVIF_SAMPLE_TRANSFORM_BIT_DEPTH_EXTENSION_12B_8B_OVERLAP_4B;
	} else if (s->depth_extension != 0) {
		fail(error, errorSize, "unsupported bit depth extension");
		goto cleanup;
	}

	if (s->progressive) {
		encoder->quality = 10;
		const avifScalingMode half = { { 1, 2 }, { 1, 2 } };
		encoder->scalingMode = half;
	}

	avifResult result;
	if (gridCells) {
		result = avifEncoderAddImageGrid(encoder, s->grid_cols, s->grid_rows, (const avifImage * const *)gridCells, AVIF_ADD_IMAGE_FLAG_SINGLE);
	} else {
		result = avifEncoderAddImage(encoder, image, 1, layers == 1 ? AVIF_ADD_IMAGE_FLAG_SINGLE : AVIF_ADD_IMAGE_FLAG_NONE);
		if (result == AVIF_RESULT_OK && layers > 1) {
			// avifEncodeRestOfLayeredImage, the --progressive case: the last layer reaches
			// the target quality, unscaled.
			const int targetQuality = overrideQuality != INVALID_QUALITY ? overrideQuality : file->quality.value;
			for (int layerIndex = 1; layerIndex < layers && result == AVIF_RESULT_OK; ++layerIndex) {
				encoder->quality = targetQuality - (targetQuality - PROGRESSIVE_START_QUALITY) * (encoder->extraLayerCount - layerIndex) / encoder->extraLayerCount;
				const avifScalingMode none = { { 1, 1 }, { 1, 1 } };
				encoder->scalingMode = none;
				result = avifEncoderAddImage(encoder, image, 1, AVIF_ADD_IMAGE_FLAG_NONE);
			}
		}
	}
	if (result != AVIF_RESULT_OK) {
		snprintf(error, errorSize, "failed to encode image: %s (%s)", avifResultToString(result), encoder->diag.error);
		goto cleanup;
	}
	result = avifEncoderFinish(encoder, encoded);
	if (result != AVIF_RESULT_OK) {
		snprintf(error, errorSize, "failed to finish encoding: %s (%s)", avifResultToString(result), encoder->diag.error);
		goto cleanup;
	}
	success = AVIF_TRUE;
cleanup:
	avifEncoderDestroy(encoder);
	return success;
}

const char * skid_avif_versions(void)
{
	static char versions[256];
	avifCodecVersions(versions);
	return versions;
}

const char * skid_avif_version(void)
{
	return avifVersion();
}

void skid_avif_free(uint8_t * data)
{
	avifFree(data);
}

// Encode one still image as avifenc would. On success, *out and *out_size hold the file,
// to be released with skid_avif_free; on failure, error holds why and 0 is returned.
int skid_avif_encode(const SkidAvifInput * in, const SkidAvifSettings * s, uint8_t ** out, size_t * out_size, char * error, size_t error_size)
{
	int ok = 0;
	avifImage * image = NULL;
	avifImage ** gridCells = NULL;
	uint32_t gridCellCount = 0;
	avifRGBImage rgb;
	memset(&rgb, 0, sizeof(rgb));
	avifRWData raw = AVIF_DATA_EMPTY;
	*out = NULL;
	*out_size = 0;

	int requestedFormat = s->yuv_format < 0 ? AVIF_PIXEL_FORMAT_NONE : s->yuv_format;
	avifMatrixCoefficients matrixCoefficients = s->cicp_set ? (avifMatrixCoefficients)s->matrix : AVIF_MATRIX_COEFFICIENTS_BT601;
	const avifRange requestedRange = s->range_limited ? AVIF_RANGE_LIMITED : AVIF_RANGE_FULL;

	// The input's settings, with avifenc's first-input defaults (main, 2020-2320).
	FileSettings file;
	memset(&file, 0, sizeof(file));
	if (s->quality >= 0) file.quality = entryOf(s->quality);
	if (s->quality_alpha >= 0) file.qualityAlpha = entryOf(s->quality_alpha);
	if (s->min_quantizer >= 0) file.minQuantizer = entryOf(s->min_quantizer);
	if (s->max_quantizer >= 0) file.maxQuantizer = entryOf(s->max_quantizer);
	if (s->min_quantizer_alpha >= 0) file.minQuantizerAlpha = entryOf(s->min_quantizer_alpha);
	if (s->max_quantizer_alpha >= 0) file.maxQuantizerAlpha = entryOf(s->max_quantizer_alpha);
	if (s->tile_rows_log2 >= 0) file.tileRowsLog2 = entryOf(s->tile_rows_log2);
	if (s->tile_cols_log2 >= 0) file.tileColsLog2 = entryOf(s->tile_cols_log2);
	if (s->autotiling) file.autoTiling = entryOf(AVIF_TRUE);
	if (s->scaling_set) {
		const avifFraction fraction = { (int32_t)s->scaling_n, (int32_t)s->scaling_d };
		file.scalingSet = 1;
		file.scalingMode.horizontal = fraction;
		file.scalingMode.vertical = fraction;
	}
	// "Alpha quality defaults to the same value as (color) quality until the first time
	// it's explicitly set."
	if (!file.qualityAlpha.set) {
		file.qualityAlpha = file.quality;
	}

	if (s->lossless) {
		if (requestedFormat != AVIF_PIXEL_FORMAT_NONE && requestedFormat != AVIF_PIXEL_FORMAT_YUV444 && requestedFormat != AVIF_PIXEL_FORMAT_YUV400) {
			fail(error, error_size, "when set, the pixel format can only be 4:4:4 in lossless mode (4:0:0 for grayscale)");
			goto cleanup;
		}
		if (requestedRange != AVIF_RANGE_FULL) {
			fail(error, error_size, "range has to be full in lossless mode");
			goto cleanup;
		}
		if (s->cicp_set) {
			if (matrixCoefficients != AVIF_MATRIX_COEFFICIENTS_IDENTITY && matrixCoefficients != AVIF_MATRIX_COEFFICIENTS_YCGCO_RE && matrixCoefficients != AVIF_MATRIX_COEFFICIENTS_YCGCO_RO) {
				fail(error, error_size, "matrix coefficients have to be identity, YCgCo-Re or YCgCo-Ro in lossless mode");
				goto cleanup;
			}
		} else {
			matrixCoefficients = AVIF_MATRIX_COEFFICIENTS_IDENTITY;
		}
	}

	const int layers = s->progressive ? 2 : 1;
	if (layers > 1 && s->grid_cols) {
		fail(error, error_size, "a layered grid image is not implemented in avifenc");
		goto cleanup;
	}

	if (file.autoTiling.set) {
		if (file.tileRowsLog2.set || file.tileColsLog2.set) {
			fail(error, error_size, "automatic tiling cannot be combined with tile rows or columns");
			goto cleanup;
		}
		file.tileRowsLog2 = entryOf(0);
		file.tileColsLog2 = entryOf(0);
	} else if (file.tileColsLog2.set || file.tileRowsLog2.set) {
		file.autoTiling = entryOf(AVIF_FALSE);
	}

	avifBool qualityIsConstrained = AVIF_FALSE, qualityAlphaIsConstrained = AVIF_FALSE;
	if (s->lossless) {
		if ((file.quality.set && file.quality.value != AVIF_QUALITY_LOSSLESS) || (file.qualityAlpha.set && file.qualityAlpha.value != AVIF_QUALITY_LOSSLESS)) {
			fail(error, error_size, "quality cannot be set in lossless mode, except to 100");
			goto cleanup;
		}
		if ((file.minQuantizer.set && file.minQuantizer.value != AVIF_QUANTIZER_LOSSLESS) || (file.maxQuantizer.set && file.maxQuantizer.value != AVIF_QUANTIZER_LOSSLESS) || (file.minQuantizerAlpha.set && file.minQuantizerAlpha.value != AVIF_QUANTIZER_LOSSLESS) || (file.maxQuantizerAlpha.set && file.maxQuantizerAlpha.value != AVIF_QUANTIZER_LOSSLESS)) {
			fail(error, error_size, "quantizers cannot be set in lossless mode, except to 0");
			goto cleanup;
		}
	} else if (s->progressive && file.quality.set && file.quality.value < PROGRESSIVE_WORST_QUALITY) {
		fail(error, error_size, "quality must be at least 10 for progressive encoding");
		goto cleanup;
	}
	if (file.minQuantizer.set != file.maxQuantizer.set || file.minQuantizerAlpha.set != file.maxQuantizerAlpha.set) {
		fail(error, error_size, "a minimum and a maximum quantizer must be given together");
		goto cleanup;
	}
	if (!file.autoTiling.set) file.autoTiling = entryOf(AVIF_TRUE);
	if (!file.tileRowsLog2.set) file.tileRowsLog2 = entryOf(0);
	if (!file.tileColsLog2.set) file.tileColsLog2 = entryOf(0);
	if (s->lossless) {
		file.quality = entryOf(AVIF_QUALITY_LOSSLESS);
		file.qualityAlpha = entryOf(AVIF_QUALITY_LOSSLESS);
		file.minQuantizer = entryOf(AVIF_QUANTIZER_LOSSLESS);
		file.maxQuantizer = entryOf(AVIF_QUANTIZER_LOSSLESS);
		file.minQuantizerAlpha = entryOf(AVIF_QUANTIZER_LOSSLESS);
		file.maxQuantizerAlpha = entryOf(AVIF_QUANTIZER_LOSSLESS);
	} else {
		qualityIsConstrained = file.quality.set;
		qualityAlphaIsConstrained = file.qualityAlpha.set;
		if (file.minQuantizer.set) {
			if (!file.quality.set) file.quality = entryOf(quantizerToQuality(file.minQuantizer.value, file.maxQuantizer.value));
		} else {
			if (!file.quality.set) file.quality = entryOf(DEFAULT_QUALITY);
			file.minQuantizer = entryOf(AVIF_QUANTIZER_BEST_QUALITY);
			file.maxQuantizer = entryOf(AVIF_QUANTIZER_WORST_QUALITY);
		}
		if (file.minQuantizerAlpha.set) {
			if (!file.qualityAlpha.set) file.qualityAlpha = entryOf(quantizerToQuality(file.minQuantizerAlpha.value, file.maxQuantizerAlpha.value));
		} else {
			if (!file.qualityAlpha.set) file.qualityAlpha = file.quality;
			file.minQuantizerAlpha = entryOf(AVIF_QUANTIZER_BEST_QUALITY);
			file.maxQuantizerAlpha = entryOf(AVIF_QUANTIZER_WORST_QUALITY);
		}
	}
	if (!file.scalingSet) {
		const avifFraction one = { 1, 1 };
		file.scalingSet = 1;
		file.scalingMode.horizontal = one;
		file.scalingMode.vertical = one;
	}

	image = avifImageCreateEmpty();
	if (!image) {
		fail(error, error_size, "out of memory");
		goto cleanup;
	}
	image->colorPrimaries = s->cicp_set ? (avifColorPrimaries)s->primaries : AVIF_COLOR_PRIMARIES_UNSPECIFIED;
	image->transferCharacteristics = s->cicp_set ? (avifTransferCharacteristics)s->transfer : AVIF_TRANSFER_CHARACTERISTICS_UNSPECIFIED;
	image->matrixCoefficients = matrixCoefficients;
	image->yuvRange = requestedRange;
	image->alphaPremultiplied = s->premultiply ? AVIF_TRUE : AVIF_FALSE;
	if (image->matrixCoefficients == AVIF_MATRIX_COEFFICIENTS_IDENTITY && requestedFormat != AVIF_PIXEL_FORMAT_NONE && requestedFormat != AVIF_PIXEL_FORMAT_YUV444) {
		image->matrixCoefficients = AVIF_MATRIX_COEFFICIENTS_BT601;
	}

	// avifPNGReadImpl, from png_read_update_info on.
	const avifColorPrimaries primariesBefore = image->colorPrimaries;
	const avifTransferCharacteristics transferBefore = image->transferCharacteristics;
	if (in->jpeg) {
		// avifReadImage for a JPEG: avifjpeg.c, with the size limit avifenc passes.
		const Memory memory = { in->jpeg, in->jpeg_size };
		const avifChromaDownsampling downsampling = s->sharpyuv ? AVIF_CHROMA_DOWNSAMPLING_SHARP_YUV : AVIF_CHROMA_DOWNSAMPLING_AUTOMATIC;
		jpegMessage[0] = '\0';
		if (!avifJPEGReadInternal((FILE *)(void *)&memory, "input", image, (avifPixelFormat)requestedFormat, (uint32_t)(s->depth_extension == 0 ? s->depth : 16), downsampling, in->ignore_icc ? AVIF_TRUE : AVIF_FALSE, in->ignore_exif ? AVIF_TRUE : AVIF_FALSE, in->ignore_xmp ? AVIF_TRUE : AVIF_FALSE, s->progressive ? AVIF_TRUE : AVIF_FALSE, UINT32_MAX)) {
			snprintf(error, error_size, "avifenc cannot read this JPEG: %s", jpegMessage[0] ? jpegMessage : "unknown error");
			// Trim the newline avifenc's messages end with.
			const size_t length = strlen(error);
			if (length && error[length - 1] == '\n') error[length - 1] = '\0';
			goto cleanup;
		}
	} else {
		image->width = in->width;
		image->height = in->height;
		image->yuvFormat = (avifPixelFormat)requestedFormat;
		if (image->matrixCoefficients == AVIF_MATRIX_COEFFICIENTS_YCGCO_RO) {
			fail(error, error_size, "YCgCo-Ro cannot be used with this input, because it has an even bit depth");
			goto cleanup;
		}
		if (image->yuvFormat == AVIF_PIXEL_FORMAT_NONE) {
			if (in->raw_gray) {
				image->yuvFormat = AVIF_PIXEL_FORMAT_YUV400;
			} else {
				image->yuvFormat = AVIF_PIXEL_FORMAT_YUV444; // Identity, YCgCo-Re and the default all pick 4:4:4.
			}
		}
		const int requestedDepth = s->depth_extension == 0 ? s->depth : 16;
		image->depth = requestedDepth;
		if (image->depth == 0) {
			image->depth = in->rgb_depth == 8 ? 8 : 12;
		}
		if (image->matrixCoefficients == AVIF_MATRIX_COEFFICIENTS_YCGCO_RE) {
			if (in->rgb_depth != 8) {
				fail(error, error_size, "YCgCo-Re cannot be used on 16-bit input, because it adds two bits");
				goto cleanup;
			}
			if (requestedDepth && requestedDepth != 10) {
				fail(error, error_size, "YCgCo-Re needs a depth of 10");
				goto cleanup;
			}
			image->depth = 10;
		}

		if (in->has_cicp) {
			image->colorPrimaries = (avifColorPrimaries)in->cicp_primaries;
			image->transferCharacteristics = (avifTransferCharacteristics)in->cicp_transfer;
		} else if (in->icc_size) {
			if (!in->raw_gray && image->yuvFormat == AVIF_PIXEL_FORMAT_YUV400) {
				fail(error, error_size, "the image has a colour ICC profile, which does not fit 4:0:0 (grayscale) output; ignore the profile to encode it anyway");
				goto cleanup;
			}
			if (in->raw_gray && image->yuvFormat != AVIF_PIXEL_FORMAT_YUV400) {
				fail(error, error_size, "the image has a gray ICC profile, which does not fit colour output; ignore the profile to encode it anyway");
				goto cleanup;
			}
			if (avifImageSetProfileICC(image, in->icc, in->icc_size) != AVIF_RESULT_OK) {
				fail(error, error_size, "out of memory");
				goto cleanup;
			}
		} else if (in->has_srgb) {
			image->colorPrimaries = AVIF_COLOR_PRIMARIES_SRGB;
			image->transferCharacteristics = AVIF_TRANSFER_CHARACTERISTICS_SRGB;
		} else {
			avifBool needToGenerateICC = AVIF_FALSE;
			double gamma = 2.2;
			float primaries[8];
			if (in->has_gama) {
				gamma = 1.0 / in->gama;
				image->transferCharacteristics = avifTransferCharacteristicsFindByGamma((float)gamma);
				if (image->transferCharacteristics == AVIF_TRANSFER_CHARACTERISTICS_UNKNOWN) {
					needToGenerateICC = AVIF_TRUE;
				}
			}
			if (in->has_chrm) {
				primaries[0] = (float)in->chrm[2];
				primaries[1] = (float)in->chrm[3];
				primaries[2] = (float)in->chrm[4];
				primaries[3] = (float)in->chrm[5];
				primaries[4] = (float)in->chrm[6];
				primaries[5] = (float)in->chrm[7];
				primaries[6] = (float)in->chrm[0];
				primaries[7] = (float)in->chrm[1];
				image->colorPrimaries = avifColorPrimariesFind(primaries, NULL);
				if (image->colorPrimaries == AVIF_COLOR_PRIMARIES_UNKNOWN) {
					needToGenerateICC = AVIF_TRUE;
				}
			} else {
				avifColorPrimariesGetValues(AVIF_COLOR_PRIMARIES_BT709, primaries);
			}
			if (needToGenerateICC) {
				image->colorPrimaries = AVIF_COLOR_PRIMARIES_UNSPECIFIED;
				image->transferCharacteristics = AVIF_TRANSFER_CHARACTERISTICS_UNSPECIFIED;
				// A generator failure is a warning in avifenc, leaving no profile.
				if (image->yuvFormat == AVIF_PIXEL_FORMAT_YUV400) {
					(void)avifGenerateGrayICC(&image->icc, (float)gamma, &primaries[6]);
				} else {
					(void)avifGenerateRGBICC(&image->icc, (float)gamma, primaries);
				}
			}
		}

		avifRGBImageSetDefaults(&rgb, image);
		rgb.chromaDownsampling = s->sharpyuv ? AVIF_CHROMA_DOWNSAMPLING_SHARP_YUV : AVIF_CHROMA_DOWNSAMPLING_AUTOMATIC;
		rgb.depth = in->rgb_depth;
		rgb.format = in->channels == 1 ? AVIF_RGB_FORMAT_GRAY : in->channels == 2 ? AVIF_RGB_FORMAT_GRAYA : in->channels == 3 ? AVIF_RGB_FORMAT_RGB : AVIF_RGB_FORMAT_RGBA;
		if (avifRGBImageAllocatePixels(&rgb) != AVIF_RESULT_OK) {
			fail(error, error_size, "out of memory");
			goto cleanup;
		}
		{
			const size_t row = (size_t)in->width * in->channels * (in->rgb_depth > 8 ? 2 : 1);
			for (uint32_t y = 0; y < in->height; ++y) {
				memcpy(rgb.pixels + (size_t)y * rgb.rowBytes, (const uint8_t *)in->pixels + (size_t)y * row, row);
			}
		}
		{
			const avifResult converted = avifImageRGBToYUV(image, &rgb);
			if (converted != AVIF_RESULT_OK) {
				snprintf(error, error_size, "conversion to YUV failed: %s", avifResultToString(converted));
				goto cleanup;
			}
		}
		if (in->exif_size) {
			// A PNG's Exif is copied as it is (avifpng.c avoids avifImageSetMetadataExif); a
			// JPEG's goes through it, so its orientation becomes irot/imir (avifjpeg.c).
			const avifResult set = in->exif_sets_transforms ? avifImageSetMetadataExif(image, in->exif, in->exif_size) : avifRWDataSet(&image->exif, in->exif, in->exif_size);
			if (set != AVIF_RESULT_OK) {
				fail(error, error_size, "out of memory");
				goto cleanup;
			}
			// Both readers then reset the Exif orientation to 1; errors are ignored.
			(void)avifSetExifOrientation(&image->exif, 1);
		}
		if (in->xmp_size) {
			if (avifImageSetMetadataXMP(image, in->xmp, in->xmp_size) != AVIF_RESULT_OK) {
				fail(error, error_size, "out of memory");
				goto cleanup;
			}
			// avifImageFixXMP.
			if (image->xmp.size >= 2 && image->xmp.data[image->xmp.size - 1] == '\0' && image->xmp.data[image->xmp.size - 2] != '\0') {
				--image->xmp.size;
			}
		}
	}
	// avifInputReadImage with --target-size caches the image it read and hands the encoder
	// a view of it (avifImageSetViewRect), which copies no property that needs an
	// allocation: the input's own ICC profile, Exif and XMP do not survive. The overrides,
	// set below on the view itself, do.
	if (s->target_size >= 0) {
		avifRWDataFree(&image->icc);
		avifRWDataFree(&image->exif);
		avifRWDataFree(&image->xmp);
	}
	// avifInputReadImage: avifenc keeps the CICP it was given explicitly.
	if (s->cicp_set) {
		image->colorPrimaries = primariesBefore;
		image->transferCharacteristics = transferBefore;
	}

	// Back in main.
	if (image->matrixCoefficients == AVIF_MATRIX_COEFFICIENTS_IDENTITY && image->yuvFormat == AVIF_PIXEL_FORMAT_YUV400) {
		image->matrixCoefficients = AVIF_MATRIX_COEFFICIENTS_BT601;
	}
	if (image->matrixCoefficients == AVIF_MATRIX_COEFFICIENTS_IDENTITY && image->yuvFormat != AVIF_PIXEL_FORMAT_YUV444) {
		fail(error, error_size, "matrix coefficients may not be identity when subsampling");
		goto cleanup;
	}
	if ((s->icc_override_size && avifImageSetProfileICC(image, s->icc_override, s->icc_override_size) != AVIF_RESULT_OK) || (s->exif_override_size && avifImageSetMetadataExif(image, s->exif_override, s->exif_override_size) != AVIF_RESULT_OK) || (s->xmp_override_size && avifImageSetMetadataXMP(image, s->xmp_override, s->xmp_override_size) != AVIF_RESULT_OK)) {
		fail(error, error_size, "could not set the metadata given");
		goto cleanup;
	}
	if (!image->icc.size && !s->cicp_set && image->colorPrimaries == AVIF_COLOR_PRIMARIES_UNSPECIFIED && image->transferCharacteristics == AVIF_TRANSFER_CHARACTERISTICS_UNSPECIFIED) {
		image->colorPrimaries = AVIF_COLOR_PRIMARIES_SRGB;
		image->transferCharacteristics = AVIF_TRANSFER_CHARACTERISTICS_SRGB;
	}
	if (s->pasp_set) {
		image->transformFlags |= AVIF_TRANSFORM_PASP;
		image->pasp.hSpacing = s->pasp[0];
		image->pasp.vSpacing = s->pasp[1];
	}
	{
		uint32_t clap[8];
		avifBool clapValid = AVIF_FALSE;
		if (s->clap_set) {
			memcpy(clap, s->clap, sizeof(clap));
			clapValid = AVIF_TRUE;
		}
		if (s->crop_set) {
			// convertCropToClap.
			avifCleanApertureBox box;
			avifCropRect rect = { s->crop[0], s->crop[1], s->crop[2], s->crop[3] };
			avifDiagnostics diag;
			avifDiagnosticsClearError(&diag);
			if (!avifCleanApertureBoxFromCropRect(&box, &rect, image->width, image->height, &diag)) {
				snprintf(error, error_size, "impossible crop rectangle: %s", diag.error);
				goto cleanup;
			}
			const uint32_t values[8] = { box.widthN, box.widthD, box.heightN, box.heightD, box.horizOffN, box.horizOffD, box.vertOffN, box.vertOffD };
			memcpy(clap, values, sizeof(clap));
			clapValid = AVIF_TRUE;
		}
		if (clapValid) {
			image->transformFlags |= AVIF_TRANSFORM_CLAP;
			image->clap.widthN = clap[0];
			image->clap.widthD = clap[1];
			image->clap.heightN = clap[2];
			image->clap.heightD = clap[3];
			image->clap.horizOffN = clap[4];
			image->clap.horizOffD = clap[5];
			image->clap.vertOffN = clap[6];
			image->clap.vertOffD = clap[7];
			avifCropRect rect;
			avifDiagnostics diag;
			avifDiagnosticsClearError(&diag);
			if (!avifCropRectFromCleanApertureBox(&rect, &image->clap, image->width, image->height, &diag)) {
				snprintf(error, error_size, "invalid clean aperture: %s", diag.error);
				goto cleanup;
			}
		}
	}
	if (s->irot >= 0) {
		image->transformFlags |= AVIF_TRANSFORM_IROT;
		image->irot.angle = (uint8_t)s->irot;
	}
	if (s->imir >= 0) {
		image->transformFlags |= AVIF_TRANSFORM_IMIR;
		image->imir.axis = (uint8_t)s->imir;
	}
	if (s->clli_set) {
		image->clli.maxCLL = (uint16_t)s->clli[0];
		image->clli.maxPALL = (uint16_t)s->clli[1];
	}

	if (s->grid_cols) {
		gridCellCount = s->grid_cols * s->grid_rows;
		gridCells = calloc(gridCellCount, sizeof(avifImage *));
		if (!gridCells) {
			fail(error, error_size, "out of memory");
			goto cleanup;
		}
		if (!splitGrid(image, s->grid_cols, s->grid_rows, gridCells)) {
			fail(error, error_size, "the image cannot be split into that grid (each cell must be at least 64 pixels, and even where the chroma is subsampled)");
			goto cleanup;
		}
	}

	// avifEncodeImages.
	if (s->target_size < 0) {
		if (!encodeFixedQuality(s, &file, layers, INVALID_QUALITY, INVALID_QUALITY, image, gridCells, &raw, error, error_size)) {
			goto cleanup;
		}
	} else {
		if (qualityIsConstrained && qualityAlphaIsConstrained) {
			fail(error, error_size, "a target size needs colour or alpha quality left unset");
			goto cleanup;
		}
		const size_t targetSize = (size_t)s->target_size;
		int closestQuality = INVALID_QUALITY;
		avifRWData closest = AVIF_DATA_EMPTY;
		size_t closestDiff = 0;
		int minQuality = s->progressive ? PROGRESSIVE_WORST_QUALITY : AVIF_QUALITY_WORST;
		int maxQuality = AVIF_QUALITY_BEST;
		int overrideQuality = INVALID_QUALITY, overrideQualityAlpha = INVALID_QUALITY;
		avifBool exact = AVIF_FALSE;
		while (minQuality <= maxQuality) {
			const int quality = (minQuality + maxQuality) / 2;
			if (!qualityIsConstrained) overrideQuality = quality;
			if (!qualityAlphaIsConstrained) overrideQualityAlpha = quality;
			if (!encodeFixedQuality(s, &file, layers, overrideQuality, overrideQualityAlpha, image, gridCells, &raw, error, error_size)) {
				avifRWDataFree(&closest);
				goto cleanup;
			}
			if (raw.size == targetSize) {
				exact = AVIF_TRUE;
				break;
			}
			size_t diff;
			if (raw.size > targetSize) {
				diff = raw.size - targetSize;
				maxQuality = quality - 1;
			} else {
				diff = targetSize - raw.size;
				minQuality = quality + 1;
			}
			if (closestQuality == INVALID_QUALITY || diff < closestDiff) {
				closestQuality = quality;
				avifRWDataFree(&closest);
				closest = raw;
				raw.data = NULL;
				raw.size = 0;
				closestDiff = diff;
			}
		}
		if (exact) {
			avifRWDataFree(&closest);
		} else {
			avifRWDataFree(&raw);
			raw = closest;
		}
	}

	*out = raw.data;
	*out_size = raw.size;
	raw.data = NULL;
	ok = 1;

cleanup:
	if (gridCells) {
		for (uint32_t i = 0; i < gridCellCount; ++i) {
			if (gridCells[i]) avifImageDestroy(gridCells[i]);
		}
		free(gridCells);
	}
	if (image) avifImageDestroy(image);
	avifRGBImageFreePixels(&rgb);
	avifRWDataFree(&raw);
	return ok;
}

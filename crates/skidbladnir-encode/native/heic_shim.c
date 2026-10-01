/*
 * heif-enc's still-image path (libheif 1.23.5, examples/heif_enc.cc) as one function.
 *
 * Rust reads a PNG the way heif-enc's reader (heifio) does and hands over the samples and
 * metadata. This makes the same libheif calls heif-enc makes after loading one image,
 * with the same values and in the same order, since the order decides item IDs and box
 * order in the file: context and its unif/mini flags, the encoder by name, quality, then
 * the -p parameters, the encoding options, the NCLX profile of --color-profile, the
 * premultiplied flag, the encode (or the --cut-tiles grid), the properties set on the
 * handle, Exif, XMP, the thumbnail, the udes description, the brands, and the write.
 *
 * A JPEG is not read in Rust: heif-enc's own reader does it (native/heic_jpeg.cc), since
 * it keeps the JPEG's YCbCr planes where it can.
 */

#include <libheif/heif.h>
#include <libheif/heif_items.h>
#include <libheif/heif_properties.h>

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
	uint32_t width;
	uint32_t height;
	/* 0: monochrome, Y and optional alpha planes. 1: interleaved RGB or RGBA. 2: YCbCr
	   4:2:0 planes and optional alpha, as heifio/decoder_webp.cc reads a lossy WebP. */
	int layout;
	int has_alpha;
	/* layout 0: Y, alpha. layout 1: the interleaved plane. layout 2: Y, Cb, Cr, alpha. */
	const uint8_t* planes[4];
	size_t strides[4];
	const uint8_t* icc;
	size_t icc_size;
	const uint8_t* exif;
	size_t exif_size;
	const uint8_t* xmp;
	size_t xmp_size;
	/* The Exif orientation a JPEG carries, applied before the settings' own. */
	int orientation;
	/* A JPEG file to read with heif-enc's reader instead of the fields above (but the
	   size, which must match). */
	const uint8_t* jpeg;
	size_t jpeg_size;
	/* The GPL edition's 10-bit input: each 8-bit sample widened to 10 bits (no JPEG). */
	int ten_bit;
} SkidHeicInput;

int skid_heic_load_jpeg(const unsigned char* data, size_t size, void** holder, struct heif_image** out, unsigned char** exif, size_t* exif_size, unsigned char** xmp, size_t* xmp_size, int* orientation, char* error, size_t error_size);
void skid_heic_jpeg_release(void* holder);
void skid_heic_jpeg_free(unsigned char* data);

/* An input image and what keeps it alive. */
typedef struct {
	struct heif_image* image;
	void* holder;
	unsigned char* exif;
	size_t exif_size;
	unsigned char* xmp;
	size_t xmp_size;
	int orientation;
} Loaded;

static void release(Loaded* loaded)
{
	if (loaded->holder) {
		skid_heic_jpeg_release(loaded->holder);
	}
	else if (loaded->image) {
		heif_image_release(loaded->image);
	}
	skid_heic_jpeg_free(loaded->exif);
	skid_heic_jpeg_free(loaded->xmp);
	memset(loaded, 0, sizeof(*loaded));
}

/* One -p NAME=VALUE. */
typedef struct {
	const char* name;
	const char* value;
} SkidHeicParameter;

typedef struct {
	/* The encoder's ID, or NULL for libheif's first HEVC encoder. */
	const char* encoder;
	int quality;
	/* heif-enc's -L: libheif's lossless mode and the NCLX it implies (x265 only). */
	int lossless;
	/* -p NAME=VALUE, in order. */
	const SkidHeicParameter* parameters;
	size_t parameter_count;
	int alpha;
	int premultiplied;
	int thumbnail;
	int thumbnail_alpha;
	/* 0 when -C is not given, otherwise a heif_chroma_downsampling_algorithm. */
	int chroma_downsampling;
	/* 0 custom, 1 auto, 2 601, 3 709 or compatible, 4 2020. */
	int color_profile;
	int matrix_coefficients;
	int colour_primaries;
	int transfer_characteristics;
	int full_range;
	int two_colr_boxes;
	int clli_set;
	uint16_t clli[2];
	int pasp_set;
	uint32_t pasp[2];
	int orientation;
	int cut_tiles;
	/* -1 for none, otherwise a heif_omaf_image_projection. */
	int omaf_projection;
	const char* description;
	/* brand_count four-byte brands, back to back. */
	const char* brands;
	size_t brand_count;
	int unif;
	int mini;
} SkidHeicSettings;

typedef struct {
	uint8_t* data;
	size_t size;
	size_t capacity;
} Buffer;

static struct heif_error write_buffer(struct heif_context* ctx, const void* data, size_t size, void* userdata)
{
	(void) ctx;
	Buffer* buffer = (Buffer*) userdata;
	struct heif_error ok = {heif_error_Ok, heif_suberror_Unspecified, "Success"};
	struct heif_error oom = {heif_error_Memory_allocation_error, heif_suberror_Unspecified, "out of memory"};
	if (buffer->size + size > buffer->capacity) {
		size_t capacity = buffer->capacity ? buffer->capacity : 4096;
		while (capacity < buffer->size + size) {
			capacity *= 2;
		}
		uint8_t* grown = (uint8_t*) realloc(buffer->data, capacity);
		if (!grown) {
			return oom;
		}
		buffer->data = grown;
		buffer->capacity = capacity;
	}
	memcpy(buffer->data + buffer->size, data, size);
	buffer->size += size;
	return ok;
}

static int fail(char* error, size_t error_size, const char* what, struct heif_error err)
{
	snprintf(error, error_size, "%s: %s", what, err.message ? err.message : "unknown error");
	return 0;
}

static void copy_plane(uint8_t* to, size_t to_stride, const uint8_t* from, size_t from_stride, size_t row, uint32_t rows)
{
	for (uint32_t y = 0; y < rows; y++) {
		memcpy(to + y * to_stride, from + y * from_stride, row);
	}
}

/* An 8-bit sample widened to 10 bits by repeating its top bits. */
static uint16_t widen(uint8_t value)
{
	return (uint16_t) ((value << 2) | (value >> 6));
}

/* The GPL edition's 10-bit input, which heif-enc cannot make from an 8-bit file: the
   samples widened, gray as a 10-bit Y plane (and alpha), colour as big-endian RRGGBB(AA),
   so libheif's colour conversion runs at 10 bits. */
static struct heif_error make_ten_bit_image(const SkidHeicInput* in, struct heif_image** out)
{
	struct heif_image* image = NULL;
	struct heif_error err;
	int w = (int) in->width, h = (int) in->height;
	size_t stride;
	uint8_t* plane;
	*out = NULL;
	if (in->layout == 0) {
		const heif_channel channels[2] = {heif_channel_Y, heif_channel_Alpha};
		err = heif_image_create(w, h, heif_colorspace_monochrome, heif_chroma_monochrome, &image);
		if (err.code) return err;
		for (int c = 0; c < (in->has_alpha ? 2 : 1); c++) {
			err = heif_image_add_plane(image, channels[c], w, h, 10);
			if (err.code) goto fail;
			plane = heif_image_get_plane2(image, channels[c], &stride);
			for (uint32_t y = 0; y < in->height; y++) {
				uint16_t* row = (uint16_t*) (plane + y * stride);
				const uint8_t* from = in->planes[c] + y * in->strides[c];
				for (uint32_t x = 0; x < in->width; x++) row[x] = widen(from[x]);
			}
		}
	}
	else {
		const int channels = in->has_alpha ? 4 : 3;
		err = heif_image_create(w, h, heif_colorspace_RGB, in->has_alpha ? heif_chroma_interleaved_RRGGBBAA_BE : heif_chroma_interleaved_RRGGBB_BE, &image);
		if (err.code) return err;
		err = heif_image_add_plane(image, heif_channel_interleaved, w, h, 10);
		if (err.code) goto fail;
		plane = heif_image_get_plane2(image, heif_channel_interleaved, &stride);
		for (uint32_t y = 0; y < in->height; y++) {
			uint8_t* row = plane + y * stride;
			const uint8_t* from = in->planes[0] + y * in->strides[0];
			for (uint32_t i = 0; i < in->width * (uint32_t) channels; i++) {
				const uint16_t value = widen(from[i]);
				row[2 * i] = (uint8_t) (value >> 8);
				row[2 * i + 1] = (uint8_t) (value & 0xff);
			}
		}
	}
	*out = image;
	return err;

fail:
	heif_image_release(image);
	return err;
}

/* The heif_image heifio's loadPNG builds from these samples. */
static struct heif_error make_image(const SkidHeicInput* in, struct heif_image** out)
{
	struct heif_image* image = NULL;
	struct heif_error err;
	int w = (int) in->width, h = (int) in->height;
	size_t stride;
	uint8_t* plane;
	*out = NULL;

	if (in->ten_bit) {
		err = make_ten_bit_image(in, &image);
		if (err.code) return err;
	}
	else if (in->layout == 0) {
		err = heif_image_create(w, h, heif_colorspace_monochrome, heif_chroma_monochrome, &image);
		if (err.code) return err;
		err = heif_image_add_plane(image, heif_channel_Y, w, h, 8);
		if (err.code) goto fail;
		plane = heif_image_get_plane2(image, heif_channel_Y, &stride);
		copy_plane(plane, stride, in->planes[0], in->strides[0], in->width, in->height);
		if (in->has_alpha) {
			err = heif_image_add_plane(image, heif_channel_Alpha, w, h, 8);
			if (err.code) goto fail;
			plane = heif_image_get_plane2(image, heif_channel_Alpha, &stride);
			copy_plane(plane, stride, in->planes[1], in->strides[1], in->width, in->height);
		}
	}
	else if (in->layout == 2) {
		/* decoder_webp.cc: Y, Cb and Cr at 4:2:0, then alpha. */
		int uv_w = (w + 1) / 2, uv_h = (h + 1) / 2;
		const enum heif_channel channels[4] = {heif_channel_Y, heif_channel_Cb, heif_channel_Cr, heif_channel_Alpha};
		int c;
		err = heif_image_create(w, h, heif_colorspace_YCbCr, heif_chroma_420, &image);
		if (err.code) return err;
		for (c = 0; c < (in->has_alpha ? 4 : 3); c++) {
			int full = c == 0 || c == 3;
			err = heif_image_add_plane(image, channels[c], full ? w : uv_w, full ? h : uv_h, 8);
			if (err.code) goto fail;
		}
		for (c = 0; c < (in->has_alpha ? 4 : 3); c++) {
			int full = c == 0 || c == 3;
			plane = heif_image_get_plane2(image, channels[c], &stride);
			copy_plane(plane, stride, in->planes[c], in->strides[c], full ? in->width : (size_t) uv_w, full ? in->height : (size_t) uv_h);
		}
		if (in->has_alpha) {
			heif_image_set_premultiplied_alpha(image, 0);
		}
	}
	else {
		err = heif_image_create(w, h, heif_colorspace_RGB, in->has_alpha ? heif_chroma_interleaved_RGBA : heif_chroma_interleaved_RGB, &image);
		if (err.code) return err;
		err = heif_image_add_plane(image, heif_channel_interleaved, w, h, in->has_alpha ? 32 : 24);
		if (err.code) goto fail;
		plane = heif_image_get_plane2(image, heif_channel_interleaved, &stride);
		copy_plane(plane, stride, in->planes[0], in->strides[0], (size_t) in->width * (in->has_alpha ? 4 : 3), in->height);
	}

	if (in->icc && in->icc_size > 0) {
		heif_image_set_raw_color_profile(image, "prof", in->icc, in->icc_size);
	}
	*out = image;
	return err;

fail:
	heif_image_release(image);
	return err;
}

/* heif-enc's load_image: the JPEG reader for a JPEG, otherwise the samples as given. */
static struct heif_error load(const SkidHeicInput* in, Loaded* loaded, char* error, size_t error_size)
{
	struct heif_error ok = {heif_error_Ok, heif_suberror_Unspecified, "Success"};
	struct heif_error bad = {heif_error_Invalid_input, heif_suberror_Unspecified, "the JPEG could not be read"};
	memset(loaded, 0, sizeof(*loaded));
	if (!in->jpeg) {
		loaded->orientation = in->orientation;
		return make_image(in, &loaded->image);
	}
	if (!skid_heic_load_jpeg(in->jpeg, in->jpeg_size, &loaded->holder, &loaded->image, &loaded->exif, &loaded->exif_size, &loaded->xmp, &loaded->xmp_size, &loaded->orientation, error, error_size)) {
		return bad;
	}
	return ok;
}

/* create_output_nclx_profile_and_configure_encoder, without -L (see settings/heic.rs). */
static struct heif_error make_nclx(const SkidHeicSettings* s, struct heif_encoder* encoder, const struct heif_image* image, struct heif_color_profile_nclx** out)
{
	struct heif_error ok = {heif_error_Ok, heif_suberror_Unspecified, "Success"};
	struct heif_color_profile_nclx* nclx = heif_nclx_color_profile_alloc();
	struct heif_color_profile_nclx* input_nclx = NULL;
	struct heif_error err;
	*out = nclx;
	if (!nclx) {
		struct heif_error oom = {heif_error_Encoding_error, heif_suberror_Unspecified, "Cannot allocate NCLX color profile."};
		return oom;
	}

	switch (s->color_profile) {
		case 0:
			err = heif_nclx_color_profile_set_matrix_coefficients(nclx, (uint16_t) s->matrix_coefficients);
			if (err.code) return err;
			err = heif_nclx_color_profile_set_transfer_characteristics(nclx, (uint16_t) s->transfer_characteristics);
			if (err.code) return err;
			err = heif_nclx_color_profile_set_color_primaries(nclx, (uint16_t) s->colour_primaries);
			if (err.code) return err;
			nclx->full_range_flag = (uint8_t) s->full_range;
			break;
		case 1:
			err = heif_image_get_nclx_color_profile(image, &input_nclx);
			if (err.code == heif_error_Color_profile_does_not_exist) {
				nclx->matrix_coefficients = heif_matrix_coefficients_ITU_R_BT_709_5;
				nclx->color_primaries = heif_color_primaries_ITU_R_BT_709_5;
				nclx->transfer_characteristics = heif_image_get_colorspace(image) == heif_colorspace_RGB ? heif_transfer_characteristic_IEC_61966_2_1 : heif_transfer_characteristic_ITU_R_BT_709_5;
			}
			else if (err.code) {
				return err;
			}
			else {
				nclx->matrix_coefficients = input_nclx->matrix_coefficients;
				nclx->transfer_characteristics = input_nclx->transfer_characteristics;
				nclx->color_primaries = input_nclx->color_primaries;
				nclx->full_range_flag = input_nclx->full_range_flag;
				heif_nclx_color_profile_free(input_nclx);
			}
			break;
		case 2:
			nclx->matrix_coefficients = heif_matrix_coefficients_ITU_R_BT_601_6;
			nclx->color_primaries = heif_color_primaries_ITU_R_BT_601_6;
			nclx->transfer_characteristics = heif_transfer_characteristic_ITU_R_BT_601_6;
			break;
		case 3:
			nclx->matrix_coefficients = heif_matrix_coefficients_ITU_R_BT_709_5;
			nclx->color_primaries = heif_color_primaries_ITU_R_BT_709_5;
			nclx->transfer_characteristics = heif_transfer_characteristic_ITU_R_BT_709_5;
			break;
		default:
			nclx->matrix_coefficients = heif_matrix_coefficients_ITU_R_BT_2020_2_non_constant_luminance;
			nclx->color_primaries = heif_color_primaries_ITU_R_BT_2020_2_and_2100_0;
			if (heif_image_has_channel(image, heif_channel_Y) && heif_image_get_bits_per_pixel(image, heif_channel_Y) <= 10) {
				nclx->transfer_characteristics = heif_transfer_characteristic_ITU_R_BT_2020_2_10bit;
			}
			else {
				nclx->transfer_characteristics = heif_transfer_characteristic_ITU_R_BT_2020_2_12bit;
			}
			break;
	}

	/* -L: lossless, with RGB kept as RGB at 4:4:4, or the input's own chroma. */
	if (s->lossless) {
		const char* chroma;
		err = heif_encoder_set_lossless(encoder, 1);
		if (err.code) return err;
		if (heif_image_get_colorspace(image) == heif_colorspace_RGB) {
			nclx->matrix_coefficients = heif_matrix_coefficients_RGB_GBR;
			nclx->full_range_flag = 1;
			chroma = "444";
		}
		else {
			switch (heif_image_get_chroma_format(image)) {
				case heif_chroma_422: chroma = "422"; break;
				case heif_chroma_444: chroma = "444"; break;
				default: chroma = "420"; break;
			}
		}
		err = heif_encoder_set_parameter(encoder, "chroma", chroma);
		if (err.code) return err;
	}
	return ok;
}

/* encode_tiled with heif-enc's `grid` tiling method, the tiles cut from a second copy of
   the input as input_tiles_generator_cut_image does. */
static struct heif_error encode_cut_tiles(struct heif_context* ctx, struct heif_encoder* encoder, const struct heif_encoding_options* options, const SkidHeicInput* in, int tile_size, struct heif_image_handle** out, char* error, size_t error_size)
{
	Loaded loaded;
	struct heif_image* source;
	struct heif_image_handle* grid = NULL;
	struct heif_error err = load(in, &loaded, error, error_size);
	uint32_t width, height, columns, rows;
	*out = NULL;
	if (err.code) return err;
	source = loaded.image;
	width = (uint32_t) heif_image_get_primary_width(source);
	height = (uint32_t) heif_image_get_primary_height(source);
	columns = (width + (uint32_t) tile_size - 1) / (uint32_t) tile_size;
	rows = (height + (uint32_t) tile_size - 1) / (uint32_t) tile_size;

	err = heif_context_add_grid_image(ctx, width, height, columns, rows, options, &grid);
	if (err.code) goto done;
	for (uint32_t ty = 0; ty < rows; ty++) {
		for (uint32_t tx = 0; tx < columns; tx++) {
			struct heif_image* tile = NULL;
			err = heif_image_extract_area(source, tx * (uint32_t) tile_size, ty * (uint32_t) tile_size, (uint32_t) tile_size, (uint32_t) tile_size, heif_get_global_security_limits(), &tile);
			if (err.code) goto done;
			/* heif-enc prints this error and carries on. */
			(void) heif_image_extend_to_size_fill_with_zero(tile, tile_size, tile_size);
			err = heif_context_add_image_tile(ctx, grid, tx, ty, tile, encoder);
			heif_image_release(tile);
			if (err.code) goto done;
		}
	}
	*out = grid;
	grid = NULL;

done:
	if (grid) heif_image_handle_release(grid);
	release(&loaded);
	return err;
}

/* 1 on success, 2 when heif-enc's JPEG reader refuses the JPEG, 0 on any other error. */
int skid_heic_encode(const SkidHeicInput* in, const SkidHeicSettings* s, uint8_t** out, size_t* out_size, char* error, size_t error_size)
{
	struct heif_context* ctx = NULL;
	struct heif_encoder* encoder = NULL;
	struct heif_encoding_options* options = NULL;
	Loaded loaded = {NULL, NULL, NULL, 0, NULL, 0, 1};
	struct heif_image* image = NULL;
	const uint8_t* exif;
	const uint8_t* xmp;
	size_t exif_size, xmp_size;
	struct heif_image_handle* handle = NULL;
	struct heif_color_profile_nclx* nclx = NULL;
	struct heif_error err;
	struct heif_writer writer;
	Buffer buffer = {NULL, 0, 0};
	int ok = 0;
	*out = NULL;
	*out_size = 0;

	ctx = heif_context_alloc();
	if (!ctx) {
		snprintf(error, error_size, "libheif could not allocate a context");
		return 0;
	}
	if (s->unif) heif_context_set_unif(ctx, 1);
	if (s->mini) heif_context_set_write_mini_format(ctx, 1);

	{
		const struct heif_encoder_descriptor* descriptors[10];
		int count = heif_get_encoder_descriptors(heif_compression_HEVC, NULL, descriptors, 10);
		int index = -1;
		for (int i = 0; i < count; i++) {
			if (!s->encoder || strcmp(s->encoder, heif_encoder_descriptor_get_id_name(descriptors[i])) == 0) {
				index = i;
				break;
			}
		}
		if (index < 0) {
			snprintf(error, error_size, "this libheif has no %s HEVC encoder", s->encoder ? s->encoder : "");
			goto done;
		}
		err = heif_context_get_encoder(ctx, descriptors[index], &encoder);
		if (err.code) { fail(error, error_size, "heif_context_get_encoder", err); goto done; }
	}

	if (!s->lossless) {
		err = heif_encoder_set_lossy_quality(encoder, s->quality);
		if (err.code) { fail(error, error_size, "heif_encoder_set_lossy_quality", err); goto done; }
	}
	for (size_t i = 0; i < s->parameter_count; i++) {
		err = heif_encoder_set_parameter(encoder, s->parameters[i].name, s->parameters[i].value);
		if (err.code) {
			snprintf(error, error_size, "heif_encoder_set_parameter %s=%s: %s", s->parameters[i].name, s->parameters[i].value, err.message ? err.message : "unknown error");
			goto done;
		}
	}

	options = heif_encoding_options_alloc();
	options->save_two_colr_boxes_when_ICC_and_nclx_available = (uint8_t) s->two_colr_boxes;
	if (s->chroma_downsampling) {
		options->color_conversion_options.preferred_chroma_downsampling_algorithm = (enum heif_chroma_downsampling_algorithm) s->chroma_downsampling;
		options->color_conversion_options.only_use_preferred_chroma_algorithm = 1;
	}

	err = load(in, &loaded, error, error_size);
	if (err.code) {
		if (in->jpeg) {
			ok = 2; /* heif-enc's JPEG reader refused it. */
		}
		else {
			fail(error, error_size, "reading the image", err);
		}
		goto done;
	}
	image = loaded.image;
	exif = in->jpeg ? loaded.exif : in->exif;
	exif_size = in->jpeg ? loaded.exif_size : in->exif_size;
	xmp = in->jpeg ? loaded.xmp : in->xmp;
	xmp_size = in->jpeg ? loaded.xmp_size : in->xmp_size;

	err = make_nclx(s, encoder, image, &nclx);
	if (err.code) { fail(error, error_size, "the colour profile", err); goto done; }
	options->save_alpha_channel = (uint8_t) s->alpha;
	options->output_nclx_profile = nclx;
	options->image_orientation = heif_orientation_concat((enum heif_orientation) loaded.orientation, (enum heif_orientation) s->orientation);
	if (s->premultiplied) heif_image_set_premultiplied_alpha(image, 1);

	if (s->cut_tiles > 0) {
		err = encode_cut_tiles(ctx, encoder, options, in, s->cut_tiles, &handle, error, error_size);
		if (err.code) {
			if (err.code != heif_error_Invalid_input || !in->jpeg) fail(error, error_size, "encoding the tiles", err);
			goto done;
		}
	}
	else {
		err = heif_context_encode_image(ctx, image, encoder, options, &handle);
		if (err.code) { fail(error, error_size, "heif_context_encode_image", err); goto done; }
	}

	if (s->clli_set) {
		struct heif_content_light_level clli;
		clli.max_content_light_level = s->clli[0];
		clli.max_pic_average_light_level = s->clli[1];
		heif_image_handle_set_content_light_level(handle, &clli);
	}
	if (s->pasp_set) heif_image_handle_set_pixel_aspect_ratio(handle, s->pasp[0], s->pasp[1]);
	if (s->omaf_projection >= 0) heif_image_handle_set_omaf_image_projection(handle, (enum heif_omaf_image_projection) s->omaf_projection);
	heif_context_set_primary_image(ctx, handle);

	if (exif && exif_size > 0) {
		err = heif_context_add_exif_metadata(ctx, handle, exif, (int) exif_size);
		if (err.code) { fail(error, error_size, "writing the Exif", err); goto done; }
	}
	if (xmp && xmp_size > 0) {
		err = heif_context_add_XMP_metadata2(ctx, handle, xmp, (int) xmp_size, heif_metadata_compression_off);
		if (err.code) { fail(error, error_size, "writing the XMP", err); goto done; }
	}

	if (s->thumbnail > 0) {
		struct heif_image_handle* thumbnail = NULL;
		options->save_alpha_channel = (uint8_t) (s->alpha && s->thumbnail_alpha);
		err = heif_context_encode_thumbnail(ctx, image, handle, encoder, options, s->thumbnail, &thumbnail);
		if (err.code) { fail(error, error_size, "heif_context_encode_thumbnail", err); goto done; }
		if (thumbnail) heif_image_handle_release(thumbnail);
	}

	heif_image_handle_release(handle);
	handle = NULL;

	if (s->description && s->description[0]) {
		struct heif_image_handle* primary = NULL;
		struct heif_property_user_description udes;
		err = heif_context_get_primary_image_handle(ctx, &primary);
		if (err.code) { fail(error, error_size, "heif_context_get_primary_image_handle", err); goto done; }
		udes.version = 1;
		udes.lang = NULL;
		udes.name = NULL;
		udes.tags = NULL;
		udes.description = s->description;
		err = heif_item_add_property_user_description(ctx, heif_image_handle_get_item_id(primary), &udes, NULL);
		heif_image_handle_release(primary);
		if (err.code) { fail(error, error_size, "heif_item_add_property_user_description", err); goto done; }
	}

	for (size_t i = 0; i < s->brand_count; i++) {
		heif_context_add_compatible_brand(ctx, heif_fourcc_to_brand(s->brands + 4 * i));
	}

	writer.writer_api_version = 1;
	writer.write = write_buffer;
	err = heif_context_write(ctx, &writer, &buffer);
	if (err.code) { fail(error, error_size, "heif_context_write", err); goto done; }
	*out = buffer.data;
	*out_size = buffer.size;
	buffer.data = NULL;
	ok = 1;

done:
	free(buffer.data);
	if (handle) heif_image_handle_release(handle);
	release(&loaded);
	if (nclx) heif_nclx_color_profile_free(nclx);
	if (options) heif_encoding_options_free(options);
	if (encoder) heif_encoder_release(encoder);
	heif_context_free(ctx);
	return ok;
}

void skid_heic_free(uint8_t* data)
{
	free(data);
}

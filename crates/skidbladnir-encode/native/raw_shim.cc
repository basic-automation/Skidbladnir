/*
 * A camera raw file developed by LibRaw (CDDL-1.0, the licence of the two LibRaw offers
 * that this app takes): what `dcraw_emu -w -o 1 -6 -g 2.4 12.92` writes. The camera's
 * white balance, sRGB primaries, the sRGB curve and 16 bits per sample, turned the way
 * the camera says it was held. Everything else is LibRaw's default: AHD demosaicing, and
 * the brightening that puts the 99th percentile at white, as dcraw does.
 *
 * A camera LibRaw does not list is refused: it still opens such a file, but guesses the
 * black level and borrows the colour matrix of the nearest name (an EOS R6 Mark III gets
 * the Mark II's, and no black level at all), so the picture comes out washed out and the
 * wrong colour rather than failing.
 *
 * A LibRaw object per call, so calls on different threads do not share anything.
 */

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "libraw/libraw.h"

typedef struct {
	uint32_t width;
	uint32_t height;
	/* RGB, 16 bits per sample in native byte order, rows tight. */
	uint16_t* pixels;
} SkidRawImage;

/* Whether LibRaw lists the camera, by name or by name with a note after it. */
static int known_camera(const libraw_data_t* raw)
{
	char name[160];
	snprintf(name, sizeof(name), "%s %s", raw->idata.make, raw->idata.model);
	size_t length = strlen(name);
	for (const char** camera = libraw_cameraList(); *camera; camera++) {
		if (strncmp(*camera, name, length) == 0 && ((*camera)[length] == 0 || (*camera)[length] == ' ' && (*camera)[length + 1] == '(')) return 1;
	}
	return 0;
}

static int fail(libraw_data_t* raw, int code, char* error, size_t error_size)
{
	snprintf(error, error_size, "%s", libraw_strerror(code));
	libraw_close(raw);
	return 0;
}

/* 1 on success, 0 on an error (in `error`). */
extern "C" int skid_raw_decode(const uint8_t* data, size_t size, SkidRawImage* out, char* error, size_t error_size)
{
	memset(out, 0, sizeof(*out));
	libraw_data_t* raw = libraw_init(0);
	if (!raw) {
		snprintf(error, error_size, "LibRaw could not start");
		return 0;
	}
	raw->params.use_camera_wb = 1;
	raw->params.output_color = 1;
	raw->params.output_bps = 16;
	raw->params.gamm[0] = 1 / 2.4;
	raw->params.gamm[1] = 12.92;

	int code = libraw_open_buffer(raw, data, size);
	if (code != LIBRAW_SUCCESS) return fail(raw, code, error, error_size);
	if (!known_camera(raw)) {
		snprintf(error, error_size, "LibRaw %s does not support the %s %s yet", libraw_version(), raw->idata.make, raw->idata.model);
		libraw_close(raw);
		return 0;
	}
	if ((code = libraw_unpack(raw)) != LIBRAW_SUCCESS) return fail(raw, code, error, error_size);
	if ((code = libraw_dcraw_process(raw)) != LIBRAW_SUCCESS) return fail(raw, code, error, error_size);
	libraw_processed_image_t* image = libraw_dcraw_make_mem_image(raw, &code);
	if (!image) return fail(raw, code, error, error_size);
	if (image->type != LIBRAW_IMAGE_BITMAP || image->colors != 3 || image->bits != 16) {
		snprintf(error, error_size, "LibRaw made a %d-colour, %d-bit image", image->colors, image->bits);
		libraw_dcraw_clear_mem(image);
		libraw_close(raw);
		return 0;
	}
	size_t length = (size_t) image->width * image->height * 3;
	out->pixels = (uint16_t*) malloc(length * sizeof(uint16_t));
	if (!out->pixels) {
		snprintf(error, error_size, "out of memory");
		libraw_dcraw_clear_mem(image);
		libraw_close(raw);
		return 0;
	}
	memcpy(out->pixels, image->data, length * sizeof(uint16_t));
	out->width = image->width;
	out->height = image->height;
	libraw_dcraw_clear_mem(image);
	libraw_close(raw);
	return 1;
}

extern "C" void skid_raw_free(uint16_t* pixels)
{
	free(pixels);
}

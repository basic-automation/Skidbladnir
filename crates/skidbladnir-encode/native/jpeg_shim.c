/*
 * A JPEG decoded by libjpeg-turbo with libjpeg's defaults, the way cwebp
 * (imageio/jpegdec.c) and cjxl (lib/extras/dec/jpg.cc) decode one: islow IDCT, fancy
 * upsampling, and YCbCr converted to RGB (a gray JPEG stays gray). Every reference tool
 * links libjpeg-turbo, and the samples it produces are what their encoders see, so the
 * app decodes with the same library instead of another decoder that rounds differently.
 *
 * CMYK and YCCK are left to the caller: cwebp and cjxl refuse them.
 */

#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "jpeglib.h"

typedef struct {
	uint32_t width;
	uint32_t height;
	/* 1 gray or 3 RGB, interleaved, rows tight. */
	uint32_t components;
	uint8_t* pixels;
} SkidJpegImage;

typedef struct {
	struct jpeg_error_mgr pub;
	jmp_buf jump;
	char* error;
	size_t error_size;
} ErrorManager;

static void error_exit(j_common_ptr cinfo)
{
	ErrorManager* manager = (ErrorManager*) cinfo->err;
	char message[JMSG_LENGTH_MAX];
	(*cinfo->err->format_message)(cinfo, message);
	snprintf(manager->error, manager->error_size, "%s", message);
	longjmp(manager->jump, 1);
}

/* Warnings (a truncated file, say) do not stop the tools, and are not printed here. */
static void output_message(j_common_ptr cinfo)
{
	(void) cinfo;
}

/* 1 on success, 0 on an error (in `error`), 2 for a CMYK or YCCK JPEG. */
int skid_jpeg_decode(const uint8_t* data, size_t size, SkidJpegImage* out, char* error, size_t error_size)
{
	struct jpeg_decompress_struct cinfo;
	ErrorManager manager;
	uint8_t* volatile pixels = NULL;
	volatile int result = 0;
	memset(out, 0, sizeof(*out));
	memset(&cinfo, 0, sizeof(cinfo));

	cinfo.err = jpeg_std_error(&manager.pub);
	manager.pub.error_exit = error_exit;
	manager.pub.output_message = output_message;
	manager.error = error;
	manager.error_size = error_size;
	if (setjmp(manager.jump)) {
		free(pixels);
		jpeg_destroy_decompress(&cinfo);
		return 0;
	}

	jpeg_create_decompress(&cinfo);
	jpeg_mem_src(&cinfo, data, (unsigned long) size);
	jpeg_read_header(&cinfo, TRUE);
	if (cinfo.jpeg_color_space == JCS_CMYK || cinfo.jpeg_color_space == JCS_YCCK) {
		jpeg_destroy_decompress(&cinfo);
		return 2;
	}
	jpeg_start_decompress(&cinfo);
	if (cinfo.output_components != 1 && cinfo.output_components != 3) {
		snprintf(error, error_size, "a JPEG with %d components", cinfo.output_components);
		jpeg_destroy_decompress(&cinfo);
		return 0;
	}
	{
		const size_t row = (size_t) cinfo.output_width * (size_t) cinfo.output_components;
		pixels = (uint8_t*) malloc(row * cinfo.output_height);
		if (!pixels) {
			snprintf(error, error_size, "out of memory");
			jpeg_destroy_decompress(&cinfo);
			return 0;
		}
		while (cinfo.output_scanline < cinfo.output_height) {
			JSAMPROW rows[1];
			rows[0] = pixels + row * cinfo.output_scanline;
			jpeg_read_scanlines(&cinfo, rows, 1);
		}
	}
	jpeg_finish_decompress(&cinfo);
	out->width = cinfo.output_width;
	out->height = cinfo.output_height;
	out->components = (uint32_t) cinfo.output_components;
	out->pixels = pixels;
	result = 1;
	jpeg_destroy_decompress(&cinfo);
	return result;
}

void skid_jpeg_free(uint8_t* pixels)
{
	free(pixels);
}

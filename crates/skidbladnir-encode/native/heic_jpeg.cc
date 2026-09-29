/*
 * heif-enc's JPEG reader, heifio/decoder_jpeg.cc from libheif 1.23.5 (MIT), compiled as it
 * is but reading from memory: heif-enc keeps a JPEG's own YCbCr planes where it can, and
 * what it does with the rest (the sampling it decimates, CMYK, the Exif orientation) is
 * the file below, not a copy of it.
 *
 * `loadJPEG` opens a file by name. Here the "name" is a pointer to the bytes, `fopen`
 * hands it back as the "file", and libjpeg's stdio source becomes its memory source.
 * heifio/exif.cc comes along for `read_exif_orientation_tag`; the two byte helpers it
 * takes from libheif's internal common_utils.h are defined here instead of pulling in
 * libheif's internals.
 *
 * libjpeg's default error handler ends the process, which a command-line tool can afford
 * and an app cannot: its `jpeg_std_error` is swapped for one whose fatal errors jump back
 * to skid_heic_load_jpeg instead (leaking what the reader had allocated, on that path
 * only), and whose warnings are not printed.
 */

#include <csetjmp>
#include <algorithm>
#include <array>
#include <cassert>
#include <cinttypes>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <iostream>
#include <memory>
#include <new>
#include <string>
#include <vector>

#include "libheif/heif.h"

extern "C" {
#include <jpeglib.h>
}

// common_utils.h, as far as exif.cc uses it.
#define LIBHEIF_COMMON_UTILS_H
constexpr uint32_t four_bytes_to_uint32(uint8_t msb, uint8_t b, uint8_t c, uint8_t lsb)
{
	return (static_cast<uint32_t>(msb << 24) | static_cast<uint32_t>(b << 16) | static_cast<uint32_t>(c << 8) | static_cast<uint32_t>(lsb));
}
constexpr uint16_t two_bytes_to_uint16(uint8_t msb, uint8_t lsb)
{
	return static_cast<uint16_t>((msb << 8) | lsb);
}

namespace {
struct Memory
{
	const unsigned char* data;
	size_t size;
};

thread_local std::jmp_buf* fatal = nullptr;
thread_local char fatal_message[JMSG_LENGTH_MAX];

void error_exit(j_common_ptr cinfo)
{
	(*cinfo->err->format_message)(cinfo, fatal_message);
	std::longjmp(*fatal, 1);
}

void output_message(j_common_ptr) {}

jpeg_error_mgr* app_std_error(jpeg_error_mgr* err)
{
	jpeg_std_error(err);
	err->error_exit = error_exit;
	err->output_message = output_message;
	return err;
}
}

#define jpeg_std_error app_std_error
#define fopen(name, mode) (reinterpret_cast<FILE*>(const_cast<char*>(name)))
#define fclose(file) (0)
#define jpeg_stdio_src(cinfo, file) jpeg_mem_src((cinfo), reinterpret_cast<const Memory*>(file)->data, static_cast<unsigned long>(reinterpret_cast<const Memory*>(file)->size))

#include "exif.cc"
#include "decoder_jpeg.cc"

#undef jpeg_std_error
#undef fopen
#undef fclose
#undef jpeg_stdio_src

/* loadJPEG, with a fatal libjpeg error returned instead of ending the process. */
static int load(const Memory* memory, InputImage* input, char* error, size_t error_size)
{
	std::jmp_buf jump;
	fatal = &jump;
	if (setjmp(jump)) {
		fatal = nullptr;
		snprintf(error, error_size, "%s", fatal_message);
		return 0;
	}
	heif_error err = loadJPEG(reinterpret_cast<const char*>(memory), input);
	fatal = nullptr;
	if (err.code != heif_error_Ok) {
		snprintf(error, error_size, "%s", err.message ? err.message : "the JPEG could not be read");
		return 0;
	}
	return 1;
}

/* The heif_image heif-enc makes of this JPEG, with its Exif, XMP and Exif orientation.
   `*holder` keeps the image alive until skid_heic_jpeg_release; the two buffers are freed
   with skid_heic_jpeg_free. */
extern "C" int skid_heic_load_jpeg(const unsigned char* data, size_t size, void** holder, heif_image** out, unsigned char** exif, size_t* exif_size, unsigned char** xmp, size_t* xmp_size, int* orientation, char* error, size_t error_size)
{
	const Memory memory{data, size};
	InputImage input;
	*holder = nullptr;
	*out = nullptr;
	*exif = nullptr;
	*xmp = nullptr;
	*exif_size = 0;
	*xmp_size = 0;
	if (!load(&memory, &input, error, error_size)) {
		return 0;
	}
	auto copy = [](const std::vector<uint8_t>& from, unsigned char** to, size_t* to_size) {
		if (from.empty()) return true;
		*to = static_cast<unsigned char*>(malloc(from.size()));
		if (!*to) return false;
		memcpy(*to, from.data(), from.size());
		*to_size = from.size();
		return true;
	};
	if (!copy(input.exif, exif, exif_size) || !copy(input.xmp, xmp, xmp_size)) {
		free(*exif);
		*exif = nullptr;
		snprintf(error, error_size, "out of memory");
		return 0;
	}
	*orientation = static_cast<int>(input.orientation);
	auto* kept = new (std::nothrow) std::shared_ptr<heif_image>(input.image);
	if (!kept) {
		free(*exif);
		free(*xmp);
		*exif = nullptr;
		*xmp = nullptr;
		snprintf(error, error_size, "out of memory");
		return 0;
	}
	*holder = kept;
	*out = kept->get();
	return 1;
}

extern "C" void skid_heic_jpeg_release(void* holder)
{
	delete static_cast<std::shared_ptr<heif_image>*>(holder);
}

extern "C" void skid_heic_jpeg_free(unsigned char* data)
{
	free(data);
}

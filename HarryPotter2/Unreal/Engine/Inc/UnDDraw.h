/*=============================================================================
	UnDDraw.h: Unreal DirectDraw definitions
	Required for S3TC compression/decompression

	Created by Seth Sowerby
=============================================================================*/

#ifndef __DDRAW_INCLUDED__
#define __DDRAW_INCLUDED__

#include <cstddef>
#include <cstdint>

// DirectDraw Surface definitions
// Needed for S3TC compression/decompression code
#ifdef __cplusplus
extern "C" {
#endif

	//
// For compilers that don't support nameless unions, do a
//
// #define NONAMELESSUNION
//
// before #include <ddraw.h>
//
#ifndef DUMMYUNIONNAMEN
#if defined(__cplusplus) || !defined(NONAMELESSUNION)
#define DUMMYUNIONNAMEN(n)
#else
#define DUMMYUNIONNAMEN(n)      u##n
#endif
#endif

typedef void *LPVOID;

//
// DDCOLORKEY
//
typedef struct _DDCOLORKEY
{
    std::uint32_t dwColorSpaceLowValue;	// low boundary of color space that is to
					// be treated as Color Key, inclusive
    std::uint32_t dwColorSpaceHighValue;	// high boundary of color space that is
					// to be treated as Color Key, inclusive
} DDCOLORKEY;

//
// DDSCAPS
//
typedef struct _DDSCAPS
{
    std::uint32_t dwCaps;		// capabilities of surface wanted
} DDSCAPS;

//
// DDPIXELFORMAT
//
typedef struct _DDPIXELFORMAT
{
    std::uint32_t dwSize;			// size of structure
    std::uint32_t dwFlags;			// pixel format flags
    std::uint32_t dwFourCC;			// (FOURCC code)
    union
    {
	std::uint32_t dwRGBBitCount;		// how many bits per pixel
	std::uint32_t dwYUVBitCount;		// how many bits per pixel
	std::uint32_t dwZBufferBitDepth;	// how many total bits/pixel in z buffer (including any stencil bits)
	std::uint32_t dwAlphaBitDepth;		// how many bits for alpha channels
	std::uint32_t dwLuminanceBitCount;	// how many bits per pixel
	std::uint32_t dwBumpBitCount;		// how many bits per "buxel", total
    } DUMMYUNIONNAMEN(1);
    union
    {
	std::uint32_t dwRBitMask;		// mask for red bit
	std::uint32_t dwYBitMask;		// mask for Y bits
	std::uint32_t dwStencilBitDepth;	// how many stencil bits (note: dwZBufferBitDepth-dwStencilBitDepth is total Z-only bits)
	std::uint32_t dwLuminanceBitMask;	// mask for luminance bits
	std::uint32_t dwBumpDuBitMask;		// mask for bump map U delta bits
    } DUMMYUNIONNAMEN(2);
    union
    {
	std::uint32_t dwGBitMask;		// mask for green bits
	std::uint32_t dwUBitMask;		// mask for U bits
	std::uint32_t dwZBitMask;		// mask for Z bits
	std::uint32_t dwBumpDvBitMask;		// mask for bump map V delta bits
    } DUMMYUNIONNAMEN(3);
    union
    {
	std::uint32_t dwBBitMask;		// mask for blue bits
	std::uint32_t dwVBitMask;		// mask for V bits
	std::uint32_t dwStencilBitMask;	// mask for stencil bits
	std::uint32_t dwBumpLuminanceBitMask; // mask for luminance in bump map
    } DUMMYUNIONNAMEN(4);
    union
    {
	std::uint32_t dwRGBAlphaBitMask;	// mask for alpha channel
	std::uint32_t dwYUVAlphaBitMask;	// mask for alpha channel
	std::uint32_t dwLuminanceAlphaBitMask;// mask for alpha channel
	std::uint32_t dwRGBZBitMask;		// mask for Z channel
	std::uint32_t dwYUVZBitMask;		// mask for Z channel
    } DUMMYUNIONNAMEN(5);
} DDPIXELFORMAT;

//
// DDSURFACEDESC
//
typedef struct _DDSURFACEDESC
{
    std::uint32_t dwSize;			// size of the DDSURFACEDESC structure
    std::uint32_t dwFlags;			// determines what fields are valid
    std::uint32_t dwHeight;			// height of surface to be created
    std::uint32_t dwWidth;			// width of input surface
    union
    {
        std::int32_t  lPitch;			// distance to start of next line (return value only)
        std::uint32_t dwLinearSize;		// Formless late-allocated optimized surface size
    } DUMMYUNIONNAMEN(1);
    std::uint32_t dwBackBufferCount;		// number of back buffers requested
    union
    {
        std::uint32_t dwMipMapCount;		// number of mip-map levels requested
	std::uint32_t dwZBufferBitDepth;	// depth of Z buffer requested
	std::uint32_t dwRefreshRate;		// refresh rate (used when display mode is described)
    } DUMMYUNIONNAMEN(2);
    std::uint32_t dwAlphaBitDepth;		// depth of alpha buffer requested
    std::uint32_t dwReserved;			// reserved
    LPVOID lpSurface;				// pointer to the associated surface memory
    DDCOLORKEY ddckCKDestOverlay;		// color key for destination overlay use
    DDCOLORKEY ddckCKDestBlt;			// color key for destination blt use
    DDCOLORKEY ddckCKSrcOverlay;		// color key for source overlay use
    DDCOLORKEY ddckCKSrcBlt;			// color key for source blt use
    DDPIXELFORMAT ddpfPixelFormat;		// pixel format description of the surface
    DDSCAPS ddsCaps;				// direct draw surface capabilities
} DDSURFACEDESC;

/*
 * ddsCaps field is valid.
 */
#define DDSD_CAPS		0x00000001u	// default

/*
 * dwHeight field is valid.
 */
#define DDSD_HEIGHT		0x00000002u

/*
 * dwWidth field is valid.
 */
#define DDSD_WIDTH		0x00000004u

/*
 * lPitch is valid.
 */
#define DDSD_PITCH		0x00000008u

/*
 * lpSurface is valid.
 */
#define DDSD_LPSURFACE		0x00000800u

/*
 * ddpfPixelFormat is valid.
 */
#define DDSD_PIXELFORMAT	0x00001000u

/*
 * The FourCC code is valid.
 */
#define DDPF_FOURCC				0x00000004u

/*
 * The RGB data in the pixel format structure is valid.
 */
#define DDPF_RGB				0x00000040u



#ifdef __cplusplus
}

static_assert(sizeof(DDCOLORKEY) == 8, "DDCOLORKEY numeric layout must remain fixed");
static_assert(sizeof(DDSCAPS) == 4, "DDSCAPS numeric layout must remain fixed");
static_assert(sizeof(DDPIXELFORMAT) == 32, "DDPIXELFORMAT numeric layout must remain fixed");
static_assert(sizeof(((DDSURFACEDESC*)0)->lPitch) == 4, "DDSURFACEDESC pitch must remain 32-bit");
#if defined(HP2_HOST_WCHAR_TCHAR)
static_assert(offsetof(DDSURFACEDESC, lpSurface) == 40, "DDSURFACEDESC host pointer offset changed");
static_assert(sizeof(DDSURFACEDESC) == 120, "DDSURFACEDESC host arm64 API layout changed");
#endif
#endif

#endif

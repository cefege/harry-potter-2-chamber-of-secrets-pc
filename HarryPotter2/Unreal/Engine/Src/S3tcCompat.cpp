/*=============================================================================
	S3tcCompat.cpp: S3TC/DXT1 compatibility over pinned libSquish.
=============================================================================*/

#include "S3tc.h"

#include <atomic>
#include <cstdint>
#include <cstring>
#include <limits>

// The pinned libSquish target does not publish its source directory as an
// interface include. Keep the small, stable ABI surface used here local rather
// than leaking a build-tree include path into Engine.
namespace squish
{
enum
{
	kDxt1 = 1 << 0,
	kColourClusterFit = 1 << 3,
};
void CompressMasked(const unsigned char* Rgba, int Mask, void* Block,
	int Flags, float* Metric);
void Decompress(unsigned char* Rgba, const void* Block, int Flags);
}

namespace
{
constexpr std::uint32_t DDSD_LINEARSIZE_VALUE = 0x00080000u;
constexpr std::uint32_t DDPF_ALPHAPIXELS_VALUE = 0x00000001u;
constexpr std::uint32_t DXT1_FOURCC =
	static_cast<std::uint32_t>('D') |
	(static_cast<std::uint32_t>('X') << 8) |
	(static_cast<std::uint32_t>('T') << 16) |
	(static_cast<std::uint32_t>('1') << 24);
constexpr unsigned DXT1_BLOCK_BYTES = 8;

std::atomic<int> GAlphaReference(128);

bool ValidDimensions(const DDSURFACEDESC* Desc)
{
	return Desc
		&& Desc->dwSize == sizeof(DDSURFACEDESC)
		&& Desc->dwWidth != 0
		&& Desc->dwHeight != 0
		&& Desc->dwWidth <= static_cast<std::uint32_t>(std::numeric_limits<int>::max())
		&& Desc->dwHeight <= static_cast<std::uint32_t>(std::numeric_limits<int>::max());
}

bool Dxt1Size(const DDSURFACEDESC* Desc, unsigned& Size)
{
	if (!ValidDimensions(Desc))
		return false;

	const std::uint64_t BlocksWide = (static_cast<std::uint64_t>(Desc->dwWidth) + 3u) / 4u;
	const std::uint64_t BlocksHigh = (static_cast<std::uint64_t>(Desc->dwHeight) + 3u) / 4u;
	const std::uint64_t Bytes = BlocksWide * BlocksHigh * DXT1_BLOCK_BYTES;
	if (Bytes > std::numeric_limits<unsigned>::max())
		return false;

	Size = static_cast<unsigned>(Bytes);
	return true;
}

bool DecodedSize(const DDSURFACEDESC* Desc, unsigned& Size)
{
	if (!ValidDimensions(Desc))
		return false;

	const std::uint64_t Bytes = static_cast<std::uint64_t>(Desc->dwWidth) * Desc->dwHeight * 4u;
	if (Bytes > std::numeric_limits<unsigned>::max())
		return false;

	Size = static_cast<unsigned>(Bytes);
	return true;
}

bool SupportedEncodeType(unsigned EncodeType)
{
	const unsigned RgbMode = EncodeType & _S3TC_ENCODE_RGB_MASK;
	const unsigned AlphaMode = EncodeType & _S3TC_ENCODE_ALPHA_MASK;
	const unsigned KnownBits = _S3TC_ENCODE_RGB_MASK | _S3TC_ENCODE_ALPHA_MASK |
		S3TC_ENCODE_ALPHA_NEED0 | S3TC_ENCODE_ALPHA_NEED1;
	return (EncodeType & ~KnownBits) == 0
		&& AlphaMode == S3TC_ENCODE_ALPHA_NONE
		&& (RgbMode == S3TC_ENCODE_RGB_FULL
			|| RgbMode == S3TC_ENCODE_RGB_COLOR_KEY
			|| RgbMode == S3TC_ENCODE_RGB_ALPHA_COMPARE);
}

bool ContiguousMask(std::uint32_t Mask)
{
	if (Mask == 0)
		return false;
	while ((Mask & 1u) == 0)
		Mask >>= 1;
	return (Mask & (Mask + 1u)) == 0;
}

std::uint8_t ExtractChannel(std::uint32_t Pixel, std::uint32_t Mask)
{
	if (Mask == 0)
		return 255;

	unsigned Shift = 0;
	while ((Mask & 1u) == 0)
	{
		Mask >>= 1;
		++Shift;
	}
	const std::uint64_t Value = (Pixel >> Shift) & Mask;
	return static_cast<std::uint8_t>((Value * 255u + Mask / 2u) / Mask);
}

bool SourceLayout(const DDSURFACEDESC* Desc, const PALETTEENTRY* Palette,
	unsigned EncodeType, unsigned& BytesPerPixel, std::uint32_t& Pitch)
{
	if (!ValidDimensions(Desc) || !Desc->lpSurface)
		return false;
	if ((Desc->dwFlags & (DDSD_WIDTH | DDSD_HEIGHT | DDSD_LPSURFACE | DDSD_PIXELFORMAT))
		!= (DDSD_WIDTH | DDSD_HEIGHT | DDSD_LPSURFACE | DDSD_PIXELFORMAT))
		return false;

	const DDPIXELFORMAT& Format = Desc->ddpfPixelFormat;
	if (Palette && Format.dwRGBBitCount == 8)
	{
		BytesPerPixel = 1;
	}
	else
	{
		if ((Format.dwFlags & DDPF_RGB) == 0
			|| (Format.dwRGBBitCount != 16 && Format.dwRGBBitCount != 24 && Format.dwRGBBitCount != 32))
			return false;
		BytesPerPixel = Format.dwRGBBitCount / 8;

		const std::uint32_t R = Format.dwRBitMask;
		const std::uint32_t G = Format.dwGBitMask;
		const std::uint32_t B = Format.dwBBitMask;
		if (!ContiguousMask(R) || !ContiguousMask(G) || !ContiguousMask(B)
			|| (R & G) != 0 || (R & B) != 0 || (G & B) != 0)
			return false;
		const std::uint32_t PixelMask = BytesPerPixel == 4
			? 0xffffffffu
			: ((1u << (BytesPerPixel * 8u)) - 1u);
		if (((R | G | B) & ~PixelMask) != 0)
			return false;

		if ((EncodeType & _S3TC_ENCODE_RGB_MASK) == S3TC_ENCODE_RGB_ALPHA_COMPARE)
		{
			const std::uint32_t A = Format.dwRGBAlphaBitMask;
			if (A != 0 && (!ContiguousMask(A) || (A & (R | G | B)) != 0 || (A & ~PixelMask) != 0))
				return false;
		}
	}

	const std::uint64_t TightPitch = static_cast<std::uint64_t>(Desc->dwWidth) * BytesPerPixel;
	if (TightPitch > std::numeric_limits<std::uint32_t>::max())
		return false;
	if (Desc->dwFlags & DDSD_PITCH)
	{
		if (Desc->lPitch <= 0 || static_cast<std::uint32_t>(Desc->lPitch) < TightPitch)
			return false;
		Pitch = static_cast<std::uint32_t>(Desc->lPitch);
	}
	else
	{
		Pitch = static_cast<std::uint32_t>(TightPitch);
	}
	return true;
}

std::uint32_t ReadLittleEndianPixel(const std::uint8_t* Source, unsigned BytesPerPixel)
{
	std::uint32_t Pixel = 0;
	for (unsigned Index = 0; Index < BytesPerPixel; ++Index)
		Pixel |= static_cast<std::uint32_t>(Source[Index]) << (Index * 8u);
	return Pixel;
}

void ReadRgba(const DDSURFACEDESC* Desc, const PALETTEENTRY* Palette,
	unsigned EncodeType, unsigned BytesPerPixel, std::uint32_t Pitch,
	std::uint32_t X, std::uint32_t Y, std::uint8_t* Rgba)
{
	const std::uint8_t* Source = static_cast<const std::uint8_t*>(Desc->lpSurface) +
		static_cast<std::size_t>(Y) * Pitch + static_cast<std::size_t>(X) * BytesPerPixel;
	const std::uint32_t Pixel = ReadLittleEndianPixel(Source, BytesPerPixel);
	std::uint8_t Alpha = 255;
	std::uint32_t ColorKeyPixel = Pixel;
	const unsigned RgbMode = EncodeType & _S3TC_ENCODE_RGB_MASK;

	if (Palette && BytesPerPixel == 1)
	{
		const PALETTEENTRY& Entry = Palette[Pixel];
		Rgba[0] = Entry.peRed;
		Rgba[1] = Entry.peGreen;
		Rgba[2] = Entry.peBlue;
		if (RgbMode == S3TC_ENCODE_RGB_ALPHA_COMPARE)
			Alpha = Entry.peFlags;
	}
	else
	{
		const DDPIXELFORMAT& Format = Desc->ddpfPixelFormat;
		Rgba[0] = ExtractChannel(Pixel, Format.dwRBitMask);
		Rgba[1] = ExtractChannel(Pixel, Format.dwGBitMask);
		Rgba[2] = ExtractChannel(Pixel, Format.dwBBitMask);
		if (RgbMode == S3TC_ENCODE_RGB_ALPHA_COMPARE && Format.dwRGBAlphaBitMask != 0)
			Alpha = ExtractChannel(Pixel, Format.dwRGBAlphaBitMask);
		ColorKeyPixel &= Format.dwRBitMask | Format.dwGBitMask | Format.dwBBitMask;
	}

	if (RgbMode == S3TC_ENCODE_RGB_COLOR_KEY)
	{
		const std::uint32_t Low = Desc->ddckCKSrcBlt.dwColorSpaceLowValue;
		const std::uint32_t High = Desc->ddckCKSrcBlt.dwColorSpaceHighValue;
		Alpha = ColorKeyPixel >= Low && ColorKeyPixel <= High ? 0 : 255;
	}
	else if (RgbMode == S3TC_ENCODE_RGB_ALPHA_COMPARE)
	{
		Alpha = Alpha < GAlphaReference.load(std::memory_order_relaxed) ? 0 : 255;
	}
	else
	{
		Alpha = 255;
	}
	Rgba[3] = Alpha;
}

void SetCompressedDescription(DDSURFACEDESC* Desc, std::uint32_t Width,
	std::uint32_t Height, void* Surface, unsigned Size)
{
	std::memset(Desc, 0, sizeof(*Desc));
	Desc->dwSize = sizeof(*Desc);
	Desc->dwFlags = DDSD_WIDTH | DDSD_HEIGHT | DDSD_LPSURFACE |
		DDSD_PIXELFORMAT | DDSD_LINEARSIZE_VALUE;
	Desc->dwWidth = Width;
	Desc->dwHeight = Height;
	Desc->dwLinearSize = Size;
	Desc->lpSurface = Surface;
	Desc->ddpfPixelFormat.dwSize = sizeof(DDPIXELFORMAT);
	Desc->ddpfPixelFormat.dwFlags = DDPF_FOURCC;
	Desc->ddpfPixelFormat.dwFourCC = DXT1_FOURCC;
	Desc->ddpfPixelFormat.dwRGBBitCount = 4;
}

void SetDecodedDescription(DDSURFACEDESC* Desc, std::uint32_t Width,
	std::uint32_t Height, void* Surface)
{
	std::memset(Desc, 0, sizeof(*Desc));
	Desc->dwSize = sizeof(*Desc);
	Desc->dwFlags = DDSD_WIDTH | DDSD_HEIGHT | DDSD_LPSURFACE |
		DDSD_PITCH | DDSD_PIXELFORMAT;
	Desc->dwWidth = Width;
	Desc->dwHeight = Height;
	Desc->lPitch = static_cast<std::int32_t>(Width * 4u);
	Desc->lpSurface = Surface;
	Desc->ddpfPixelFormat.dwSize = sizeof(DDPIXELFORMAT);
	Desc->ddpfPixelFormat.dwFlags = DDPF_RGB | DDPF_ALPHAPIXELS_VALUE;
	Desc->ddpfPixelFormat.dwRGBBitCount = 32;
	Desc->ddpfPixelFormat.dwRBitMask = 0x00ff0000u;
	Desc->ddpfPixelFormat.dwGBitMask = 0x0000ff00u;
	Desc->ddpfPixelFormat.dwBBitMask = 0x000000ffu;
	Desc->ddpfPixelFormat.dwRGBAlphaBitMask = 0xff000000u;
}
} // namespace

extern "C" void S3TCsetAlphaReference(int Reference)
{
	if (Reference < 0)
		Reference = 0;
	else if (Reference > 255)
		Reference = 255;
	GAlphaReference.store(Reference, std::memory_order_relaxed);
}

extern "C" unsigned S3TCgetEncodeSize(DDSURFACEDESC* Desc, unsigned EncodeType)
{
	unsigned Size = 0;
	return SupportedEncodeType(EncodeType) && Dxt1Size(Desc, Size) ? Size : 0;
}

extern "C" void S3TCencode(DDSURFACEDESC* Source, PALETTEENTRY* Palette,
	DDSURFACEDESC* Destination, void* DestinationBuffer, unsigned EncodeType, float* Weight)
{
	unsigned OutputSize = 0;
	unsigned BytesPerPixel = 0;
	std::uint32_t SourcePitch = 0;
	if (!Destination || Destination == Source || !DestinationBuffer
		|| !SupportedEncodeType(EncodeType) || !Dxt1Size(Source, OutputSize)
		|| !SourceLayout(Source, Palette, EncodeType, BytesPerPixel, SourcePitch))
		return;

	const std::uint32_t BlocksWide = (Source->dwWidth + 3u) / 4u;
	const std::uint32_t BlocksHigh = (Source->dwHeight + 3u) / 4u;
	std::uint8_t* Output = static_cast<std::uint8_t*>(DestinationBuffer);
	const int Flags = squish::kDxt1 | squish::kColourClusterFit;

	for (std::uint32_t BlockY = 0; BlockY < BlocksHigh; ++BlockY)
	{
		for (std::uint32_t BlockX = 0; BlockX < BlocksWide; ++BlockX)
		{
			std::uint8_t Rgba[4 * 4 * 4] = {};
			int Mask = 0;
			for (std::uint32_t LocalY = 0; LocalY < 4; ++LocalY)
			{
				const std::uint32_t Y = BlockY * 4u + LocalY;
				if (Y >= Source->dwHeight)
					continue;
				for (std::uint32_t LocalX = 0; LocalX < 4; ++LocalX)
				{
					const std::uint32_t X = BlockX * 4u + LocalX;
					if (X >= Source->dwWidth)
						continue;
					const unsigned Texel = LocalY * 4u + LocalX;
					Mask |= 1 << Texel;
					ReadRgba(Source, Palette, EncodeType, BytesPerPixel, SourcePitch,
						X, Y, &Rgba[Texel * 4u]);
				}
			}
			squish::CompressMasked(Rgba, Mask, Output, Flags, Weight);
			Output += DXT1_BLOCK_BYTES;
		}
	}

	SetCompressedDescription(Destination, Source->dwWidth, Source->dwHeight,
		DestinationBuffer, OutputSize);
}

extern "C" unsigned S3TCgetDecodeSize(DDSURFACEDESC* Desc)
{
	unsigned Size = 0;
	return DecodedSize(Desc, Size) ? Size : 0;
}

extern "C" void S3TCdecode(DDSURFACEDESC* Source, DDSURFACEDESC* Destination,
	void* DestinationBuffer)
{
	unsigned InputSize = 0;
	unsigned OutputSize = 0;
	if (!Destination || Destination == Source || !DestinationBuffer || !Source || !Source->lpSurface
		|| !Dxt1Size(Source, InputSize) || !DecodedSize(Source, OutputSize))
		return;
	static_cast<void>(OutputSize);

	const std::uint32_t BlocksWide = (Source->dwWidth + 3u) / 4u;
	const std::uint32_t BlocksHigh = (Source->dwHeight + 3u) / 4u;
	const std::uint32_t TightBlockPitch = BlocksWide * DXT1_BLOCK_BYTES;
	std::uint32_t BlockPitch = TightBlockPitch;
	if (Source->dwFlags & DDSD_PITCH)
	{
		if (Source->lPitch <= 0 || static_cast<std::uint32_t>(Source->lPitch) < TightBlockPitch)
			return;
		BlockPitch = static_cast<std::uint32_t>(Source->lPitch);
	}
	const std::uint64_t RequiredInputBytes =
		static_cast<std::uint64_t>(BlocksHigh - 1u) * BlockPitch + TightBlockPitch;
	if ((Source->dwFlags & DDSD_LINEARSIZE_VALUE)
		&& static_cast<std::uint64_t>(Source->dwLinearSize) < RequiredInputBytes)
		return;

	const std::uint8_t* Blocks = static_cast<const std::uint8_t*>(Source->lpSurface);
	std::uint8_t* Output = static_cast<std::uint8_t*>(DestinationBuffer);
	const std::size_t OutputPitch = static_cast<std::size_t>(Source->dwWidth) * 4u;

	for (std::uint32_t BlockY = 0; BlockY < BlocksHigh; ++BlockY)
	{
		for (std::uint32_t BlockX = 0; BlockX < BlocksWide; ++BlockX)
		{
			std::uint8_t Rgba[4 * 4 * 4];
			const std::uint8_t* Block = Blocks +
				static_cast<std::size_t>(BlockY) * BlockPitch + BlockX * DXT1_BLOCK_BYTES;
			squish::Decompress(Rgba, Block, squish::kDxt1);
			for (std::uint32_t LocalY = 0; LocalY < 4; ++LocalY)
			{
				const std::uint32_t Y = BlockY * 4u + LocalY;
				if (Y >= Source->dwHeight)
					continue;
				for (std::uint32_t LocalX = 0; LocalX < 4; ++LocalX)
				{
					const std::uint32_t X = BlockX * 4u + LocalX;
					if (X >= Source->dwWidth)
						continue;
					const std::uint8_t* Pixel = &Rgba[(LocalY * 4u + LocalX) * 4u];
					std::uint8_t* Dest = Output + static_cast<std::size_t>(Y) * OutputPitch + X * 4u;
					Dest[0] = Pixel[2];
					Dest[1] = Pixel[1];
					Dest[2] = Pixel[0];
					Dest[3] = Pixel[3];
				}
			}
		}
	}

	SetDecodedDescription(Destination, Source->dwWidth, Source->dwHeight, DestinationBuffer);
}

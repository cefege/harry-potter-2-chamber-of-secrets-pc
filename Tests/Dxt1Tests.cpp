#include "S3tc.h"

#include <algorithm>
#include <array>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <limits>
#include <vector>

namespace
{
constexpr std::uint32_t DDSD_LINEARSIZE_VALUE = 0x00080000u;
int Failures = 0;

void Check(bool Condition, const char* Message)
{
	if (!Condition)
	{
		std::fprintf(stderr, "dxt1_codec: %s\n", Message);
		++Failures;
	}
}

DDSURFACEDESC MakeBgraSource(void* Pixels, std::uint32_t Width, std::uint32_t Height,
	std::int32_t Pitch = 0)
{
	DDSURFACEDESC Desc = {};
	Desc.dwSize = sizeof(Desc);
	Desc.dwFlags = DDSD_WIDTH | DDSD_HEIGHT | DDSD_LPSURFACE | DDSD_PITCH | DDSD_PIXELFORMAT;
	Desc.dwWidth = Width;
	Desc.dwHeight = Height;
	Desc.lPitch = Pitch != 0 ? Pitch : static_cast<std::int32_t>(Width * 4u);
	Desc.lpSurface = Pixels;
	Desc.ddpfPixelFormat.dwSize = sizeof(DDPIXELFORMAT);
	Desc.ddpfPixelFormat.dwFlags = DDPF_RGB | 0x00000001u;
	Desc.ddpfPixelFormat.dwRGBBitCount = 32;
	Desc.ddpfPixelFormat.dwRBitMask = 0x00ff0000u;
	Desc.ddpfPixelFormat.dwGBitMask = 0x0000ff00u;
	Desc.ddpfPixelFormat.dwBBitMask = 0x000000ffu;
	Desc.ddpfPixelFormat.dwRGBAlphaBitMask = 0xff000000u;
	return Desc;
}

DDSURFACEDESC MakeDxtSource(const void* Blocks, std::uint32_t Width, std::uint32_t Height,
	std::uint32_t Bytes)
{
	DDSURFACEDESC Desc = {};
	Desc.dwSize = sizeof(Desc);
	Desc.dwFlags = DDSD_WIDTH | DDSD_HEIGHT | DDSD_LPSURFACE |
		DDSD_PIXELFORMAT | DDSD_LINEARSIZE_VALUE;
	Desc.dwWidth = Width;
	Desc.dwHeight = Height;
	Desc.dwLinearSize = Bytes;
	Desc.lpSurface = const_cast<void*>(Blocks);
	Desc.ddpfPixelFormat.dwSize = sizeof(DDPIXELFORMAT);
	Desc.ddpfPixelFormat.dwFlags = DDPF_FOURCC;
	Desc.ddpfPixelFormat.dwRGBBitCount = 4;
	return Desc;
}

std::vector<std::uint8_t> Decode(const void* Blocks, std::uint32_t Width,
	std::uint32_t Height, std::uint32_t Bytes, DDSURFACEDESC* ResultDesc = nullptr)
{
	DDSURFACEDESC Source = MakeDxtSource(Blocks, Width, Height, Bytes);
	DDSURFACEDESC Destination = {};
	std::vector<std::uint8_t> Result(static_cast<std::size_t>(Width) * Height * 4u, 0xcd);
	S3TCdecode(&Source, &Destination, Result.data());
	if (ResultDesc)
		*ResultDesc = Destination;
	return Result;
}

void TestKnownFourColourBlock()
{
	const std::array<std::uint8_t, 8> Block = {0x00, 0xf8, 0xe0, 0x07,
		0xe4, 0xe4, 0xe4, 0xe4};
	DDSURFACEDESC Desc = {};
	const std::vector<std::uint8_t> Pixels = Decode(Block.data(), 4, 4, Block.size(), &Desc);
	const std::array<std::array<std::uint8_t, 4>, 4> Expected = {{
		{{0, 0, 255, 255}}, {{0, 255, 0, 255}},
		{{0, 85, 170, 255}}, {{0, 170, 85, 255}},
	}};
	for (unsigned Y = 0; Y < 4; ++Y)
		for (unsigned X = 0; X < 4; ++X)
			Check(std::memcmp(&Pixels[(Y * 4u + X) * 4u], Expected[X].data(), 4) == 0,
				"known four-colour decode changed BGRA channel or DXT index order");
	Check(Desc.dwWidth == 4 && Desc.dwHeight == 4 && Desc.lPitch == 16,
		"decode destination dimensions or pitch are wrong");
	Check(Desc.ddpfPixelFormat.dwRGBBitCount == 32
		&& Desc.ddpfPixelFormat.dwRBitMask == 0x00ff0000u
		&& Desc.ddpfPixelFormat.dwBBitMask == 0x000000ffu,
		"decode destination is not ARGB8888/XRGB word order");
}

void TestKnownTransparentBlock()
{
	const std::array<std::uint8_t, 8> Block = {0x00, 0x00, 0xff, 0xff,
		0xe4, 0xe4, 0xe4, 0xe4};
	const std::vector<std::uint8_t> Pixels = Decode(Block.data(), 4, 4, Block.size());
	const std::array<std::array<std::uint8_t, 4>, 4> Expected = {{
		{{0, 0, 0, 255}}, {{255, 255, 255, 255}},
		{{127, 127, 127, 255}}, {{0, 0, 0, 0}},
	}};
	for (unsigned X = 0; X < 4; ++X)
		Check(std::memcmp(&Pixels[X * 4u], Expected[X].data(), 4) == 0,
			"known transparent decode changed three-colour or mask semantics");
}

void TestBlockCroppingAndSizes()
{
	const std::array<std::uint8_t, 16> Blocks = {
		0x00, 0xf8, 0x00, 0x00, 0, 0, 0, 0,
		0xe0, 0x07, 0x00, 0x00, 0, 0, 0, 0,
	};
	DDSURFACEDESC Source = MakeDxtSource(Blocks.data(), 5, 3, Blocks.size());
	Check(S3TCgetEncodeSize(&Source, S3TC_ENCODE_RGB_FULL) == 16,
		"DXT1 size did not round dimensions to complete 4x4 blocks");
	Check(S3TCgetDecodeSize(&Source) == 5u * 3u * 4u,
		"decoded size is not width times height times four");
	const std::vector<std::uint8_t> Pixels = Decode(Blocks.data(), 5, 3, Blocks.size());
	for (unsigned Y = 0; Y < 3; ++Y)
	{
		Check(Pixels[(Y * 5u) * 4u + 2u] == 255,
			"first DXT block did not decode as red");
		Check(Pixels[(Y * 5u + 4u) * 4u + 1u] == 255,
			"cropped second DXT block did not decode as green");
	}
}

void TestDeterministicRoundTrip()
{
	constexpr std::uint32_t Width = 8;
	constexpr std::uint32_t Height = 8;
	std::vector<std::uint8_t> SourcePixels(Width * Height * 4u);
	const std::array<std::array<std::uint8_t, 4>, 4> Colours = {{
		{{17, 65, 231, 255}}, {{209, 33, 74, 255}},
		{{91, 222, 28, 255}}, {{245, 138, 51, 255}},
	}};
	for (std::uint32_t Y = 0; Y < Height; ++Y)
		for (std::uint32_t X = 0; X < Width; ++X)
			std::memcpy(&SourcePixels[(Y * Width + X) * 4u],
				Colours[(Y / 4u) * 2u + X / 4u].data(), 4);

	DDSURFACEDESC Source = MakeBgraSource(SourcePixels.data(), Width, Height);
	const unsigned EncodedSize = S3TCgetEncodeSize(&Source, S3TC_ENCODE_RGB_FULL);
	Check(EncodedSize == 32, "8x8 DXT1 storage size is not four blocks");
	std::vector<std::uint8_t> EncodedA(EncodedSize, 0xa5);
	std::vector<std::uint8_t> EncodedB(EncodedSize, 0x5a);
	DDSURFACEDESC EncodedDescA = {};
	DDSURFACEDESC EncodedDescB = {};
	const float Weight[3] = {0.309f, 0.609f, 0.082f};
	S3TCencode(&Source, nullptr, &EncodedDescA, EncodedA.data(), S3TC_ENCODE_RGB_FULL,
		const_cast<float*>(Weight));
	S3TCencode(&Source, nullptr, &EncodedDescB, EncodedB.data(), S3TC_ENCODE_RGB_FULL,
		const_cast<float*>(Weight));
	Check(EncodedA == EncodedB, "DXT1 encoder output is not deterministic");
	Check(EncodedDescA.dwLinearSize == EncodedSize && EncodedDescA.lpSurface == EncodedA.data(),
		"encode destination descriptor does not describe the DXT1 buffer");

	const std::vector<std::uint8_t> Decoded = Decode(EncodedA.data(), Width, Height, EncodedSize);
	unsigned MaxError = 0;
	for (std::size_t Index = 0; Index < SourcePixels.size(); Index += 4)
	{
		for (unsigned Channel = 0; Channel < 3; ++Channel)
		{
			const int Error = static_cast<int>(Decoded[Index + Channel]) - SourcePixels[Index + Channel];
			MaxError = std::max(MaxError, static_cast<unsigned>(Error < 0 ? -Error : Error));
		}
		Check(Decoded[Index + 3] == 255, "opaque DXT1 roundtrip lost alpha");
	}
	Check(MaxError <= 8, "solid-colour DXT1 roundtrip exceeded RGB565 quantization bound");
}

void TestAlphaCompareAndColourKey()
{
	std::vector<std::uint8_t> Pixels(4u * 4u * 4u);
	for (unsigned Index = 0; Index < 16; ++Index)
	{
		Pixels[Index * 4u] = 9;
		Pixels[Index * 4u + 1u] = 41;
		Pixels[Index * 4u + 2u] = 233;
		Pixels[Index * 4u + 3u] = (Index & 1u) ? 128 : 127;
	}
	DDSURFACEDESC Source = MakeBgraSource(Pixels.data(), 4, 4);
	DDSURFACEDESC EncodedDesc = {};
	std::array<std::uint8_t, 8> Encoded = {};
	S3TCsetAlphaReference(128);
	S3TCencode(&Source, nullptr, &EncodedDesc, Encoded.data(),
		S3TC_ENCODE_RGB_ALPHA_COMPARE | S3TC_ENCODE_ALPHA_NONE, nullptr);
	const std::uint16_t C0 = static_cast<std::uint16_t>(Encoded[0] | Encoded[1] << 8);
	const std::uint16_t C1 = static_cast<std::uint16_t>(Encoded[2] | Encoded[3] << 8);
	Check(C0 <= C1, "alpha-compare encode did not select DXT1 masked mode");
	const std::vector<std::uint8_t> Decoded = Decode(Encoded.data(), 4, 4, Encoded.size());
	for (unsigned Index = 0; Index < 16; ++Index)
		Check(Decoded[Index * 4u + 3u] == ((Index & 1u) ? 255 : 0),
			"alpha reference boundary or DXT1 transparency was not preserved");

	for (unsigned Index = 0; Index < 16; ++Index)
	{
		const bool Keyed = (Index & 1u) == 0;
		Pixels[Index * 4u] = 0;
		Pixels[Index * 4u + 1u] = Keyed ? 0 : 255;
		Pixels[Index * 4u + 2u] = 0;
		Pixels[Index * 4u + 3u] = 255;
	}
	Source = MakeBgraSource(Pixels.data(), 4, 4);
	Source.ddckCKSrcBlt.dwColorSpaceLowValue = 0;
	Source.ddckCKSrcBlt.dwColorSpaceHighValue = 0;
	Encoded.fill(0);
	EncodedDesc = {};
	S3TCencode(&Source, nullptr, &EncodedDesc, Encoded.data(),
		S3TC_ENCODE_RGB_COLOR_KEY | S3TC_ENCODE_ALPHA_NONE, nullptr);
	const std::vector<std::uint8_t> KeyDecoded = Decode(Encoded.data(), 4, 4, Encoded.size());
	for (unsigned Index = 0; Index < 16; ++Index)
		Check(KeyDecoded[Index * 4u + 3u] == ((Index & 1u) ? 255 : 0),
			"RGB colour-key encode did not preserve one-bit mask");
}

void TestMalformedInputsDoNotWrite()
{
	DDSURFACEDESC Invalid = {};
	Check(S3TCgetEncodeSize(nullptr, S3TC_ENCODE_RGB_FULL) == 0,
		"null encode descriptor was accepted");
	Check(S3TCgetDecodeSize(nullptr) == 0, "null decode descriptor was accepted");
	Check(S3TCgetEncodeSize(&Invalid, S3TC_ENCODE_RGB_FULL) == 0,
		"descriptor with missing size and dimensions was accepted");
	Invalid.dwSize = sizeof(Invalid);
	Invalid.dwWidth = std::numeric_limits<std::uint32_t>::max();
	Invalid.dwHeight = std::numeric_limits<std::uint32_t>::max();
	Check(S3TCgetEncodeSize(&Invalid, S3TC_ENCODE_RGB_FULL) == 0,
		"overflowing DXT block count was accepted");
	Check(S3TCgetDecodeSize(&Invalid) == 0, "overflowing decoded byte count was accepted");

	std::array<std::uint8_t, 64> Pixels = {};
	DDSURFACEDESC Source = MakeBgraSource(Pixels.data(), 4, 4, 15);
	std::array<std::uint8_t, 8> Encoded;
	Encoded.fill(0x6d);
	DDSURFACEDESC Destination = {};
	S3TCencode(&Source, nullptr, &Destination, Encoded.data(), S3TC_ENCODE_RGB_FULL, nullptr);
	Check(std::all_of(Encoded.begin(), Encoded.end(), [](std::uint8_t Byte) { return Byte == 0x6d; }),
		"encoder wrote output for a source pitch smaller than one row");
	Check(Destination.dwFlags == 0, "failed encode mutated its destination descriptor");

	Source = MakeBgraSource(Pixels.data(), 4, 4);
	Check(S3TCgetEncodeSize(&Source, S3TC_ENCODE_RGB_FULL | S3TC_ENCODE_ALPHA_EXPLICIT) == 0,
		"unsupported non-DXT1 alpha mode reported storage");
	S3TCencode(&Source, nullptr, &Destination, Encoded.data(),
		S3TC_ENCODE_RGB_FULL | S3TC_ENCODE_ALPHA_EXPLICIT, nullptr);
	Check(std::all_of(Encoded.begin(), Encoded.end(), [](std::uint8_t Byte) { return Byte == 0x6d; }),
		"unsupported non-DXT1 alpha mode wrote output");

	const std::array<std::uint8_t, 8> Block = {0x00, 0xf8, 0, 0, 0, 0, 0, 0};
	DDSURFACEDESC Compressed = MakeDxtSource(Block.data(), 4, 4, 7);
	std::array<std::uint8_t, 64> Decoded;
	Decoded.fill(0x39);
	Destination = {};
	S3TCdecode(&Compressed, &Destination, Decoded.data());
	Check(std::all_of(Decoded.begin(), Decoded.end(), [](std::uint8_t Byte) { return Byte == 0x39; }),
		"decoder read a DXT1 payload shorter than one block");
	Check(Destination.dwFlags == 0, "failed decode mutated its destination descriptor");

	Compressed = MakeDxtSource(Block.data(), 4, 4, Block.size());
	Compressed.dwFlags &= ~DDSD_LINEARSIZE_VALUE;
	Compressed.dwFlags |= DDSD_PITCH;
	Compressed.lPitch = 7;
	S3TCdecode(&Compressed, &Destination, Decoded.data());
	Check(std::all_of(Decoded.begin(), Decoded.end(), [](std::uint8_t Byte) { return Byte == 0x39; }),
		"decoder accepted a compressed pitch smaller than one DXT1 block");

	const std::array<std::uint8_t, 16> PitchedBlocks = {
		0x00, 0xf8, 0, 0, 0, 0, 0, 0,
		0xe0, 0x07, 0, 0, 0, 0, 0, 0,
	};
	Compressed = MakeDxtSource(PitchedBlocks.data(), 4, 8, PitchedBlocks.size());
	Compressed.dwFlags |= DDSD_PITCH;
	// DDSURFACEDESC aliases lPitch and dwLinearSize. A value of 16 therefore
	// declares 16 available bytes but needs 24 bytes to reach the second row.
	std::array<std::uint8_t, 128> StrideDecoded;
	StrideDecoded.fill(0x7b);
	Destination = {};
	S3TCdecode(&Compressed, &Destination, StrideDecoded.data());
	Check(std::all_of(StrideDecoded.begin(), StrideDecoded.end(),
		[](std::uint8_t Byte) { return Byte == 0x7b; }),
		"decoder accepted a payload shorter than its pitched block-row extent");
	Check(Destination.dwFlags == 0,
		"failed pitched decode mutated its destination descriptor");
}
} // namespace

int main()
{
	TestKnownFourColourBlock();
	TestKnownTransparentBlock();
	TestBlockCroppingAndSizes();
	TestDeterministicRoundTrip();
	TestAlphaCompareAndColourKey();
	TestMalformedInputsDoNotWrite();
	if (Failures != 0)
	{
		std::fprintf(stderr, "dxt1_codec: %d checks failed\n", Failures);
		return 1;
	}
	std::puts("dxt1_codec: all checks passed");
	return 0;
}

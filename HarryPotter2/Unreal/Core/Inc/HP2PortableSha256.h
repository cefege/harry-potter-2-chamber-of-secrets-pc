/*=============================================================================
	HP2PortableSha256.h: Cross-platform SHA-256, exposing the CommonCrypto
	CC_SHA256_* names so call sites written against CommonCrypto need zero
	changes on non-Apple hosts. Include this header instead of
	<CommonCrypto/CommonDigest.h>; on Apple hosts it simply forwards there.
=============================================================================*/
#ifndef HP2PORTABLESHA256_H
#define HP2PORTABLESHA256_H

#if defined(__APPLE__)

#include <CommonCrypto/CommonDigest.h>

#else

#include <cstddef>
#include <cstdint>
#include <cstring>

// Minimal, self-contained SHA-256 (FIPS 180-4) matching CommonCrypto's
// classic init/update/final call shape so existing CC_SHA256_* call sites
// are portable unchanged.
typedef std::uint32_t CC_LONG;
#define CC_SHA256_DIGEST_LENGTH 32

struct CC_SHA256_CTX
{
	std::uint32_t State[8];
	std::uint64_t BitCount;
	unsigned char Buffer[64];
	std::size_t BufferLength;
};

namespace HP2PortableSha256Detail
{
	inline std::uint32_t RotateRight(std::uint32_t Value, unsigned Bits)
	{
		return (Value >> Bits) | (Value << (32 - Bits));
	}

	inline void Transform(std::uint32_t State[8], const unsigned char Block[64])
	{
		static const std::uint32_t RoundConstants[64] = {
			0x428a2f98u,0x71374491u,0xb5c0fbcfu,0xe9b5dba5u,0x3956c25bu,0x59f111f1u,0x923f82a4u,0xab1c5ed5u,
			0xd807aa98u,0x12835b01u,0x243185beu,0x550c7dc3u,0x72be5d74u,0x80deb1feu,0x9bdc06a7u,0xc19bf174u,
			0xe49b69c1u,0xefbe4786u,0x0fc19dc6u,0x240ca1ccu,0x2de92c6fu,0x4a7484aau,0x5cb0a9dcu,0x76f988dau,
			0x983e5152u,0xa831c66du,0xb00327c8u,0xbf597fc7u,0xc6e00bf3u,0xd5a79147u,0x06ca6351u,0x14292967u,
			0x27b70a85u,0x2e1b2138u,0x4d2c6dfcu,0x53380d13u,0x650a7354u,0x766a0abbu,0x81c2c92eu,0x92722c85u,
			0xa2bfe8a1u,0xa81a664bu,0xc24b8b70u,0xc76c51a3u,0xd192e819u,0xd6990624u,0xf40e3585u,0x106aa070u,
			0x19a4c116u,0x1e376c08u,0x2748774cu,0x34b0bcb5u,0x391c0cb3u,0x4ed8aa4au,0x5b9cca4fu,0x682e6ff3u,
			0x748f82eeu,0x78a5636fu,0x84c87814u,0x8cc70208u,0x90befffau,0xa4506cebu,0xbef9a3f7u,0xc67178f2u
		};

		std::uint32_t Schedule[64];
		for (int Index = 0; Index < 16; ++Index)
		{
			Schedule[Index] =
				(static_cast<std::uint32_t>(Block[Index * 4 + 0]) << 24) |
				(static_cast<std::uint32_t>(Block[Index * 4 + 1]) << 16) |
				(static_cast<std::uint32_t>(Block[Index * 4 + 2]) << 8) |
				(static_cast<std::uint32_t>(Block[Index * 4 + 3]));
		}
		for (int Index = 16; Index < 64; ++Index)
		{
			const std::uint32_t S0 = RotateRight(Schedule[Index - 15], 7)
				^ RotateRight(Schedule[Index - 15], 18) ^ (Schedule[Index - 15] >> 3);
			const std::uint32_t S1 = RotateRight(Schedule[Index - 2], 17)
				^ RotateRight(Schedule[Index - 2], 19) ^ (Schedule[Index - 2] >> 10);
			Schedule[Index] = Schedule[Index - 16] + S0 + Schedule[Index - 7] + S1;
		}

		std::uint32_t A = State[0], B = State[1], C = State[2], D = State[3];
		std::uint32_t E = State[4], F = State[5], G = State[6], H = State[7];

		for (int Index = 0; Index < 64; ++Index)
		{
			const std::uint32_t S1 = RotateRight(E, 6) ^ RotateRight(E, 11) ^ RotateRight(E, 25);
			const std::uint32_t Choose = (E & F) ^ (~E & G);
			const std::uint32_t Temp1 = H + S1 + Choose + RoundConstants[Index] + Schedule[Index];
			const std::uint32_t S0 = RotateRight(A, 2) ^ RotateRight(A, 13) ^ RotateRight(A, 22);
			const std::uint32_t Majority = (A & B) ^ (A & C) ^ (B & C);
			const std::uint32_t Temp2 = S0 + Majority;

			H = G; G = F; F = E; E = D + Temp1;
			D = C; C = B; B = A; A = Temp1 + Temp2;
		}

		State[0] += A; State[1] += B; State[2] += C; State[3] += D;
		State[4] += E; State[5] += F; State[6] += G; State[7] += H;
	}
}

inline int CC_SHA256_Init(CC_SHA256_CTX* Context)
{
	static const std::uint32_t InitialState[8] = {
		0x6a09e667u,0xbb67ae85u,0x3c6ef372u,0xa54ff53au,
		0x510e527fu,0x9b05688cu,0x1f83d9abu,0x5be0cd19u
	};
	std::memcpy(Context->State, InitialState, sizeof(InitialState));
	Context->BitCount = 0;
	Context->BufferLength = 0;
	return 1;
}

inline int CC_SHA256_Update(CC_SHA256_CTX* Context, const void* Data, CC_LONG Length)
{
	const unsigned char* Bytes = static_cast<const unsigned char*>(Data);
	Context->BitCount += static_cast<std::uint64_t>(Length) * 8;

	while (Length > 0)
	{
		const std::size_t Take = (64 - Context->BufferLength < Length)
			? (64 - Context->BufferLength) : static_cast<std::size_t>(Length);
		std::memcpy(Context->Buffer + Context->BufferLength, Bytes, Take);
		Context->BufferLength += Take;
		Bytes += Take;
		Length -= static_cast<CC_LONG>(Take);

		if (Context->BufferLength == 64)
		{
			HP2PortableSha256Detail::Transform(Context->State, Context->Buffer);
			Context->BufferLength = 0;
		}
	}
	return 1;
}

inline int CC_SHA256_Final(unsigned char* Digest, CC_SHA256_CTX* Context)
{
	const std::uint64_t BitCount = Context->BitCount;

	unsigned char Pad = 0x80;
	CC_SHA256_Update(Context, &Pad, 1);
	Pad = 0x00;
	while (Context->BufferLength != 56)
		CC_SHA256_Update(Context, &Pad, 1);

	unsigned char LengthBytes[8];
	for (int Index = 0; Index < 8; ++Index)
		LengthBytes[Index] = static_cast<unsigned char>(BitCount >> (56 - Index * 8));
	CC_SHA256_Update(Context, LengthBytes, 8);

	for (int Word = 0; Word < 8; ++Word)
	{
		Digest[Word * 4 + 0] = static_cast<unsigned char>(Context->State[Word] >> 24);
		Digest[Word * 4 + 1] = static_cast<unsigned char>(Context->State[Word] >> 16);
		Digest[Word * 4 + 2] = static_cast<unsigned char>(Context->State[Word] >> 8);
		Digest[Word * 4 + 3] = static_cast<unsigned char>(Context->State[Word]);
	}
	return 1;
}

inline unsigned char* CC_SHA256(const void* Data, CC_LONG Length, unsigned char* Digest)
{
	CC_SHA256_CTX Context;
	CC_SHA256_Init(&Context);
	CC_SHA256_Update(&Context, Data, Length);
	CC_SHA256_Final(Digest, &Context);
	return Digest;
}

#endif // !__APPLE__

#endif // HP2PORTABLESHA256_H

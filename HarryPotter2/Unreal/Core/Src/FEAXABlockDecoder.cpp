/*=============================================================================
	FEAXABlockDecoder.cpp: Allocation-free mono EA-XA block decoding.
=============================================================================*/

#include "Core.h"

namespace
{
	struct FXACoefficients
	{
		INT C0;
		INT C1;
	};

	// EA's extended-XA N=8 predictor table from pinned vgmstream commit
	// 11508e91a069066d0f6526d0096568b46c354014. See
	// ThirdParty/vgmstream/COPYING for the retained ISC-style notice.
	static const FXACoefficients GCoefficients[16] =
	{
		{   0,    0 },
		{ 240,    0 },
		{ 460, -208 },
		{ 392, -220 },
		{ 488, -240 },
		{ 328, -208 },
		{ 440, -168 },
		{ 420, -188 },
		{ 432, -176 },
		{ 240,  -16 },
		{ 416, -192 },
		{ 424, -160 },
		{ 288,   -8 },
		{ 436, -188 },
		{ 224,   -1 },
		{ 272,  -16 }
	};

	static INT FloorDivide( INT Value, INT Divisor )
	{
		return Value >= 0
			? Value / Divisor
			: -static_cast<INT>((-static_cast<long long>(Value) + Divisor - 1) / Divisor);
	}

	static INT ScaleNibble( INT Nibble, INT Range )
	{
		if( Range <= 12 )
			return Nibble * (1 << (12 - Range));
		return FloorDivide( Nibble, 1 << (Range - 12) );
	}

	static SWORD ClampSample( INT Sample )
	{
		if( Sample > MAXSWORD )
			return static_cast<SWORD>(MAXSWORD);
		if( Sample < -MAXSWORD - 1 )
			return static_cast<SWORD>(-MAXSWORD - 1);
		return static_cast<SWORD>(Sample);
	}
}

FEAXABlockDecoder::FEAXABlockDecoder()
: EncodedData(NULL)
, EncodedBytes(0)
, EncodedOffset(0)
, SamplesRemaining(0)
, History1(0)
, History2(0)
, ResidueOffset(0)
, ResidueSamples(0)
{
}

UBOOL FEAXABlockDecoder::Feed( const BYTE* Data, INT Bytes, INT NumSamples )
{
	if( Bytes < 0 || NumSamples < 0 || Bytes % 15 != 0 )
		return 0;
	if( Bytes > 0 && Data == NULL )
		return 0;

	const long long Capacity = static_cast<long long>(Bytes / 15) * 28;
	if( static_cast<long long>(NumSamples) > Capacity )
		return 0;

	EncodedData = Data;
	EncodedBytes = Bytes;
	EncodedOffset = 0;
	SamplesRemaining = NumSamples;
	ResidueOffset = 0;
	ResidueSamples = 0;
	return 1;
}

INT FEAXABlockDecoder::Decode( SWORD* Dest, INT Samples )
{
	if( Dest == NULL || Samples <= 0 )
		return 0;

	INT Produced = 0;
	while( Produced < Samples )
	{
		if( ResidueSamples > 0 )
		{
			INT CopySamples = Samples - Produced;
			if( CopySamples > ResidueSamples )
				CopySamples = ResidueSamples;
			for( INT Index=0; Index<CopySamples; ++Index )
				Dest[Produced + Index] = Residue[ResidueOffset + Index];
			Produced += CopySamples;
			ResidueOffset += CopySamples;
			ResidueSamples -= CopySamples;
			continue;
		}

		if( SamplesRemaining <= 0 || EncodedOffset > EncodedBytes - 15 )
			break;

		const BYTE* Block = EncodedData + EncodedOffset;
		const INT Predictor = Block[0] >> 4;
		const INT Range = Block[0] & 15;
		const FXACoefficients& Coefficients = GCoefficients[Predictor];
		INT BlockSamples = SamplesRemaining;
		if( BlockSamples > 28 )
			BlockSamples = 28;

		for( INT Index=0; Index<BlockSamples; ++Index )
		{
			const BYTE Packed = Block[1 + Index / 2];
			const INT UnsignedNibble = (Index & 1) ? (Packed >> 4) : (Packed & 15);
			const INT SignedNibble = UnsignedNibble >= 8 ? UnsignedNibble - 16 : UnsignedNibble;
			const INT Sample = ScaleNibble( SignedNibble, Range );
			const long long Accumulator =
				static_cast<long long>(Sample) * 256
				+ static_cast<long long>(Coefficients.C0) * History1
				+ static_cast<long long>(Coefficients.C1) * History2;
			const INT Decoded = Accumulator >= 0
				? static_cast<INT>(Accumulator / 256)
				: -static_cast<INT>((-Accumulator + 255) / 256);
			const SWORD Clamped = ClampSample( Decoded );
			Residue[Index] = Clamped;
			History2 = History1;
			History1 = Clamped;
		}

		EncodedOffset += 15;
		SamplesRemaining -= BlockSamples;
		ResidueOffset = 0;
		ResidueSamples = BlockSamples;
	}
	return Produced;
}

void FEAXABlockDecoder::ResetState( SWORD Sample1, SWORD Sample2 )
{
	History1 = Sample1;
	History2 = Sample2;
}

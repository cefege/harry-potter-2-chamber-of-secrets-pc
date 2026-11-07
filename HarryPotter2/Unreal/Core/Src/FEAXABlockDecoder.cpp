/*=============================================================================
	FEAXABlockDecoder.cpp: Allocation-free mono EA-XA block decoding.
=============================================================================*/

#include "Core.h"

namespace
{
	// EA-XA v1 predictor table from vgmstream commit
	// 11508e91a069066d0f6526d0096568b46c354014, ea_xa_decoder.c.
	// See ThirdParty/vgmstream/COPYING for the ISC-style notice.
	static const INT GCoefficients[20] =
	{
		0, 240, 460, 392,
		0,   0, -208, -220,
		0,   1,   3,   4,
		7,   8,  10,  11,
		0,  -1,  -3,  -4
	};

	static INT FloorDivide( INT Value, INT Divisor )
	{
		return Value >= 0
			? Value / Divisor
			: -static_cast<INT>((-static_cast<long long>(Value) + Divisor - 1) / Divisor);
	}

	static INT ScaleNibble( INT Nibble, INT Range )
	{
		// Keep eight fractional bits until after the predictor is applied.
		// Division is explicitly rounded down, including negative samples.
		return FloorDivide( Nibble * (1 << 28), 1 << (Range + 8) );
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
		INT BlockSamples = SamplesRemaining;
		if( BlockSamples > 28 )
			BlockSamples = 28;

		for( INT Index=0; Index<BlockSamples; ++Index )
		{
			const BYTE Packed = Block[1 + Index / 2];
			const INT UnsignedNibble = (Index & 1) ? (Packed & 15) : (Packed >> 4);
			const INT SignedNibble = UnsignedNibble >= 8 ? UnsignedNibble - 16 : UnsignedNibble;
			const INT Sample = ScaleNibble( SignedNibble, Range );
			const long long Accumulator =
				static_cast<long long>(Sample)
				+ static_cast<long long>(GCoefficients[Predictor]) * History1
				+ static_cast<long long>(GCoefficients[Predictor + 4]) * History2 + 128;
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

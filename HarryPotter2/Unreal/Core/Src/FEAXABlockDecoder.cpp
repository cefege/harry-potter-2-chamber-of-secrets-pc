/*=============================================================================
	FEAXABlockDecoder.cpp: Allocation-free mono EA-XA block decoding.
	The predictor table, nibble scaling and block decode math now live in
	crates/hp2-codecs::eaxa; this file only wires the class to that Rust
	state through hp2_rs.h.
=============================================================================*/

#include "Core.h"
#include "hp2_rs.h"

FEAXABlockDecoder::FEAXABlockDecoder()
: Handle( hp2_rs_eaxa_new() )
{
}

FEAXABlockDecoder::~FEAXABlockDecoder()
{
	if( Handle )
		hp2_rs_eaxa_free( Handle );
}

FEAXABlockDecoder::FEAXABlockDecoder( FEAXABlockDecoder&& Other ) noexcept
: Handle( Other.Handle )
{
	Other.Handle = nullptr;
}

FEAXABlockDecoder& FEAXABlockDecoder::operator=( FEAXABlockDecoder&& Other ) noexcept
{
	if( this != &Other )
	{
		if( Handle )
			hp2_rs_eaxa_free( Handle );
		Handle = Other.Handle;
		Other.Handle = nullptr;
	}
	return *this;
}

UBOOL FEAXABlockDecoder::Feed( const BYTE* Data, INT Bytes, INT NumSamples )
{
	return hp2_rs_eaxa_feed( Handle, Data, Bytes, NumSamples ) ? 1 : 0;
}

INT FEAXABlockDecoder::Decode( SWORD* Dest, INT Samples )
{
	return hp2_rs_eaxa_decode( Handle, Dest, Samples );
}

void FEAXABlockDecoder::ResetState( SWORD Sample1, SWORD Sample2 )
{
	hp2_rs_eaxa_reset_state( Handle, Sample1, Sample2 );
}

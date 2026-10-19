// Fixture driver for the ORIGINAL palette.cpp (HeckbertQuantize) at the pinned ref.
// Each record prints the exact inputs (source pixels, destination prefill) and
// outputs (return value, palette, destination incl. mip area) so the Rust replay
// can assert bit-exact results without re-deriving any PRNG stream.
#include "EnginePrivate.h"
#include "Palette.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>

class HarnessMalloc : public FMalloc
{
public:
	void* Malloc( DWORD Count, const TCHAR* Tag ) { return malloc( Count ? Count : 1 ); }
	void* Realloc( void* Original, DWORD Count, const TCHAR* Tag ) { return realloc( Original, Count ? Count : 1 ); }
	void Free( void* Original ) { free( Original ); }
	void DumpAllocs() {}
	void HeapCheck() {}
	void Init() {}
	void Exit() {}
};
static HarnessMalloc GHarnessMalloc;
FMalloc* GMalloc = &GHarnessMalloc;

void appMemset( void* Dest, INT C, INT Count ) { memset( Dest, C, Count ); }

static uint64_t gState = 0;
static uint64_t next64()
{
	uint64_t z = (gState += 0x9E3779B97F4A7C15ULL);
	z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9ULL;
	z = (z ^ (z >> 27)) * 0x94D049BB133111EBULL;
	return z ^ (z >> 31);
}

static void hexOut( const unsigned char* p, size_t n )
{
	for( size_t i = 0; i < n; i++ )
		printf( "%02x", p[i] );
}

int main()
{
	static const int bpp[6] = { 4, 4, 3, 2, 2, 2 };
	static const int nents[4] = { 1, 2, 16, 256 };
	static const int sizes[6][2] = { { 1, 1 }, { 2, 2 }, { 3, 5 }, { 7, 3 }, { 16, 16 }, { 33, 17 } };
	long long index = 0;
	int records = 0;
	for( int seed = 1; seed <= 2; seed++ )
	for( int fmt = 0; fmt < 6; fmt++ )
	for( int keyed = 0; keyed < 2; keyed++ )
	for( int ni = 0; ni < 4; ni++ )
	for( int si = 0; si < 6; si++ )
	for( int mips = 0; mips < 2; mips++ )
	{
		index++;
		gState = (uint64_t)seed * 1000003ULL + (uint64_t)index * 7919ULL;
		int w = sizes[si][0], h = sizes[si][1];
		int nent = nents[ni];
		int b = bpp[fmt];
		size_t srcBytes = (size_t)w * h * b;
		size_t dstBytes = mips ? ( (size_t)w * h * 22 / 16 + 512 ) : (size_t)w * h;
		DWORD keyval = (DWORD)next64();
		unsigned char* src = (unsigned char*)malloc( srcBytes );
		for( size_t i = 0; i < srcBytes; i++ )
			src[i] = (unsigned char)next64();
		if( keyed )
		{
			for( size_t t = 0; t < (size_t)w * h; t++ )
				if( next64() % 4 == 0 )
					for( int k = 0; k < b; k++ )
						src[t * b + k] = (unsigned char)( keyval >> (8 * k) );
		}
		unsigned char* dst = (unsigned char*)malloc( dstBytes );
		for( size_t i = 0; i < dstBytes; i++ )
			dst[i] = (unsigned char)next64();
		unsigned short pal[256];
		for( int i = 0; i < 256; i++ )
			pal[i] = 0xA5A5;

		// Record inputs before the call: the callee may free the source buffer.
		printf( "rec fmt=%d key=%d nent=%d w=%d h=%d mips=%d keyval=%08x src=", fmt, keyed, nent, w, h, mips, (unsigned)keyval );
		hexOut( src, srcBytes );
		printf( " dst_in=" );
		hexOut( dst, dstBytes );

		int ret = HeckbertQuantize( src, dst, pal, fmt, nent, w, h, keyed, keyval, mips );

		printf( " ret=%d pal=", ret );
		for( int i = 0; i < 256; i++ )
			printf( "%04x", pal[i] );
		printf( " dst=" );
		hexOut( dst, dstBytes );
		printf( "\n" );
		records++;
		free( dst );
	}
	fprintf( stderr, "records %d\n", records );
	return 0;
}

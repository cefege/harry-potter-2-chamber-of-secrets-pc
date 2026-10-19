// Fixture generator for the gesture port.
//
// Built by gen_gesture.sh against the ORIGINAL UnGesture.cpp extracted from the pinned ref, with
// product flags and -fno-access-control. The original TU is a separate object. This harness calls its
// external helpers directly, and drives its exec shells (UGesture::execCompareGesturePoint and
// execCompareGesture) through a minimal script frame: a bytecode buffer read by the real FFrame::Step.
//
// Output lines (gen script splits them into fixtures by prefix):
//   cmp2 <acc> <nm> (<xb> <yb>)*nm <nt> (..)*nt <ret>   InternalGestureCompare2
//   pt <px> <py> <nt> (<xb> <yb>)*nt <ret>              InternalPointCompare
//   xpt <acc> <nt> (<x> <y> <z>)*nt <px> <py> <pz> <ret>            UGesture::execCompareGesturePoint
//   xcg <acc> <nt> (<x> <y> <z>)*nt <nin> (<x> <y> <z>)*nin <ret>   UGesture::execCompareGesture
// Floats are printed as their 32-bit patterns in hex. Integers are decimal.

#include "EnginePrivate.h"

#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <vector>

// Same layout as the pinned UnGesture.cpp; the original's external helpers take it by value/array.
class CMousePoint
{
public:
	float	xPos;
	float	yPos;
};

// External-linkage helpers defined in the original UnGesture.cpp.
float InternalGestureCompare2( CMousePoint MouseArray[], int iNumMousePoints, CMousePoint nGesturePoints[], int iNumGesturePoints, float fAccuracy );
float InternalPointCompare( CMousePoint MousePoint, CMousePoint nGesturePoints[], int iNumGesturePoints, float fAccuracy );

// ---------------------------------------------------------------------------------------------
// Engine hooks. Same contracts as particle_fixture_harness.cpp.
// ---------------------------------------------------------------------------------------------

// libm wrapper with the body of the pinned Core/Src/UnAnsi.cpp.
DOUBLE appSqrt( DOUBLE Value ) { return sqrt(Value); }
void appMemcpy( void* Dest, const void* Src, INT Count ) { memcpy(Dest, Src, Count); }
void* appMemmove( void* Dest, const void* Src, INT Count ) { return memmove(Dest, Src, Count); }

class HarnessMalloc : public FMalloc
{
public:
	void* Malloc( DWORD Count, const TCHAR* Tag ) { (void)Tag; return malloc(Count); }
	void* Realloc( void* Original, DWORD Count, const TCHAR* Tag ) { (void)Tag; return realloc(Original, Count); }
	void Free( void* Original ) { free(Original); }
	void DumpAllocs() {}
	void HeapCheck() {}
	void Init() {}
	void Exit() {}
};
static HarnessMalloc g_harness_malloc;
FMalloc* GMalloc = &g_harness_malloc;

// Opcode dispatch table used by FFrame::Step. Only the three slots the exec bodies use are set.
Native GNatives[EX_Max];

void UObject::execFloatConst( FFrame& Stack, RESULT_DECL )
{
	*(FLOAT*)Result = Stack.ReadFloat();
}

void UObject::execVectorConst( FFrame& Stack, RESULT_DECL )
{
	((FVector*)Result)->X = Stack.ReadFloat();
	((FVector*)Result)->Y = Stack.ReadFloat();
	((FVector*)Result)->Z = Stack.ReadFloat();
}

// Harness-only reader for TArray<FVector> arguments. Slot EX_DynArrayCount carries a count-prefixed
// FVector array, so P_GET_STRUCT(TArray<FVector>, ...) receives one. The exec bodies never use the
// real opcode.
void UObject::execDynArrayCount( FFrame& Stack, RESULT_DECL )
{
	INT n = Stack.ReadInt();
	TArray<FVector>* Arr = (TArray<FVector>*)Result;
	Arr->Data = n > 0 ? (FVector*)appMalloc( n*sizeof(FVector), TEXT("") ) : NULL;
	Arr->ArrayNum = n;
	Arr->ArrayMax = n;
	for( INT k=0; k<n; k++ )
	{
		Arr->Data[k].X = Stack.ReadFloat();
		Arr->Data[k].Y = Stack.ReadFloat();
		Arr->Data[k].Z = Stack.ReadFloat();
	}
}

// ---------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------

static uint64_t g_rng = 0x9E3779B97F4A7C15ULL;
static uint32_t rnd32()
{
	g_rng = g_rng * 6364136223846793005ULL + 1442695040888963407ULL;
	return (uint32_t)(g_rng >> 33);
}
static float rndf()   // [0,1)
{
	return (float)(rnd32() & 0xFFFFFF) * (1.0f / 16777216.0f);
}

// Boundary pool: 0.33 and 0.66 (as floats and their neighbours), plus the extremes. Used for
// roughly a quarter of the coordinates so comparison edges are exercised.
static float pool_coord( )
{
	static const float pool[] = {
		0.0f, 0.33f, nextafterf(0.33f, 1.0f), nextafterf(0.33f, 0.0f),
		0.66f, nextafterf(0.66f, 1.0f), 0.5f, 1.0f,
	};
	if( (rnd32() & 3) == 0 )
		return pool[rnd32() % (sizeof(pool)/sizeof(pool[0]))];
	return rndf();
}

static void pf( float f )
{
	uint32_t bits;
	memcpy(&bits, &f, sizeof(bits));
	printf(" %08x", bits);
}

static void pi( int v )
{
	printf(" %d", v);
}

static void pmouse( const CMousePoint& p )
{
	pf(p.xPos);
	pf(p.yPos);
}

static void pvec( const FVector& v )
{
	pf(v.X);
	pf(v.Y);
	pf(v.Z);
}

// ---------------------------------------------------------------------------------------------
// Exec drivers. Each builds a bytecode buffer for the exec's P_GET_* sequence and calls the original.
// ---------------------------------------------------------------------------------------------

static void put_byte( std::vector<BYTE>& c, BYTE b ) { c.push_back(b); }
static void put_float( std::vector<BYTE>& c, float f )
{
	BYTE b[4];
	memcpy(b, &f, 4);
	c.insert(c.end(), b, b+4);
}
static void put_int( std::vector<BYTE>& c, INT v )
{
	BYTE b[4];
	memcpy(b, &v, 4);
	c.insert(c.end(), b, b+4);
}

// Replaces the gesture's Points array (the Rust mirror's TArray<FVector> at offset 72).
static void set_points( UGesture* g, const std::vector<FVector>& pts )
{
	if( g->Points.Data )
		appFree( g->Points.Data );
	INT n = (INT)pts.size();
	g->Points.Data = n > 0 ? (FVector*)appMalloc( n*sizeof(FVector), TEXT("") ) : NULL;
	for( INT k=0; k<n; k++ )
		g->Points.Data[k] = pts[k];
	g->Points.ArrayNum = n;
	g->Points.ArrayMax = n;
}

static float run_exec_point( UGesture* g, const FVector& p, float acc )
{
	std::vector<BYTE> code;
	put_byte(code, EX_VectorConst);
	put_float(code, p.X);
	put_float(code, p.Y);
	put_float(code, p.Z);
	put_byte(code, EX_FloatConst);
	put_float(code, acc);
	put_byte(code, EX_Nothing);   // P_FINISH skips one byte, then checks the next for EX_DebugInfo
	put_byte(code, EX_Nothing);

	FFrame Stack(g);
	Stack.Code = code.data();
	float Result = 0.0f;
	g->execCompareGesturePoint(Stack, &Result);
	return Result;
}

static float run_exec_compare( UGesture* g, const std::vector<FVector>& in, float acc )
{
	std::vector<BYTE> code;
	put_byte(code, EX_DynArrayCount);
	put_int(code, (INT)in.size());
	for( size_t k=0; k<in.size(); k++ )
	{
		put_float(code, in[k].X);
		put_float(code, in[k].Y);
		put_float(code, in[k].Z);
	}
	put_byte(code, EX_FloatConst);
	put_float(code, acc);
	put_byte(code, EX_Nothing);
	put_byte(code, EX_Nothing);

	FFrame Stack(g);
	Stack.Code = code.data();
	float Result = 0.0f;
	g->execCompareGesture(Stack, &Result);
	return Result;
}

// ---------------------------------------------------------------------------------------------
// Generators
// ---------------------------------------------------------------------------------------------

static void gen_cmp2()
{
	for( int c=0; c<400; c++ )
	{
		int nm = (int)(rnd32() % 31);
		int nt = (int)(rnd32() % 31);
		float acc = 0.02f + rndf() * 0.48f;
		std::vector<CMousePoint> m, t;
		for( int k=0; k<nm; k++ )
			m.push_back(CMousePoint{ pool_coord(), pool_coord() });
		if( nm > 0 && (rnd32() & 63) == 0 )
			m[rnd32() % nm].xPos = NAN;   // NaN path: never assigned to dMinVal
		for( int k=0; k<nt; k++ )
			t.push_back(CMousePoint{ pool_coord(), pool_coord() });
		std::vector<CMousePoint> mm = m, tt = t;
		if( mm.empty() ) mm.push_back(CMousePoint{0.0f, 0.0f});
		if( tt.empty() ) tt.push_back(CMousePoint{0.0f, 0.0f});
		float ret = InternalGestureCompare2(mm.data(), nm, tt.data(), nt, acc);
		printf("cmp2");
		pf(acc);
		pi(nm);
		for( int k=0; k<nm; k++ )
			pmouse(m[k]);
		pi(nt);
		for( int k=0; k<nt; k++ )
			pmouse(t[k]);
		pf(ret);
		printf("\n");
	}
}

static void gen_pt()
{
	for( int c=0; c<300; c++ )
	{
		int nt = (int)(rnd32() % 41);
		std::vector<CMousePoint> t;
		for( int k=0; k<nt; k++ )
			t.push_back(CMousePoint{ pool_coord(), pool_coord() });
		CMousePoint p{ pool_coord(), pool_coord() };
		std::vector<CMousePoint> tt = t;
		if( tt.empty() ) tt.push_back(CMousePoint{0.0f, 0.0f});
		float ret = InternalPointCompare(p, tt.data(), nt, 0.5f);
		printf("pt");
		pmouse(p);
		pi(nt);
		for( int k=0; k<nt; k++ )
			pmouse(t[k]);
		pf(ret);
		printf("\n");
	}
}

static void gen_xpt()
{
	UGesture* g = (UGesture*)calloc(1, sizeof(UGesture));
	for( int c=0; c<300; c++ )
	{
		// Target counts stay within 128: the original's expansion writes slot 8k+7 into a 1024-entry
		// array, so counts above 128 write out of bounds (UB) and are not fixture inputs.
		int nt = (c % 10 == 0) ? 128 : (int)(rnd32() % 21);
		if( c == 0 ) nt = 0;
		std::vector<FVector> t;
		for( int k=0; k<nt; k++ )
			t.push_back(FVector{ pool_coord(), pool_coord(), rndf() });
		float acc = 0.02f + rndf() * 0.48f;
		FVector p{ pool_coord(), pool_coord(), rndf() };
		set_points(g, t);
		float ret = run_exec_point(g, p, acc);
		printf("xpt");
		pf(acc);
		pi(nt);
		for( int k=0; k<nt; k++ )
			pvec(t[k]);
		pvec(p);
		pf(ret);
		printf("\n");
	}
}

static void gen_xcg()
{
	UGesture* g = (UGesture*)calloc(1, sizeof(UGesture));
	for( int c=0; c<300; c++ )
	{
		int nt = (c % 10 == 0) ? 128 : (int)(rnd32() % 21);
		if( c == 1 ) nt = 0;
		std::vector<FVector> t;
		for( int k=0; k<nt; k++ )
			t.push_back(FVector{ pool_coord(), pool_coord(), 0.0f });
		int nin = (int)(rnd32() % 61);
		std::vector<FVector> in;
		for( int k=0; k<nin; k++ )
		{
			uint32_t r = rnd32() % 16;
			if( r == 0 && !in.empty() )
			{
				// Exact duplicate of an earlier point: deduplicated.
				in.push_back(in[rnd32() % in.size()]);
			}
			else if( r == 1 && !in.empty() )
			{
				// Within the 0.001 dedupe tolerance of an earlier point.
				FVector b = in[rnd32() % in.size()];
				in.push_back(FVector{ b.X + 0.0005f, b.Y, b.Z });
			}
			else
			{
				float z = (rnd32() % 5 == 0) ? -1.0f : ((rnd32() & 1) ? 0.5f : 0.0f);
				in.push_back(FVector{ pool_coord(), pool_coord(), z });
			}
		}
		float acc = 0.02f + rndf() * 0.48f;
		set_points(g, t);
		float ret = run_exec_compare(g, in, acc);
		printf("xcg");
		pf(acc);
		pi(nt);
		for( int k=0; k<nt; k++ )
			pvec(t[k]);
		pi(nin);
		for( int k=0; k<nin; k++ )
			pvec(in[k]);
		pf(ret);
		printf("\n");
	}
	// Clamp path: 1100 distinct input points (nNumMousePoints is capped at MAX_MOUSE_POINTS=1024).
	{
		std::vector<FVector> t;
		for( int k=0; k<40; k++ )
			t.push_back(FVector{ pool_coord(), pool_coord(), 0.0f });
		std::vector<FVector> in;
		for( int k=0; k<1100; k++ )
			in.push_back(FVector{ (float)k / 1100.0f, rndf(), 0.0f });
		float acc = 0.1f;
		set_points(g, t);
		float ret = run_exec_compare(g, in, acc);
		printf("xcg");
		pf(acc);
		pi(40);
		for( int k=0; k<40; k++ )
			pvec(t[k]);
		pi(1100);
		for( int k=0; k<1100; k++ )
			pvec(in[k]);
		pf(ret);
		printf("\n");
	}
}

int main()
{
	GNatives[EX_FloatConst] = &UObject::execFloatConst;
	GNatives[EX_VectorConst] = &UObject::execVectorConst;
	GNatives[EX_DynArrayCount] = &UObject::execDynArrayCount;

	gen_cmp2();
	gen_pt();
	gen_xpt();
	gen_xcg();
	return 0;
}

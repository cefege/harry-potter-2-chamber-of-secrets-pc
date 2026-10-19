#define TEXT(x) x
using TCHAR = char;
void appErrorf(const TCHAR*);

#include "../../HarryPotter2/Unreal/Core/Inc/Hp2FfiException.h"

#include <cstring>
#include <stdexcept>

void appFailAssert(const char* Expr, const char* File, int Line)
{
	if( std::strcmp( Expr, "NetDriver" ) != 0 || std::strcmp( File, "UnLevel.cpp" ) != 0 || Line != 1328 )
		throw std::runtime_error("unexpected assertion arguments");
	throw std::runtime_error("expected check failure");
}

static int NetCheckFailed( const char* Expr, const char* File, int Line )
{
	return Hp2FfiCall( [&]{ appFailAssert( Expr, File, Line ); } );
}
static int NetAppErrorf( const TCHAR* Text )
{
	return Hp2FfiCall( [&]{ appErrorf( Text ); } );
}

void appErrorf(const TCHAR* Text)
{
	if( std::strcmp( Text, "Level received unexpected file" ) != 0 )
		throw std::runtime_error("unexpected appErrorf arguments");
	throw std::runtime_error("expected appErrorf");
}

static bool RethrowsAs( int Status, const char* Expected )
{
	try
	{
		Hp2RethrowPending( Status );
	}
	catch( const std::runtime_error& Error )
	{
		return std::strcmp( Error.what(), Expected ) == 0;
	}
	return false;
}

int main()
{
	if( !RethrowsAs( NetCheckFailed( "NetDriver", "UnLevel.cpp", 1328 ), "expected check failure" ) )
		return 1;
	if( !RethrowsAs( NetAppErrorf( "Level received unexpected file" ), "expected appErrorf" ) )
		return 2;
	return 0;
}

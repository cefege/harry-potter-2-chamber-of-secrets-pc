// C callbacks behind the Rust UnParams port (crates/hp2-engine/src/params.rs).
// Each thunk forwards to the real Core function, so the Rust side never
// re-implements Core string or parse semantics.

#include "EnginePrivate.h"
#include "Hp2ParamsServices.h"

static int Hp2ParamsParseFloat( const void* Stream, const void* Match, float* Value )
{
	return Parse( static_cast<const TCHAR*>(Stream), static_cast<const TCHAR*>(Match), *Value );
}

static float Hp2ParamsAtof( const void* Str )
{
	return appAtof( static_cast<const TCHAR*>(Str) );
}

static void* Hp2ParamsStrchr( const void* String, int C )
{
	return appStrchr( static_cast<const TCHAR*>(String), C );
}

static const void* Hp2ParamsStrfind( const void* Str, const void* Find )
{
	return appStrfind( static_cast<const TCHAR*>(Str), static_cast<const TCHAR*>(Find) );
}

static int Hp2ParamsStrlen( const void* String )
{
	return appStrlen( static_cast<const TCHAR*>(String) );
}

static void Hp2ParamsStrncpy( void* Dest, const void* Src, int MaxLen )
{
	appStrncpy( static_cast<TCHAR*>(Dest), static_cast<const TCHAR*>(Src), MaxLen );
}

const Hp2ParamsServices GHp2ParamsServices =
{
	Hp2ParamsParseFloat,
	Hp2ParamsAtof,
	Hp2ParamsStrchr,
	Hp2ParamsStrfind,
	Hp2ParamsStrlen,
	Hp2ParamsStrncpy,
};

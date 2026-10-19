#ifndef _INC_HP2PARAMSSERVICES_H
#define _INC_HP2PARAMSSERVICES_H

// C callbacks the Rust UnParams port (crates/hp2-engine/src/params.rs) calls.
// Field order must match `ParamsServices` on the Rust side.
// Pointers are TCHAR strings; they are typed `void` here so this header stays
// free of Core includes.
struct Hp2ParamsServices
{
	// UBOOL Parse(const TCHAR* Stream, const TCHAR* Match, FLOAT& Value)
	int   (*parse_float)( const void* Stream, const void* Match, float* Value );
	// FLOAT appAtof(const TCHAR* Str)
	float (*app_atof)( const void* Str );
	// TCHAR* appStrchr(const TCHAR* String, INT c)
	void* (*app_strchr)( const void* String, int C );
	// const TCHAR* appStrfind(const TCHAR* Str, const TCHAR* Find)
	const void* (*app_strfind)( const void* Str, const void* Find );
	// INT appStrlen(const TCHAR* String)
	int   (*app_strlen)( const void* String );
	// TCHAR* appStrncpy(TCHAR* Dest, const TCHAR* Src, INT MaxLen)
	void  (*app_strncpy)( void* Dest, const void* Src, int MaxLen );
};

extern const Hp2ParamsServices GHp2ParamsServices;

#endif // _INC_HP2PARAMSSERVICES_H

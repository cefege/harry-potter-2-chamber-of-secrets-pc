/*=============================================================================
	UnCoreNative.h: Native function lookup table for static libraries.
	Copyright 2000 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Brandon Reinhart
=============================================================================*/

#ifndef UNCORENATIVE_H
#define UNCORENATIVE_H

#define NATIVE_NAME_TOKEN_INNER(cls,name) int##cls##name
#define NATIVE_NAME_TOKEN(cls,name) NATIVE_NAME_TOKEN_INNER(cls,name)
#define NATIVE_NAME_STRING_INNER(value) #value
#define NATIVE_NAME_STRING(value) NATIVE_NAME_STRING_INNER(value)
#define NATIVE_NAME_TEXT_INNER(value) TEXT(value)
#define NATIVE_NAME_TEXT(value) NATIVE_NAME_TEXT_INNER(value)
#define NATIVE_NAME(cls,name) NATIVE_NAME_TEXT(NATIVE_NAME_STRING(NATIVE_NAME_TOKEN(cls,name)))

#define MAP_NATIVE(cls,name) \
	{NATIVE_NAME(cls,name), &cls::name},

#define DECLARE_NATIVE_TYPE(pkg,type) \
	typedef void (type::*type##Native)( FFrame& TheStack, RESULT_DECL ); \
	struct type##NativeInfo \
	{ const TCHAR* Name; type##Native Pointer; }; \
	extern void* Find##pkg##type##Native( const TCHAR* NativeName );

#define IMPLEMENT_NATIVE_HANDLER(pkg,type) \
	void* Find##pkg##type##Native( const TCHAR* NativeName ) \
	{ INT i=0; while (G##pkg##type##Natives[i].Name) \
		{ if (appStrcmp(NativeName, G##pkg##type##Natives[i].Name) == 0) \
			return &G##pkg##type##Natives[i].Pointer; \
		i++; } return NULL; }

typedef void* (*NativeLookup)(const TCHAR*);
extern NativeLookup GNativeLookupFuncs[32];

static void* FindNative( const TCHAR* NativeName )
{
	for (INT i=0; i<ARRAY_COUNT(GNativeLookupFuncs); i++)
		if (GNativeLookupFuncs[i])
		{
			void* Result = GNativeLookupFuncs[i]( NativeName );
			if (Result)
				return Result;
		}
	return NULL;
}

DECLARE_NATIVE_TYPE(Core,UObject);
DECLARE_NATIVE_TYPE(Core,UCommandlet);

#endif

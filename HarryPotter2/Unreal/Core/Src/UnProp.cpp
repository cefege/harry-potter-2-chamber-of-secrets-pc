/*=============================================================================
	UnClsPrp.cpp: FProperty implementation
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "CorePrivate.h"

//!!fix hardcoded lengths
/*-----------------------------------------------------------------------------
	Helpers.
-----------------------------------------------------------------------------*/

//
// Parse a hex digit.
//
static INT HexDigit( TCHAR c )
{
	if( c>='0' && c<='9' )
		return c - '0';
	else if( c>='a' && c<='f' )
		return c + 10 - 'a';
	else if( c>='A' && c<='F' )
		return c + 10 - 'A';
	else
		return 0;
}

//
// Parse a token.
//
const TCHAR* ReadToken( const TCHAR* Buffer, TCHAR* String, INT MaxLength, UBOOL DottedNames=0 )
{
	guard(ReadToken);
	INT Count=0;
	if( *Buffer == 0x22 )
	{
		// Get quoted string.
		Buffer++;
		while( *Buffer && *Buffer!=0x22 && *Buffer!=13 && *Buffer!=10 && Count<MaxLength-1 )
		{
			if( *Buffer != '\\' )
			{
				String[Count++] = *Buffer++;
			}
			else if( *++Buffer=='\\' )
			{
				String[Count++] = '\\';
				Buffer++;
			}
			else
			{
				String[Count++] = HexDigit(Buffer[0])*16 + HexDigit(Buffer[1]);
				Buffer += 2;
			}
		}
		if( Count==MaxLength-1 )
		{
			debugf( NAME_Warning, TEXT("ReadToken: Quoted string too long") );
			return NULL;
		}
		if( *Buffer++!=0x22 )
		{
			GWarn->Logf( NAME_Warning, TEXT("ReadToken: Bad quoted string") );
			return NULL;
		}
	}
	else if( appIsAlnum( *Buffer ) )
	{
		// Get identifier.
		while
		(	(appIsAlnum(*Buffer) || *Buffer=='_' || *Buffer=='-' || (DottedNames && *Buffer=='.' ))
		&&	Count<MaxLength-1 )
			String[Count++] = *Buffer++;
		if( Count==MaxLength-1 )
		{
			debugf( NAME_Warning, TEXT("ReadToken: Alphanumeric overflow") );
			return NULL;
		}
	}
	else
	{
		// Get just one.
		String[Count++] = *Buffer;
	}
	String[Count] = 0;
	return Buffer;
	unguard;
}

/*-----------------------------------------------------------------------------
	UProperty implementation.
-----------------------------------------------------------------------------*/
struct FNativePropertyOffset
{
	UField* Owner;
	INT NameIndex;
	INT Offset;
};
static FNativePropertyOffset GNativePropertyOffsets[16384];
static INT GNativePropertyOffsetCount = 0;

// Root package identity for cross-instance native-layout matching: a data
// root may re-export a natively-registered class through its own script shell
// (e.g. an older Engine.u exporting Actor); the freshly deserialized UClass is
// a distinct object, but it represents the same engine class.
static FName NativeLayoutRootPackageName( UObject* Obj )
{
	UObject* Top = Obj;
	while( Top->GetOuter() )
		Top = Top->GetOuter();
	return Top->GetFName();
}

static UProperty* NativeLayoutFindChildProperty( UStruct* Strct, FName PropName )
{
	for( UField* Field=Strct->Children; Field; Field=Field->Next )
	{
		UProperty* Property = Cast<UProperty>( Field );
		if( Property && Property->GetFName()==PropName )
			return Property;
	}
	return NULL;
}
//
// Constructors.
//
UProperty::UProperty()
:	UField( NULL )
,	ArrayDim( 1 )
,	Offset( 0 )
,	CppProperty( 0 )
{}
UProperty::UProperty( ECppProperty, INT InOffset, const TCHAR* InCategory, DWORD InFlags )
:	UField( NULL )
,	ArrayDim( 1 )
,	PropertyFlags( InFlags )
,	Category( InCategory )
,	Offset( InOffset )
,	CppProperty( 1 )
{
	GetOuterUField()->AddCppProperty( this );
	check(GNativePropertyOffsetCount < ARRAY_COUNT(GNativePropertyOffsets));
	FNativePropertyOffset& NativeOffset = GNativePropertyOffsets[GNativePropertyOffsetCount++];
	NativeOffset.Owner = GetOuterUField();
	NativeOffset.NameIndex = GetFName().GetIndex();
	NativeOffset.Offset = InOffset;
}

//
// Serializer.
//
void UProperty::Serialize( FArchive& Ar )
{
	guard(UProperty::Serialize);
	const INT SavedNativeOffset = Offset;
	const UBOOL WasCppProperty = CppProperty;
	Super::Serialize( Ar );

	// Archive the basic info.
	Ar << ArrayDim << PropertyFlags << Category;
	INT RegisteredNativeOffset = 0;
	UBOOL HasRegisteredNativeOffset = 0;
	if( Ar.IsLoading() )
	{
		UField*  OwnerField    = GetOuterUField();
		const INT PropNameIndex = GetFName().GetIndex();
		for( INT Index=0; Index<GNativePropertyOffsetCount; ++Index )
		{
			const FNativePropertyOffset& NativeOffset = GNativePropertyOffsets[Index];
			if( NativeOffset.NameIndex!=PropNameIndex )
				continue;
			if( NativeOffset.Owner==OwnerField )
			{
				// Exact instance match: the registered native class itself.
				RegisteredNativeOffset = NativeOffset.Offset;
				HasRegisteredNativeOffset = 1;
				break;
			}
			// Cross-instance reconciliation: same class name in the same root
			// package. Adopt the registered offset only when the serialized
			// property's type and dimensions agree with the live native one;
			// anything else stays sequential and reports loudly.
			UStruct* LoadedOwner = Cast<UStruct>( OwnerField );
			UStruct* NativeOwner = Cast<UStruct>( NativeOffset.Owner );
			if( !LoadedOwner || !NativeOwner
			||  LoadedOwner->GetFName()!=NativeOwner->GetFName()
			||  NativeLayoutRootPackageName(LoadedOwner)!=NativeLayoutRootPackageName(NativeOwner) )
				continue;
			UProperty* NativeProp = NativeLayoutFindChildProperty( NativeOwner, GetFName() );
			if( !NativeProp
			||  NativeProp->GetClass()!=GetClass()
			||  NativeProp->ElementSize!=ElementSize
			||  NativeProp->ArrayDim!=ArrayDim )
			{
				debugf( NAME_Error, TEXT("UProperty::Serialize: %s.%s native layout mismatch; using sequential offset"), NativeOwner->GetName(), *GetFName() );
				continue;
			}
			RegisteredNativeOffset = NativeProp->Offset;
			HasRegisteredNativeOffset = 1;
			break;
		}
	}
	if( PropertyFlags & CPF_Net )
		Ar << RepOffset;
	if( Ar.Ver() <= 61 )//oldver: clear old net flags.
		PropertyFlags &= ~0x00080040;
	if( Ar.IsLoading() )
	{
		Offset = HasRegisteredNativeOffset ? RegisteredNativeOffset : (WasCppProperty ? SavedNativeOffset : 0);
		CppProperty = HasRegisteredNativeOffset || WasCppProperty;
		ConstructorLinkNext = NULL;
	}
	unguardobj;
}

//
// Export this class property to an output
// device as a C++ header file.
//
void UProperty::ExportCpp( FOutputDevice& Out, UBOOL IsLocal, UBOOL IsParm ) const
{
	guard(UProperty::ExportCpp)
	TCHAR ArrayStr[80] = TEXT("");
	if
	(	IsParm
	&&	IsA(UStrProperty::StaticClass())
	&&	!(PropertyFlags & CPF_OutParm) )
		Out.Log( TEXT("const ") );
	ExportCppItem( Out );
	if( ArrayDim != 1 )
		appSprintf( ArrayStr, TEXT("[%i]"), ArrayDim );
	if( IsA(UBoolProperty::StaticClass()) )
	{
		if( ArrayDim==1 && !IsLocal && !IsParm )
			Out.Logf( TEXT(" %s%s:1"), GetName(), ArrayStr );
		else if( IsParm && (PropertyFlags & CPF_OutParm) )
			Out.Logf( TEXT("& %s%s"), GetName(), ArrayStr );
		else
			Out.Logf( TEXT(" %s%s"), GetName(), ArrayStr );
	}
	else if( IsA(UStrProperty::StaticClass()) )
	{
		if( IsParm && ArrayDim>1 )
			Out.Logf( TEXT("* %s"), GetName() );
		else if( IsParm )
			Out.Logf( TEXT("& %s"), GetName() );
		else if( IsLocal )
			Out.Logf( TEXT(" %s"), GetName() );
		else
			Out.Logf( TEXT("NoInit %s%s"), GetName(), ArrayStr );
	}
	else
	{
		if( IsParm && ArrayDim>1 )
			Out.Logf( TEXT("* %s"), GetName() );
		else if( IsParm && (PropertyFlags & CPF_OutParm) )
			Out.Logf( TEXT("& %s%s"), GetName(), ArrayStr );
		else
			Out.Logf( TEXT(" %s%s"), GetName(), ArrayStr );
	}
	unguardobj;
}

//
// Export the contents of a property.
//
UBOOL UProperty::ExportText
(
	INT		Index,
	TCHAR*	ValueStr,
	BYTE*	Data,
	BYTE*	Delta,
	INT		PortFlags
) const
{
	guard(UProperty::ExportText);
	ValueStr[0]=0;
	if( Data==Delta || !Matches(Data,Delta,Index) )
	{
		ExportTextItem
		(
			ValueStr,
			Data + Offset + Index * ElementSize,
			Delta ? (Delta + Offset + Index * ElementSize) : NULL,
			PortFlags
		);
		return 1;
	}
	else return 0;
	unguardobj;
}

//
// Copy a unique instance of a value.
//
void UProperty::CopySingleValue( void* Dest, void* Src ) const
{
	guardSlow(UProperty::CopySingleValue);
	appMemcpy( Dest, Src, ElementSize );
	unguardobjSlow;
}

//
// Destroy a value.
//
void UProperty::DestroyValue( void* Dest ) const
{}

//
// Net serialization.
//
UBOOL UProperty::NetSerializeItem( FArchive& Ar, UPackageMap* Map, void* Data ) const
{
	guardSlow(UProperty::NetSerializeItem);
	SerializeItem( Ar, Data );
	return 1;
	unguardobjSlow;
}

//
// Return whether the property should be exported.
//
UBOOL UProperty::Port() const
{
	return 
	(	GetSize()
	&&	(Category!=NAME_None || !(PropertyFlags & (CPF_Transient | CPF_Native)))
	&&	GetFName()!=NAME_Class );
}

//
// Return type id for encoding properties in .u files.
//
BYTE UProperty::GetID() const
{
	return GetClass()->GetFName().GetIndex();
}

// Return the storage alignment for this rebuilt host property.
INT UProperty::GetAlignment()
{
	if( CppProperty || (PropertyFlags & CPF_Native) )
		return PROPERTY_ALIGNMENT;
	if( IsA(UObjectProperty::StaticClass()) )
		return Min<INT>(appCheckedIntSize(alignof(UObject*)), PROPERTY_ALIGNMENT);
	if( IsA(UNameProperty::StaticClass()) )
		return Min<INT>(appCheckedIntSize(alignof(FName)), PROPERTY_ALIGNMENT);
	if( IsA(UStrProperty::StaticClass()) )
		return Min<INT>(appCheckedIntSize(alignof(FString)), PROPERTY_ALIGNMENT);
	if( IsA(UArrayProperty::StaticClass()) )
		return Min<INT>(appCheckedIntSize(alignof(FArray)), PROPERTY_ALIGNMENT);
	if( IsA(UMapProperty::StaticClass()) )
		return Min<INT>(appCheckedIntSize(alignof(TMap<BYTE,BYTE>)), PROPERTY_ALIGNMENT);
	if( UFixedArrayProperty* FixedArray = Cast<UFixedArrayProperty>(this) )
		return FixedArray->Inner->GetAlignment();
	if( UStructProperty* StructProperty = Cast<UStructProperty>(this) )
		return StructProperty->Struct->PropertiesAlign;
	return ElementSize==2 ? 2 : ElementSize>=4 ? PROPERTY_ALIGNMENT : 1;
}

//
// Copy a complete value.
//
void UProperty::CopyCompleteValue( void* Dest, void* Src ) const
{
	guardSlow(UProperty::CopyCompleteValue);
	for( INT i=0; i<ArrayDim; i++ )
		CopySingleValue( (BYTE*)Dest+i*ElementSize, (BYTE*)Src+i*ElementSize );
	unguardobjSlow;
}

//
// Link property loaded from file.
//
void UProperty::Link( FArchive& Ar, UProperty* Prev )
{}

IMPLEMENT_CLASS(UProperty);

/*-----------------------------------------------------------------------------
	UByteProperty.
-----------------------------------------------------------------------------*/

void UByteProperty::Link( FArchive& Ar, UProperty* Prev )
{
	guard(UByteProperty::Link);
	Super::Link( Ar, Prev );
	ElementSize = appCheckedIntSize(sizeof(BYTE));
	Offset      = Align( GetOuterUField()->GetPropertiesSize(), ElementSize );
	unguardobj;
}
void UByteProperty::CopySingleValue( void* Dest, void* Src ) const
{
	guardSlow(UByteProperty::CopySingleValue);
	*(BYTE*)Dest = *(BYTE*)Src;
	unguardSlow;
}
void UByteProperty::CopyCompleteValue( void* Dest, void* Src ) const
{
	guardSlow(UByteProperty::CopyCompleteValue);
	if( ArrayDim==1 )
		*(BYTE*)Dest = *(BYTE*)Src;
	else
		appMemcpy( Dest, Src, ArrayDim );
	unguardSlow;
}
UBOOL UByteProperty::Identical( const void* A, const void* B ) const
{
	guardSlow(UByteProperty::Identical);
	return *(BYTE*)A == (B ? *(BYTE*)B : 0);
	unguardobjSlow;
}
void UByteProperty::SerializeItem( FArchive& Ar, void* Value ) const
{
	guardSlow(UByteProperty::SerializeItem);
	Ar << *(BYTE*)Value;
	unguardobjSlow;
}
UBOOL UByteProperty::NetSerializeItem( FArchive& Ar, UPackageMap* Map, void* Data ) const
{
	guardSlow(UByteProperty::NetSerializeItem);
	Ar.SerializeBits( Data, Enum ? appCeilLogTwo(Enum->Names.Num()) : 8 );
	return 1;
	unguardobjSlow;
}
void UByteProperty::Serialize( FArchive& Ar )
{
	guard(UByteProperty::Serialize);
	Super::Serialize( Ar );
	Ar << Enum;
	unguardobj;
}
void UByteProperty::ExportCppItem( FOutputDevice& Out ) const
{
	guard(UByteProperty::ExportCppItem);
	Out.Log( TEXT("BYTE") );
	unguardobj;
}
void UByteProperty::ExportTextItem( TCHAR* ValueStr, BYTE* PropertyValue, BYTE* DefaultValue, INT PortFlags ) const
{
	guard(UByteProperty::ExportTextItem);
	if( Enum )
		appSprintf( ValueStr, TEXT("%s"), *Enum->Names(*(BYTE*)PropertyValue) );
	else
		appSprintf( ValueStr, TEXT("%i"), *(BYTE*)PropertyValue );
	unguardobj;
}
const TCHAR* UByteProperty::ImportText( const TCHAR* Buffer, BYTE* Data, INT PortFlags ) const
{
	guard(UByteProperty::ImportText);
	TCHAR Temp[1024];
	if( Enum )
	{
		Buffer = ReadToken( Buffer, Temp, ARRAY_COUNT(Temp) );
		if( !Buffer )
			return NULL;
		FName EnumName = FName( Temp, FNAME_Find );
		if( EnumName != NAME_None )
		{
			INT EnumIndex=0;
			if( Enum->Names.FindItem( EnumName, EnumIndex ) )
			{
				*(BYTE*)Data = EnumIndex;
				return Buffer;
			}
		}
	}
	if( appIsDigit(*Buffer) )
	{
		*(BYTE*)Data = appAtoi( Buffer );
		while( *Buffer>='0' && *Buffer<='9' )
			Buffer++;
	}
	else
	{
		//debugf( "Import: Missing byte" );
		return NULL;
	}
	return Buffer;
	unguardobj;
}
IMPLEMENT_CLASS(UByteProperty);

/*-----------------------------------------------------------------------------
	UIntProperty.
-----------------------------------------------------------------------------*/

void UIntProperty::Link( FArchive& Ar, UProperty* Prev )
{
	guard(UIntProperty::Link);
	Super::Link( Ar, Prev );
	ElementSize = appCheckedIntSize(sizeof(INT));
	Offset      = Align( GetOuterUField()->GetPropertiesSize(), ElementSize );
	unguardobj;
}
void UIntProperty::CopySingleValue( void* Dest, void* Src ) const
{
	guardSlow(UIntProperty::CopySingleValue);
	*(INT*)Dest = *(INT*)Src;
	unguardSlow;
}
void UIntProperty::CopyCompleteValue( void* Dest, void* Src ) const
{
	guardSlow(UIntProperty::CopyCompleteValue);
	if( ArrayDim==1 )
		*(INT*)Dest = *(INT*)Src;
	else
		for( INT i=0; i<ArrayDim; i++ )
			((INT*)Dest)[i] = ((INT*)Src)[i];
	unguardSlow;
}
UBOOL UIntProperty::Identical( const void* A, const void* B ) const
{
	guardSlow(UIntProperty::Identical);
	return *(INT*)A == (B ? *(INT*)B : 0);
	unguardobjSlow;
}
void UIntProperty::SerializeItem( FArchive& Ar, void* Value ) const
{
	guardSlow(UIntProperty::SerializeItem);
	Ar << *(INT*)Value;
	unguardobjSlow;
}
UBOOL UIntProperty::NetSerializeItem( FArchive& Ar, UPackageMap* Map, void* Data ) const
{
	guardSlow(UIntProperty::NetSerializeItem);
	Ar << *(INT*)Data;
	return 1;
	unguardSlow;
}
void UIntProperty::ExportCppItem( FOutputDevice& Out ) const
{
	guard(UIntProperty::ExportCppItem);
	Out.Log( TEXT("INT") );
	unguardobj;
}
void UIntProperty::ExportTextItem( TCHAR* ValueStr, BYTE* PropertyValue, BYTE* DefaultValue, INT PortFlags ) const
{
	guard(UIntProperty::ExportTextItem);
	appSprintf( ValueStr, TEXT("%i"), *(INT *)PropertyValue );
	unguardobj;
}
const TCHAR* UIntProperty::ImportText( const TCHAR* Buffer, BYTE* Data, INT PortFlags ) const
{
	guard(UIntProperty::ImportText);
	if( *Buffer=='-' || (*Buffer>='0' && *Buffer<='9') )
		*(INT*)Data = appAtoi( Buffer );
	while( *Buffer=='-' || (*Buffer>='0' && *Buffer<='9') )
		Buffer++;
	return Buffer;
	unguardobj;
}
IMPLEMENT_CLASS(UIntProperty);

/*-----------------------------------------------------------------------------
	UBoolProperty.
-----------------------------------------------------------------------------*/

void UBoolProperty::Link( FArchive& Ar, UProperty* Prev )
{
	guard(UBoolProperty::Link);
	const INT NativeOffset = Offset;
	const DWORD NativeBitMask = BitMask;
	Super::Link( Ar, Prev );
	if( CppProperty )
	{
		Offset = NativeOffset;
		BitMask = NativeBitMask;
	}
	else
	{
		UBoolProperty* PrevBool = Cast<UBoolProperty>( Prev );
		if( GetOuterUField()->MergeBools() && PrevBool && NEXT_BITFIELD(PrevBool->BitMask) )
		{
			Offset  = Prev->Offset;
			BitMask = NEXT_BITFIELD(PrevBool->BitMask);
		}
		else
		{
			Offset  = Align(GetOuterUField()->GetPropertiesSize(),appCheckedIntSize(sizeof(BITFIELD)));
			BitMask = FIRST_BITFIELD;
		}
	}
	ElementSize = appCheckedIntSize(sizeof(BITFIELD));
	unguardobj;
}
void UBoolProperty::Serialize( FArchive& Ar )
{
	guard(UBoolProperty::Serialize);
	Super::Serialize( Ar );
	if( !Ar.IsLoading() && !Ar.IsSaving() )
		Ar << BitMask;
	unguardobj;
}
void UBoolProperty::ExportCppItem( FOutputDevice& Out ) const
{
	guard(UBoolProperty::ExportCppItem);
	Out.Log( TEXT("BITFIELD") );
	unguardobj;
}
void UBoolProperty::ExportTextItem( TCHAR* ValueStr, BYTE* PropertyValue, BYTE* DefaultValue, INT PortFlags ) const
{
	guard(UBoolProperty::ExportTextItem);
	TCHAR* Temp
	=	(TCHAR*) ((PortFlags & PPF_Localized)
	?	(((*(BITFIELD*)PropertyValue) & BitMask) ? GTrue  : GFalse )
	:	(((*(BITFIELD*)PropertyValue) & BitMask) ? TEXT("True") : TEXT("False")));
	appSprintf( ValueStr, TEXT("%s"), Temp );
	unguardobj;
}
const TCHAR* UBoolProperty::ImportText( const TCHAR* Buffer, BYTE* Data, INT PortFlags ) const
{
	guard(UBoolProperty::ImportText);
	TCHAR Temp[1024];
	Buffer = ReadToken( Buffer, Temp, ARRAY_COUNT(Temp) );
	if( !Buffer )
		return NULL;
	if( appStricmp(Temp,TEXT("1"))==0 || appStricmp(Temp,TEXT("True"))==0 || appStricmp(Temp,GTrue)==0 )
	{
		*(BITFIELD*)Data |= BitMask;
	}
	else if( appStricmp(Temp,TEXT("0"))==0 || appStricmp(Temp,TEXT("False"))==0  || appStricmp(Temp,GFalse)==0 )
	{
		*(BITFIELD*)Data &= ~BitMask;
	}
	else
	{
		//debugf( "Import: Failed to get bool" );
		return NULL;
	}
	return Buffer;
	unguardobj;
}
UBOOL UBoolProperty::Identical( const void* A, const void* B ) const
{
	guardSlow(UBoolProperty::Identical);
	return ((*(BITFIELD*)A ^ (B ? *(BITFIELD*)B : 0)) & BitMask) == 0;
	unguardobjSlow;
}
void UBoolProperty::SerializeItem( FArchive& Ar, void* Value ) const
{
	guardSlow(UBoolProperty::SerializeItem);
	BYTE B = (*(BITFIELD*)Value & BitMask) ? 1 : 0;
	Ar << B;
	if( B ) *(BITFIELD*)Value |=  BitMask;
	else    *(BITFIELD*)Value &= ~BitMask;
	unguardobjSlow;
}
UBOOL UBoolProperty::NetSerializeItem( FArchive& Ar, UPackageMap* Map, void* Data ) const
{
	guardSlow(UByteProperty::NetSerializeItem);
	BYTE Value = ((*(BITFIELD*)Data & BitMask)!=0);
	Ar.SerializeBits( &Value, 1 );
	if( Value )
		*(BITFIELD*)Data |= BitMask;
	else
		*(BITFIELD*)Data &= ~BitMask;
	return 1;
	unguardobjSlow;
}
void UBoolProperty::CopySingleValue( void* Dest, void* Src ) const
{
	guardSlow(UProperty::CopySingleValue);
	*(BITFIELD*)Dest = (*(BITFIELD*)Dest & ~BitMask) | (*(BITFIELD*)Src & BitMask);
	unguardobjSlow;
}
IMPLEMENT_CLASS(UBoolProperty);

/*-----------------------------------------------------------------------------
	UFloatProperty.
-----------------------------------------------------------------------------*/

void UFloatProperty::Link( FArchive& Ar, UProperty* Prev )
{
	guard(UFloatProperty::Link);
	Super::Link( Ar, Prev );
	ElementSize = appCheckedIntSize(sizeof(FLOAT));
	Offset      = Align( GetOuterUField()->GetPropertiesSize(), ElementSize );
	unguardobj;
}
void UFloatProperty::CopySingleValue( void* Dest, void* Src ) const
{
	guardSlow(UFloatProperty::CopySingleValue);
	*(FLOAT*)Dest = *(FLOAT*)Src;
	unguardSlow;
}
void UFloatProperty::CopyCompleteValue( void* Dest, void* Src ) const
{
	guardSlow(UFloatProperty::CopyCompleteValue);
	if( ArrayDim==1 )
		*(FLOAT*)Dest = *(FLOAT*)Src;
	else
		for( INT i=0; i<ArrayDim; i++ )
			((FLOAT*)Dest)[i] = ((FLOAT*)Src)[i];
	unguardSlow;
}
UBOOL UFloatProperty::Identical( const void* A, const void* B ) const
{
	guardSlow(UFloatProperty::Identical);
	return *(FLOAT*)A == (B ? *(FLOAT*)B : 0);
	unguardobjSlow;
}
void UFloatProperty::SerializeItem( FArchive& Ar, void* Value ) const
{
	guardSlow(UFloatProperty::SerializeItem);
	Ar << *(FLOAT*)Value;
	unguardobjSlow;
}
UBOOL UFloatProperty::NetSerializeItem( FArchive& Ar, UPackageMap* Map, void* Data ) const
{
	guardSlow(UFloatProperty::NetSerializeItem);
	Ar << *(FLOAT*)Data;
	return 1;
	unguardSlow;
}
void UFloatProperty::ExportCppItem( FOutputDevice& Out ) const
{
	guard(UFloatProperty::ExportCppItem);
	Out.Log( TEXT("FLOAT") );
	unguardobj;
}
void UFloatProperty::ExportTextItem( TCHAR* ValueStr, BYTE* PropertyValue, BYTE* DefaultValue, INT PortFlags ) const
{
	guard(UFloatProperty::ExportTextItem);
	appSprintf( ValueStr, TEXT("%.7g"), *(FLOAT*)PropertyValue );
	unguardobj;
}
const TCHAR* UFloatProperty::ImportText( const TCHAR* Buffer, BYTE* Data, INT PortFlags ) const
{
	guard(UFloatProperty::ImportText);
	*(FLOAT*)Data = appAtof(Buffer);
	while( *Buffer && *Buffer!=',' && *Buffer!=')' && *Buffer!=13 && *Buffer!=10 )
		Buffer++;
	return Buffer;
	unguardobj;
}
IMPLEMENT_CLASS(UFloatProperty);

/*-----------------------------------------------------------------------------
	UObjectProperty.
-----------------------------------------------------------------------------*/

void UObjectProperty::Link( FArchive& Ar, UProperty* Prev )
{
	guard(UObjectProperty::Link);
	Super::Link( Ar, Prev );
	ElementSize = appCheckedIntSize(sizeof(UObject*));
	Offset      = Align( GetOuterUField()->GetPropertiesSize(), GetAlignment() );
	unguardobj;
}
void UObjectProperty::CopySingleValue( void* Dest, void* Src ) const
{
	guardSlow(UObjectProperty::CopySingleValue);
	*(UObject**)Dest = *(UObject**)Src;
	unguardSlow;
}
void UObjectProperty::CopyCompleteValue( void* Dest, void* Src ) const
{
	guardSlow(UObjectProperty::CopyCompleteValue);
	if( ArrayDim==1 )
		*(UObject**)Dest = *(UObject**)Src;
	else
		for( INT i=0; i<ArrayDim; i++ )
			((UObject**)Dest)[i] = ((UObject**)Src)[i];
	unguardSlow;
}
UBOOL UObjectProperty::Identical( const void* A, const void* B ) const
{
	guardSlow(UObjectProperty::Identical);
	return *(UObject**)A == (B ? *(UObject**)B : NULL);
	unguardobjSlow;
}
void UObjectProperty::SerializeItem( FArchive& Ar, void* Value ) const
{
	guardSlow(UObjectProperty::SerializeItem);
	Ar << *(UObject**)Value;
	unguardobjSlow;
}
UBOOL UObjectProperty::NetSerializeItem( FArchive& Ar, UPackageMap* Map, void* Data ) const
{
	guardSlow(UByteProperty::NetSerializeItem);
	return Map->SerializeObject( Ar, PropertyClass, *(UObject**)Data );
	unguardobjSlow;
}
void UObjectProperty::Serialize( FArchive& Ar )
{
	guard(UObjectProperty::Serialize);
	Super::Serialize( Ar );
	Ar << PropertyClass;
	unguardobj;
}
void UObjectProperty::ExportCppItem( FOutputDevice& Out ) const
{
	guard(UObjectProperty::ExportCppItem);
	Out.Logf( TEXT("class %s*"), PropertyClass->GetNameCPP() );
	unguardobj;
}
void UObjectProperty::ExportTextItem( TCHAR* ValueStr, BYTE* PropertyValue, BYTE* DefaultValue, INT PortFlags ) const
{
	guard(UObjectProperty::ExportTextItem);
	UObject* Temp = *(UObject **)PropertyValue;
	if( Temp != NULL )
		appSprintf( ValueStr, TEXT("%s'%s'"), Temp->GetClass()->GetName(), Temp->GetPathName() );
	else
		appStrcpy( ValueStr, TEXT("None") );
	unguardobj;
}
const TCHAR* UObjectProperty::ImportText( const TCHAR* Buffer, BYTE* Data, INT PortFlags ) const
{
	guard(UObjectProperty::ImportText);
	TCHAR Temp[1024], Other[1024];
	Buffer = ReadToken( Buffer, Temp, ARRAY_COUNT(Temp), 1 );
	if( !Buffer )
	{
		return NULL;
	}
	if( appStricmp( Temp, TEXT("None") )==0 )
	{
		*(UObject**)Data = NULL;
	}
	else
	{
		while( *Buffer == ' ' )
			Buffer++;
		if( *Buffer++ != '\'' )
		{
			*(UObject**)Data = StaticFindObject( PropertyClass, ANY_PACKAGE, Temp );
			if( !*(UObject**)Data )
				return NULL;
		}
		else
		{
			Buffer = ReadToken( Buffer, Other, ARRAY_COUNT(Temp), 1 );
			if( !Buffer )
				return NULL;
			if( *Buffer++ != '\'' )
				return NULL;
			UClass* ObjectClass = FindObject<UClass>( ANY_PACKAGE, Temp );
			if( !ObjectClass )
				return NULL;
			*(UObject**)Data = StaticFindObject( ObjectClass, ANY_PACKAGE, Other );
			if( !*(UObject**)Data )
				return NULL;
		}
	}
	return Buffer;
	unguardobj;
}
IMPLEMENT_CLASS(UObjectProperty);

/*-----------------------------------------------------------------------------
	UClassProperty.
-----------------------------------------------------------------------------*/

void UClassProperty::Serialize( FArchive& Ar )
{
	guard(UClassProperty::Serialize);
	Super::Serialize( Ar );
	Ar << MetaClass;
	check(MetaClass);
	unguardobj;
}
const TCHAR* UClassProperty::ImportText( const TCHAR* Buffer, BYTE* Data, INT PortFlags ) const
{
	guard(UClassProperty::ImportText);
	const TCHAR* Result = UObjectProperty::ImportText( Buffer, Data, PortFlags );
	if( Result )
	{
		// Validate metaclass.
		UClass*& C = *(UClass**)Data;
		if( C && C->GetClass()!=UClass::StaticClass() || !C->IsChildOf(MetaClass) )
			C = NULL;
	}
	return Result;
	unguard;
}
IMPLEMENT_CLASS(UClassProperty);

/*-----------------------------------------------------------------------------
	UNameProperty.
-----------------------------------------------------------------------------*/

void UNameProperty::Link( FArchive& Ar, UProperty* Prev )
{
	guard(UNameProperty::Link);
	Super::Link( Ar, Prev );
	ElementSize = appCheckedIntSize(sizeof(FName));
	Offset      = Align( GetOuterUField()->GetPropertiesSize(), GetAlignment() );
	unguardobj;
}
void UNameProperty::CopySingleValue( void* Dest, void* Src ) const
{
	guardSlow(UNameProperty::CopySingleValue);
	*(FName*)Dest = *(FName*)Src;
	unguardSlow;
}
void UNameProperty::CopyCompleteValue( void* Dest, void* Src ) const
{
	guardSlow(UNameProperty::CopyCompleteValue);
	if( ArrayDim==1 )
		*(FName*)Dest = *(FName*)Src;
	else
		for( INT i=0; i<ArrayDim; i++ )
			((FName*)Dest)[i] = ((FName*)Src)[i];
	unguardSlow;
}
UBOOL UNameProperty::Identical( const void* A, const void* B ) const
{
	guardSlow(UNameProperty::Identical);
	return *(FName*)A == (B ? *(FName*)B : NAME_None);
	unguardobjSlow;
}
void UNameProperty::SerializeItem( FArchive& Ar, void* Value ) const
{
	guardSlow(UNameProperty::SerializeItem);
	Ar << *(FName*)Value;
	unguardobjSlow;
}
void UNameProperty::ExportCppItem( FOutputDevice& Out ) const
{
	guard(UNameProperty::ExportCppItem);
	Out.Log( TEXT("FName") );
	unguardobj;
}
void UNameProperty::ExportTextItem( TCHAR* ValueStr, BYTE* PropertyValue, BYTE* DefaultValue, INT PortFlags ) const
{
	guard(UNameProperty::ExportTextItem);
	const TCHAR* Str = **(FName*)PropertyValue;
	
	// Search for non-token chars.
	for( const TCHAR* S=Str; *S; S++ )
		if( *S==' ' || *S==',' )
		{
			appSprintf( ValueStr, TEXT("\"%s\""), Str );
			return;
		}

	appStrcpy( ValueStr, Str );
	unguardobj;
}
const TCHAR* UNameProperty::ImportText( const TCHAR* Buffer, BYTE* Data, INT PortFlags ) const
{
	guard(UNameProperty::ImportText);
	TCHAR Temp[1024];
	Buffer = ReadToken( Buffer, Temp, ARRAY_COUNT(Temp) );
	if( !Buffer )
		return NULL;
	*(FName*)Data = FName(Temp);
	return Buffer;
	unguardobj;
}
IMPLEMENT_CLASS(UNameProperty);

/*-----------------------------------------------------------------------------
	UStrProperty.
-----------------------------------------------------------------------------*/

void UStrProperty::Link( FArchive& Ar, UProperty* Prev )
{
	guard(UStrProperty::Link);
	Super::Link( Ar, Prev );
	ElementSize    = appCheckedIntSize(sizeof(FString));
	Offset         = Align( GetOuterUField()->GetPropertiesSize(), GetAlignment() );
	if( !(PropertyFlags & CPF_Native) )
		PropertyFlags |= CPF_NeedCtorLink;
	unguardobj;
}
UBOOL UStrProperty::Identical( const void* A, const void* B ) const
{
	guardSlow(UStrProperty::Identical);
	return appStricmp( **(const FString*)A, B ? **(const FString*)B : TEXT("") )==0;
	unguardobjSlow;
}
void UStrProperty::SerializeItem( FArchive& Ar, void* Value ) const
{
	guardSlow(UStrProperty::SerializeItem);
	Ar << *(FString*)Value;
	unguardobjSlow;
}
void UStrProperty::Serialize( FArchive& Ar )
{
	guard(UStrProperty::Serialize);
	Super::Serialize( Ar );
	unguardobj;
}
void UStrProperty::ExportCppItem( FOutputDevice& Out ) const
{
	guard(UStrProperty::ExportCppItem);
	Out.Log( TEXT("FString") );
	unguardobj;
}
void UStrProperty::ExportTextItem( TCHAR* ValueStr, BYTE* PropertyValue, BYTE* DefaultValue, INT PortFlags ) const
{
	guard(UStrProperty::ExportTextItem);
	if( !(PortFlags & PPF_Delimited) )
		appStrcpy( ValueStr, **(FString*)PropertyValue );
	else
		appSprintf( ValueStr, TEXT("\"%s\""), **(FString*)PropertyValue );
	unguardobj;
}
const TCHAR* UStrProperty::ImportText( const TCHAR* Buffer, BYTE* Data, INT PortFlags ) const
{
	guard(UStrProperty::ImportText);
	if( !(PortFlags & PPF_Delimited) )
	{
		*(FString*)Data = Buffer;
	}
	else
	{
		TCHAR Temp[4096];//!!
		Buffer = ReadToken( Buffer, Temp, ARRAY_COUNT(Temp) );
		if( !Buffer )
			return NULL;
		*(FString*)Data = Temp;
	}
	return Buffer;
	unguardobj;
}
void UStrProperty::CopySingleValue( void* Dest, void* Src ) const
{
	guardSlow(UStrProperty::CopySingleValue);
	*(FString*)Dest = *(FString*)Src;
	unguardobjSlow;
}
void UStrProperty::DestroyValue( void* Dest ) const
{
	guardSlow(UStrProperty::DestroyValue);
	for( INT i=0; i<ArrayDim; i++ )
		(*(FString*)((BYTE*)Dest+i*ElementSize)).~FString();
	unguardobjSlow;
}
IMPLEMENT_CLASS(UStrProperty);

/*-----------------------------------------------------------------------------
	UFixedArrayProperty.
-----------------------------------------------------------------------------*/

void UFixedArrayProperty::Link( FArchive& Ar, UProperty* Prev )
{
	guard(UFixedArrayProperty::Link);
	checkSlow(Count>0);
	Super::Link( Ar, Prev );
	Ar.Preload( Inner );
	Inner->Link( Ar, NULL );
	ElementSize    = appCheckedIntSize(static_cast<SIZE_T>(Inner->ElementSize)*static_cast<SIZE_T>(Count));
	Offset         = Align( GetOuterUField()->GetPropertiesSize(), GetAlignment() );
	if( !(PropertyFlags & CPF_Native) )
		PropertyFlags |= (Inner->PropertyFlags & CPF_NeedCtorLink);
	unguardobj;
}
UBOOL UFixedArrayProperty::Identical( const void* A, const void* B ) const
{
	guardSlow(UFixedArrayProperty::Identical);
	checkSlow(Inner);
	for( INT i=0; i<Count; i++ )
		if( !Inner->Identical( (BYTE*)A+i*Inner->ElementSize, B ? ((BYTE*)B+i*Inner->ElementSize) : NULL ) )
			return 0;
	return 1;
	unguardobjSlow;
}
void UFixedArrayProperty::SerializeItem( FArchive& Ar, void* Value ) const
{
	guardSlow(UFixedArrayProperty::SerializeItem);
	checkSlow(Inner);
	for( INT i=0; i<Count; i++ )
		Inner->SerializeItem( Ar, (BYTE*)Value + i*Inner->ElementSize );
	unguardobjSlow;
}
UBOOL UFixedArrayProperty::NetSerializeItem( FArchive& Ar, UPackageMap* Map, void* Data ) const
{
	guardSlow(UByteProperty::NetSerializeItem);
	return 1;
	unguardobjSlow;
}
void UFixedArrayProperty::Serialize( FArchive& Ar )
{
	guard(UFixedArrayProperty::Serialize);
	Super::Serialize( Ar );
	Ar << Inner << Count;
	checkSlow(Inner);
	unguardobj;
}
void UFixedArrayProperty::ExportCppItem( FOutputDevice& Out ) const
{
	guard(UFixedArrayProperty::ExportCppItem);
	checkSlow(Inner);
	Inner->ExportCppItem( Out );
	Out.Logf( TEXT("[%i]"), Count );
	unguardobj;
}
void UFixedArrayProperty::ExportTextItem( TCHAR* ValueStr, BYTE* PropertyValue, BYTE* DefaultValue, INT PortFlags ) const
{
	guard(UFixedArrayProperty::ExportTextItem);
	checkSlow(Inner);
	*ValueStr++ = '(';
	for( INT i=0; i<Count; i++ )
	{
		if( i>0 )
			*ValueStr++ = ',';
		Inner->ExportTextItem( ValueStr, PropertyValue + i*Inner->ElementSize, DefaultValue ? (DefaultValue + i*Inner->ElementSize) : NULL, PortFlags|PPF_Delimited );
		ValueStr += appStrlen(ValueStr);
	}
	*ValueStr++ = ')';
	*ValueStr++ = 0;
	unguardobj;
}
const TCHAR* UFixedArrayProperty::ImportText( const TCHAR* Buffer, BYTE* Data, INT PortFlags ) const
{
	guard(UFixedArrayProperty::ImportText);
	checkSlow(Inner);
	if( *Buffer++ != '(' )
		return NULL;
	appMemzero( Data, ElementSize );
	for( INT i=0; i<Count; i++ )
	{
		Buffer = Inner->ImportText( Buffer, Data + i*Inner->ElementSize, PortFlags|PPF_Delimited );
		if( !Buffer )
			return NULL;
		if( *Buffer!=',' && i!=Count-1 )
			return NULL;
		Buffer++;
	}
	if( *Buffer++ != ')' )
		return NULL;
	return Buffer;
	unguardobj;
}
void UFixedArrayProperty::AddCppProperty( UProperty* Property, INT InCount )
{
	guard(UFixedArrayProperty::AddCppProperty);
	check(!Inner);
	check(Property);
	check(InCount>0);

	Inner = Property;
	Count = InCount;

	unguardobj;
}
void UFixedArrayProperty::CopySingleValue( void* Dest, void* Src ) const
{
	guardSlow(UFixedArrayProperty::CopySingleValue);
	for( INT i=0; i<Count; i++ )
		Inner->CopyCompleteValue( (BYTE*)Dest + i*Inner->ElementSize, Src ? ((BYTE*)Src + i*Inner->ElementSize) : NULL );
	unguardobjSlow;
}
void UFixedArrayProperty::DestroyValue( void* Dest ) const
{
	guardSlow(UFixedArrayProperty::DestroyValue);
	for( INT i=0; i<Count; i++ )
		Inner->DestroyValue( (BYTE*)Dest + i*Inner->ElementSize );
	unguardobjSlow;
}
IMPLEMENT_CLASS(UFixedArrayProperty);

/*-----------------------------------------------------------------------------
	UArrayProperty.
-----------------------------------------------------------------------------*/

void UArrayProperty::Link( FArchive& Ar, UProperty* Prev )
{
	guard(UArrayProperty::Link);
	Super::Link( Ar, Prev );
	Ar.Preload( Inner );
	Inner->Link( Ar, NULL );
	ElementSize    = appCheckedIntSize(sizeof(FArray));
	Offset         = Align( GetOuterUField()->GetPropertiesSize(), GetAlignment() );
	if( !(PropertyFlags & CPF_Native) )
		PropertyFlags |= CPF_NeedCtorLink;
	unguardobj;
}
UBOOL UArrayProperty::Identical( const void* A, const void* B ) const
{
	guardSlow(UArrayProperty::Identical);
	checkSlow(Inner);
	INT n = ((FArray*)A)->Num();
	if( n!=(B ? ((FArray*)B)->Num() : 0) )
		return 0;
	INT   c = Inner->ElementSize;
	BYTE* p = (BYTE*)((FArray*)A)->GetData();
	if( B )
	{
		BYTE* q = (BYTE*)((FArray*)B)->GetData();
		for( INT i=0; i<n; i++ )
			if( !Inner->Identical( p+i*c, q+i*c ) )
				return 0;
	}
	else
	{
		for( INT i=0; i<n; i++ )
			if( !Inner->Identical( p+i*c, 0 ) )
				return 0;
	}
	return 1;
	unguardobjSlow;
}
void UArrayProperty::SerializeItem( FArchive& Ar, void* Value ) const
{
	guardSlow(UArrayProperty::SerializeItem);
	checkSlow(Inner);
	INT   c = Inner->ElementSize;
	INT   n = ((FArray*)Value)->Num();
	Ar << AR_INDEX(n);
	if( Ar.IsLoading() )
	{
		((FArray*)Value)->Empty( c );
		((FArray*)Value)->Add( n, c );
	}
	BYTE* p = (BYTE*)((FArray*)Value)->GetData();
	for( INT i=0; i<n; i++ )
		Inner->SerializeItem( Ar, p+i*c );
	unguardobjSlow;
}
UBOOL UArrayProperty::NetSerializeItem( FArchive& Ar, UPackageMap* Map, void* Data ) const
{
	guardSlow(UByteProperty::NetSerializeItem);
	return 1;
	unguardobjSlow;
}
void UArrayProperty::Serialize( FArchive& Ar )
{
	guard(UArrayProperty::Serialize);
	Super::Serialize( Ar );
	Ar << Inner;
	checkSlow(Inner);
	unguardobj;
}
void UArrayProperty::ExportCppItem( FOutputDevice& Out ) const
{
	guard(UArrayProperty::ExportCppItem);
	checkSlow(Inner);
	Out.Log( TEXT("TArray<") );
	Inner->ExportCppItem( Out );
	Out.Log( TEXT(">") );
	unguardobj;
}
void UArrayProperty::ExportTextItem( TCHAR* ValueStr, BYTE* PropertyValue, BYTE* DefaultValue, INT PortFlags ) const
{
	guard(UArrayProperty::ExportTextItem);
	checkSlow(Inner);
	*ValueStr++ = '(';
	FArray* Array       = (FArray*)PropertyValue;
	FArray* Default     = (FArray*)DefaultValue;
	INT     ElementSize = Inner->ElementSize;
	for( INT i=0; i<Array->Num(); i++ )
	{
		if( i>0 )
			*ValueStr++ = ',';
		Inner->ExportTextItem( ValueStr, (BYTE*)Array->GetData() + i*ElementSize, Default ? (BYTE*)Default->GetData() + i*ElementSize : 0, PortFlags|PPF_Delimited );
		ValueStr += appStrlen(ValueStr);
	}
	*ValueStr++ = ')';
	*ValueStr++ = 0;
	unguardobj;
}
const TCHAR* UArrayProperty::ImportText( const TCHAR* Buffer, BYTE* Data, INT PortFlags ) const
{
	guard(UArrayProperty::ImportText);
	checkSlow(Inner);
	if( *Buffer++ != '(' )
		return NULL;
	FArray* Array       = (FArray*)Data;
	INT     ElementSize = Inner->ElementSize;
	Array->Empty( ElementSize );
	while( *Buffer != ')' )
	{
		INT Index = Array->Add( 1, ElementSize );
		appMemzero( (BYTE*)Array->GetData() + Index*ElementSize, ElementSize );
		Buffer = Inner->ImportText( Buffer, (BYTE*)Array->GetData() + Index*ElementSize, PortFlags|PPF_Delimited );
		if( !Buffer )
			return NULL;
		if( *Buffer!=',' )
			break;
		Buffer++;
	}
	if( *Buffer++ != ')' )
		return NULL;
	return Buffer;
	unguardobj;
}
void UArrayProperty::AddCppProperty( UProperty* Property )
{
	guard(UArrayProperty::AddCppProperty);
	check(!Inner);
	check(Property);

	Inner = Property;

	unguardobj;
}
void UArrayProperty::CopySingleValue( void* Dest, void* Src ) const
{
	guardSlow(UArrayProperty::CopySingleValue);
	FArray* SrcArray  = (FArray*)Src;
	FArray* DestArray = (FArray*)Dest;
	INT     Size      = Inner->ElementSize;
	DestArray->Empty( Size, SrcArray->Num() );//!!must destruct it if really copying
	if( Inner->PropertyFlags & CPF_NeedCtorLink )
	{
		// Copy all the elements.
		DestArray->AddZeroed( Size, SrcArray->Num() );
		BYTE* SrcData  = (BYTE*)SrcArray->GetData();
		BYTE* DestData = (BYTE*)DestArray->GetData();
		for( INT i=0; i<DestArray->Num(); i++ )
			Inner->CopyCompleteValue( DestData+i*Size, SrcData+i*Size );
	}
	else
	{
		// Copy all the elements.
		DestArray->Add( SrcArray->Num(), Size );
		appMemcpy( DestArray->GetData(), SrcArray->GetData(), SrcArray->Num()*Size );
	}
	unguardobjSlow;
}
void UArrayProperty::DestroyValue( void* Dest ) const
{
	guardSlow(UArrayProperty::DestroyValue);
	FArray* DestArray = (FArray*)Dest;
	if( Inner->PropertyFlags & CPF_NeedCtorLink )
	{
		BYTE* DestData = (BYTE*)DestArray->GetData();
		INT   Size     = Inner->ElementSize;
		for( INT i=0; i<DestArray->Num(); i++ )
			Inner->DestroyValue( DestData+i*Size );
	}
	DestArray->~FArray();
	unguardobjSlow;
}
IMPLEMENT_CLASS(UArrayProperty);

/*-----------------------------------------------------------------------------
	UMapProperty.
-----------------------------------------------------------------------------*/

void UMapProperty::Link( FArchive& Ar, UProperty* Prev )
{
	guard(UMapProperty::Link);
	Super::Link( Ar, Prev );
	Ar.Preload( Key );
	Key->Link( Ar, NULL );
	Ar.Preload( Value );
	Value->Link( Ar, NULL );
	ElementSize    = appCheckedIntSize(sizeof(TMap<BYTE,BYTE>));
	Offset         = Align( GetOuterUField()->GetPropertiesSize(), GetAlignment() );
	if( !(PropertyFlags&CPF_Native) )
		PropertyFlags |= CPF_NeedCtorLink;
	unguardobj;
}
UBOOL UMapProperty::Identical( const void* A, const void* B ) const
{
	guardSlow(UMapProperty::Identical);
	checkSlow(Key);
	checkSlow(Value);
	/*
	INT n = ((FArray*)A)->Num();
	if( n!=(B ? ((FArray*)B)->Num() : 0) )
		return 0;
	INT   c = Inner->ElementSize;
	BYTE* p = (BYTE*)((FArray*)A)->GetData();
	if( B )
	{
		BYTE* q = (BYTE*)((FArray*)B)->GetData();
		for( INT i=0; i<n; i++ )
			if( !Inner->Identical( p+i*c, q+i*c ) )
				return 0;
	}
	else
	{
		for( INT i=0; i<n; i++ )
			if( !Inner->Identical( p+i*c, 0 ) )
				return 0;
	}
	*/
	return 1;
	unguardobjSlow;
}
void UMapProperty::SerializeItem( FArchive& Ar, void* Value ) const
{
	guardSlow(UMapProperty::SerializeItem);
	checkSlow(Key);
	checkSlow(Value);
	/*
	INT   c = Inner->ElementSize;
	INT   n = ((FArray*)Value)->Num();
	Ar << AR_INDEX(n);
	if( Ar.IsLoading() )
	{
		((FArray*)Value)->Empty( c );
		((FArray*)Value)->Add( n, c );
	}
	BYTE* p = (BYTE*)((FArray*)Value)->GetData();
	for( INT i=0; i<n; i++ )
		Inner->SerializeItem( Ar, p+i*c );
	*/
	unguardobjSlow;
}
UBOOL UMapProperty::NetSerializeItem( FArchive& Ar, UPackageMap* Map, void* Data ) const
{
	guardSlow(UByteProperty::NetSerializeItem);
	return 1;
	unguardobjSlow;
}
void UMapProperty::Serialize( FArchive& Ar )
{
	guard(UMapProperty::Serialize);
	Super::Serialize( Ar );
	Ar << Key << Value;
	checkSlow(Key);
	checkSlow(Value);
	unguardobj;
}
void UMapProperty::ExportCppItem( FOutputDevice& Out ) const
{
	guard(UMapProperty::ExportCppItem);
	checkSlow(Key);
	checkSlow(Value);
	Out.Log( TEXT("TMap<") );
	Key->ExportCppItem( Out );
	Out.Log( TEXT(",") );
	Value->ExportCppItem( Out );
	Out.Log( TEXT(">") );
	unguardobj;
}
void UMapProperty::ExportTextItem( TCHAR* ValueStr, BYTE* PropertyValue, BYTE* DefaultValue, INT PortFlags ) const
{
	guard(UMapProperty::ExportTextItem);
	checkSlow(Key);
	checkSlow(Value);
	/*
	*ValueStr++ = '(';
	FArray* Array       = (FArray*)PropertyValue;
	FArray* Default     = (FArray*)DefaultValue;
	INT     ElementSize = Inner->ElementSize;
	for( INT i=0; i<Array->Num(); i++ )
	{
		if( i>0 )
			*ValueStr++ = ',';
		Inner->ExportTextItem( ValueStr, (BYTE*)Array->GetData() + i*ElementSize, Default ? (BYTE*)Default->GetData() + i*ElementSize : 0, PortFlags|PPF_Delimited );
		ValueStr += appStrlen(ValueStr);
	}
	*ValueStr++ = ')';
	*ValueStr++ = 0;
	*/
	unguardobj;
}
const TCHAR* UMapProperty::ImportText( const TCHAR* Buffer, BYTE* Data, INT PortFlags ) const
{
	guard(UMapProperty::ImportText);
	checkSlow(Key);
	checkSlow(Value);
	/*
	if( *Buffer++ != '(' )
		return NULL;
	FArray* Array       = (FArray*)Data;
	INT     ElementSize = Inner->ElementSize;
	Array->Empty( ElementSize );
	while( *Buffer != ')' )
	{
		INT Index = Array->Add( 1, ElementSize );
		appMemzero( (BYTE*)Array->GetData() + Index*ElementSize, ElementSize );
		Buffer = Inner->ImportText( Buffer, (BYTE*)Array->GetData() + Index*ElementSize, PortFlags|PPF_Delimited );
		if( !Buffer )
			return NULL;
		if( *Buffer!=',' )
			break;
		Buffer++;
	}
	if( *Buffer++ != ')' )
		return NULL;
	*/
	return Buffer;
	unguardobj;
}
void UMapProperty::CopySingleValue( void* Dest, void* Src ) const
{
	guardSlow(UMapProperty::CopySingleValue);
	/*
	TMap<BYTE,BYTE>* SrcMap    = (TMap<BYTE,BYTE>*)Src;
	TMap<BYTE,BYTE>* DestMap   = (TMap<BYTE,BYTE>*)Dest;
	INT              KeySize   = Key->ElementSize;
	INT              ValueSize = Value->ElementSize;
	DestMap->Empty( Size, SrcArray->Num() );//must destruct it if really copying
	if( Inner->PropertyFlags & CPF_NeedsCtorLink )
	{
		// Copy all the elements.
		DestArray->AddZeroed( Size, SrcArray->Num() );
		BYTE* SrcData  = (BYTE*)SrcArray->GetData();
		BYTE* DestData = (BYTE*)DestArray->GetData();
		for( INT i=0; i<DestArray->Num(); i++ )
			Inner->CopyCompleteValue( DestData+i*Size, SrcData+i*Size );
	}
	else
	{
		// Copy all the elements.
		DestArray->Add( SrcArray->Num(), Size );
		appMemcpy( DestArray->GetData(), SrcArray->GetData(), SrcArray->Num()*Size );
	}*/
	unguardobjSlow;
}
void UMapProperty::DestroyValue( void* Dest ) const
{
	guardSlow(UMapProperty::DestroyValue);
	/*
	FArray* DestArray = (FArray*)Dest;
	if( Inner->PropertyFlags & CPF_NeedsCtorLink )
	{
		BYTE* DestData = (BYTE*)DestArray->GetData();
		INT   Size     = Inner->ElementSize;
		for( INT i=0; i<DestArray->Num(); i++ )
			Inner->DestroyValue( DestData+i*Size );
	}
	DestArray->~FArray();
	*/
	unguardobjSlow;
}
IMPLEMENT_CLASS(UMapProperty);

/*-----------------------------------------------------------------------------
	UStructProperty.
-----------------------------------------------------------------------------*/

void UStructProperty::Link( FArchive& Ar, UProperty* Prev )
{
	guard(UStructProperty::Link);
	Super::Link( Ar, Prev );
	Ar.Preload( Struct );
	ElementSize    = Struct->PropertiesSize;
#ifdef __PSX2_EE__
	if (appStrcmp( Struct->GetName(), TEXT("Vector") ) == 0)
		Offset = Align( GetOuterUField()->GetPropertiesSize(), 16 );
	else if (appStrcmp( Struct->GetName(), TEXT("Plane") ) == 0)
		Offset = Align( GetOuterUField()->GetPropertiesSize(), 16 );
	else if (appStrcmp( Struct->GetName(), TEXT("Coords") ) == 0)
		Offset = Align( GetOuterUField()->GetPropertiesSize(), 16 );
	else
#endif
		Offset = Align( GetOuterUField()->GetPropertiesSize(), GetAlignment() );
	if( Struct->ConstructorLink && !(PropertyFlags & CPF_Native) )
		PropertyFlags |= CPF_NeedCtorLink;
	unguardobj;
}
UBOOL UStructProperty::Identical( const void* A, const void* B ) const
{
	guardSlow(UStructProperty::Identical);
	for( TFieldIterator<UProperty> It(Struct); It; ++It )
		for( INT i=0; i<It->ArrayDim; i++ )
			if( !It->Matches(A,B,i) )
				return 0;
	return 1;
	unguardobjSlow;
}
void UStructProperty::SerializeItem( FArchive& Ar, void* Value ) const
{
	guardSlow(UStructProperty::SerializeItem);
	Ar.Preload( Struct );
	Struct->SerializeBin( Ar, (BYTE*)Value );
	unguardobjSlow;
}
UBOOL UStructProperty::NetSerializeItem( FArchive& Ar, UPackageMap* Map, void* Data ) const
{
	guardSlow(UStructProperty::NetSerializeItem);
	if( Struct->GetFName()==NAME_Vector )
	{
		FVector& V = *(FVector*)Data;
#if ENGINE_VERSION<230 //oldver
		SWORD X(appRound(V.X)), Y(appRound(V.Y)), Z(appRound(V.Z));
		Ar << X << Y << Z;
		if( Ar.IsLoading() )
			V = FVector(X,Y,Z);
#else
		INT X(appRound(V.X)), Y(appRound(V.Y)), Z(appRound(V.Z));
		DWORD Bits = Clamp<DWORD>( appCeilLogTwo(1+Max(Max(Abs(X),Abs(Y)),Abs(Z))), 1, 16 )-1;
		Ar.SerializeInt( Bits, 16 );
		INT   Bias = 1<<(Bits+1);
		DWORD Max  = 1<<(Bits+2);
		DWORD DX(X+Bias), DY(Y+Bias), DZ(Z+Bias);
		Ar.SerializeInt( DX, Max );
		Ar.SerializeInt( DY, Max );
		Ar.SerializeInt( DZ, Max );
		if( Ar.IsLoading() )
			V = FVector((INT)DX-Bias,(INT)DY-Bias,(INT)DZ-Bias);
#endif
	}
	else if( Struct->GetFName()==NAME_Rotator )
	{
		FRotator& R = *(FRotator*)Data;
		BYTE Pitch(R.Pitch>>8), Yaw(R.Yaw>>8), Roll(R.Roll>>8), B;
		B = (Pitch!=0);
		Ar.SerializeBits( &B, 1 );
		if( B )
			Ar << Pitch;
		B = (Yaw!=0);
		Ar.SerializeBits( &B, 1 );
		if( B )
			Ar << Yaw;
		B = (Roll!=0);
		Ar.SerializeBits( &B, 1 );
		if( B )
			Ar << Roll;
		if( Ar.IsLoading() )
			R = FRotator(Pitch<<8,Yaw<<8,Roll<<8);
	}
	else if( Struct->GetFName()==NAME_Plane )
	{
		FPlane& P = *(FPlane*)Data;
		SWORD X(appRound(P.X)), Y(appRound(P.Y)), Z(appRound(P.Z)), W(appRound(P.W));
		Ar << X << Y << Z << W;
		if( Ar.IsLoading() )
			P = FPlane(X,Y,Z,W);
	}
	else
	{
		for( TFieldIterator<UProperty> It(Struct); It; ++It )
			if( Map->ObjectToIndex(*It)!=INDEX_NONE )
				for( INT i=0; i<It->ArrayDim; i++ )
					It->NetSerializeItem( Ar, Map, (BYTE*)Data+It->Offset+i*It->ElementSize );
	}
	return 1;
	unguardobjSlow;
}
void UStructProperty::Serialize( FArchive& Ar )
{
	guard(UStructProperty::Serialize);
	Super::Serialize( Ar );
	Ar << Struct;
	unguardobj;
}
void UStructProperty::ExportCppItem( FOutputDevice& Out ) const
{
	guard(UStructProperty::ExportCppItem);
	Out.Logf( TEXT("%s"), Struct->GetNameCPP() );
	unguardobj;
}
void UStructProperty::ExportTextItem( TCHAR* ValueStr, BYTE* PropertyValue, BYTE* DefaultValue, INT PortFlags ) const
{
	guard(UStructProperty::ExportTextItem);
	INT Count=0;
	for( TFieldIterator<UProperty> It(Struct); It; ++It )
	{
		if( It->Port() )
		{
			for( INT Index=0; Index<It->ArrayDim; Index++ )
			{
				TCHAR Value[1024];
				if( It->ExportText(Index,Value,PropertyValue,DefaultValue,PPF_Delimited) )
				{
					Count++;
					if( Count == 1 )
						*ValueStr++ = '(';
					else
						*ValueStr++ = ',';
					if( It->ArrayDim == 1 )
						ValueStr += appSprintf( ValueStr, TEXT("%s="), It->GetName() );
					else
						ValueStr += appSprintf( ValueStr, TEXT("%s[%i]="), It->GetName(), Index );
					ValueStr += appSprintf( ValueStr, TEXT("%s"), Value );
				}
			}
		}
	}
	if( Count > 0 )
	{
		*ValueStr++ = ')';
		*ValueStr = 0;
	}
	unguardobj;
}
const TCHAR* UStructProperty::ImportText( const TCHAR* Buffer, BYTE* Data, INT PortFlags ) const
{
	guard(UStructProperty::ImportText);
	if( *Buffer++ == '(' )
	{
		// Parse all properties.
		while( *Buffer != ')' )
		{
			// Get key name.
			TCHAR Name[NAME_SIZE];
			int Count=0;
			while( Count<NAME_SIZE-1 && *Buffer && *Buffer!='=' && *Buffer!='[' )
				Name[Count++] = *Buffer++;
			Name[Count++] = 0;

			// Get optional array element.
			INT Element=0;
			if( *Buffer=='[' )
			{
				Buffer++;
				const TCHAR* Start=Buffer;
				while( *Buffer>='0' && *Buffer<='9' )
					Buffer++;
				if( *Buffer++ != ']' )
				{
					debugf( NAME_Warning, TEXT("ImportText: Illegal array element") );
					return NULL;
				}
				Element = appAtoi( Start );
			}

			// Verify format.
			if( *Buffer++ != '=' )
			{
				debugf( NAME_Warning, TEXT("ImportText: Illegal or missing key name") );
				return NULL;
			}

			// See if the property exists in the struct.
			FName GotName( Name, FNAME_Find );
			UBOOL Parsed = 0;
			if( GotName!=NAME_None )
			{
				for( TFieldIterator<UProperty> It(Struct); It; ++It )
				{
					UProperty* Property = *It;
					if
					(	Property->GetFName()==GotName
					&&	Element>=0
					&&	Element<Property->ArrayDim
					&&	Property->GetSize()!=0
					&&	Property->Port() )
					{
						// Import this property.
						Buffer = Property->ImportText( Buffer, Data + Property->Offset + Element*Property->ElementSize, PortFlags|PPF_Delimited );
						if( Buffer == NULL )
							return NULL;

						// Done with this property.
						Parsed = 1;
					}
				}
			}

			// If not parsed, skip this property in the stream.
			if( !Parsed )
			{
				GWarn->Logf( NAME_Warning, TEXT("ImportText: Unknown element %s of struct %s"), Name, GetName() );
				INT SubCount=0;
				while
				(	*Buffer
				&&	*Buffer!=10
				&&	*Buffer!=13 
				&&	(SubCount>0 || *Buffer!=')')
				&&	(SubCount>0 || *Buffer!=',') )
				{
					if( *Buffer == 0x22 )
					{
						while( *Buffer && *Buffer!=0x22 && *Buffer!=10 && *Buffer!=13 )
							Buffer++;
						if( *Buffer != 0x22 )
						{
							GWarn->Logf( NAME_Warning, TEXT("ImportText: Bad quoted string") );
							return NULL;
						}
					}
					else if( *Buffer == '(' )
					{
						SubCount++;
					}
					else if( *Buffer == ')' )
					{
						SubCount--;
						if( SubCount < 0 )
						{
							debugf( NAME_Warning, TEXT("ImportText: Bad parenthesised struct") );
							return NULL;
						}
					}
					Buffer++;
				}
				if( SubCount > 0 )
				{
					debugf( NAME_Warning, TEXT("ImportText: Incomplete parenthesised struct") );
					return NULL;
				}
			}

			// Skip comma.
			if( *Buffer==',' )
			{
				// Skip comma.
				Buffer++;
			}
			else if( *Buffer!=')' )
			{
				debugf( NAME_Warning, TEXT("ImportText: Bad termination") );
				return NULL;
			}
		}

		// Skip trailing ')'.
		Buffer++;
	}
	else
	{
		debugf( NAME_Warning, TEXT("ImportText: Struct missing '('") );
		return NULL;
	}
	return Buffer;
	unguardobj;
}
void UStructProperty::CopySingleValue( void* Dest, void* Src ) const
{
	guardSlow(UStructProperty::CopySingleValue);
	for( TFieldIterator<UProperty> It(Struct); It; ++It )
		It->CopyCompleteValue( (BYTE*)Dest + It->Offset, (BYTE*)Src + It->Offset );
	//could just do memcpy + ReinstanceCompleteValue
	unguardobjSlow;
}
void UStructProperty::DestroyValue( void* Dest ) const
{
	guardSlow(UStructProperty::DestroyValue);
	for( UProperty* P=Struct->ConstructorLink; P; P=P->ConstructorLinkNext )
		for( INT i=0; i<ArrayDim; i++ )
			P->DestroyValue( (BYTE*)Dest + i*ElementSize + P->Offset );
	unguardobjSlow;
}
IMPLEMENT_CLASS(UStructProperty);

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/

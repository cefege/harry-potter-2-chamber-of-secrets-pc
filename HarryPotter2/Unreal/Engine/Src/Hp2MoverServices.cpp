/*=============================================================================
	Hp2MoverServices.cpp: Thunks the Rust AMover port calls back into.
	Each thunk performs one of the original UObject/UnProp operations.
=============================================================================*/

#include "EnginePrivate.h"
#include "Hp2MoverServices.h"

static void* MoverFindStruct( const char* Name )
{
	return FindObject<UStruct>( UObject::StaticClass(), ANSI_TO_TCHAR(Name) );
}

static void* MoverNewStruct( const char* Name, int Size )
{
	UStruct* Struct = new(UObject::StaticClass(),ANSI_TO_TCHAR(Name),RF_Public)UStruct(NULL);
	Struct->SetPropertiesSize( Size );
	return Struct;
}

static void MoverLinkStruct( void* Struct )
{
	FArchive ArDummy;
	((UStruct*)Struct)->Link( ArDummy, 0 );
}

static void* MoverNewEnum( const char* Name )
{
	return new(AMover::StaticClass(),ANSI_TO_TCHAR(Name))UEnum(NULL);
}

static void MoverEnumAddName( void* Enum, const char* Name )
{
	new(((UEnum*)Enum)->Names)FName(ANSI_TO_TCHAR(Name));
}

static void* MoverClassRef( int Id )
{
	switch( Id )
	{
		case HP2_MOVER_CLASS_MOVER:            return AMover::StaticClass();
		case HP2_MOVER_CLASS_ACTOR:            return AActor::StaticClass();
		case HP2_MOVER_CLASS_SOUND:            return USound::StaticClass();
		case HP2_MOVER_CLASS_NAVIGATION_POINT: return ANavigationPoint::StaticClass();
		case HP2_MOVER_CLASS_PAWN:             return APawn::StaticClass();
		case HP2_MOVER_CLASS_TRIGGER:          return ATrigger::StaticClass();
	}
	appErrorf( TEXT("Hp2MoverServices: bad class ref %d"), Id );
	return NULL;
}

static void MoverAddProperty( void* Owner, int Kind, const char* Name, const char* Category, int Offset, int ArrayDim, unsigned int BitMask, void* RefObj )
{
	UStruct* Outer = (UStruct*)Owner;
	UProperty* Property = NULL;
	switch( Kind )
	{
		case HP2_MOVER_PROP_BYTE:
			Property = new(Outer,ANSI_TO_TCHAR(Name),RF_Public)UByteProperty(EC_CppProperty,Offset,ANSI_TO_TCHAR(Category),0,(UEnum*)RefObj);
			break;
		case HP2_MOVER_PROP_FLOAT:
			Property = new(Outer,ANSI_TO_TCHAR(Name),RF_Public)UFloatProperty(EC_CppProperty,Offset,ANSI_TO_TCHAR(Category),0);
			break;
		case HP2_MOVER_PROP_INT:
			Property = new(Outer,ANSI_TO_TCHAR(Name),RF_Public)UIntProperty(EC_CppProperty,Offset,ANSI_TO_TCHAR(Category),0);
			break;
		case HP2_MOVER_PROP_NAME:
			Property = new(Outer,ANSI_TO_TCHAR(Name),RF_Public)UNameProperty(EC_CppProperty,Offset,ANSI_TO_TCHAR(Category),0);
			break;
		case HP2_MOVER_PROP_OBJECT:
			Property = new(Outer,ANSI_TO_TCHAR(Name),RF_Public)UObjectProperty(EC_CppProperty,Offset,ANSI_TO_TCHAR(Category),0,(UClass*)RefObj);
			break;
		case HP2_MOVER_PROP_STRUCT:
			Property = new(Outer,ANSI_TO_TCHAR(Name),RF_Public)UStructProperty(EC_CppProperty,Offset,ANSI_TO_TCHAR(Category),0,(UStruct*)RefObj);
			break;
		case HP2_MOVER_PROP_BOOL:
		{
			UBoolProperty* Bool = new(Outer,ANSI_TO_TCHAR(Name),RF_Public)UBoolProperty(EC_CppProperty,Offset,ANSI_TO_TCHAR(Category),0);
			Bool->BitMask = BitMask;
			Property = Bool;
			break;
		}
		default:
			appErrorf( TEXT("Hp2MoverServices: bad property kind %d"), Kind );
	}
	Property->ArrayDim = ArrayDim;
}

static void MoverHashAdd( void* Hash, void* Actor )
{
	((FCollisionHashBase*)Hash)->AddActor( (AActor*)Actor );
}

static void MoverHashRemove( void* Hash, void* Actor )
{
	((FCollisionHashBase*)Hash)->RemoveActor( (AActor*)Actor );
}

static void MoverBrushTrackerUpdate( void* Tracker, void* Actor )
{
	((FMovingBrushTrackerBase*)Tracker)->Update( (AActor*)Actor );
}

static void MoverBrushTrackerFlush( void* Tracker, void* Actor )
{
	((FMovingBrushTrackerBase*)Tracker)->Flush( (AActor*)Actor );
}

static void MoverRenumberBrushPolys( void* Brush )
{
	// Fix brush poly iLinks which were broken.
	UModel* Model = (UModel*)Brush;
	if( Model && Model->Polys )
		for( INT i=0; i<Model->Polys->Element.Num(); i++ )
			Model->Polys->Element(i).iLink = i;
}

const Hp2MoverServices GHp2MoverServices =
{
	MoverFindStruct,
	MoverNewStruct,
	MoverLinkStruct,
	MoverNewEnum,
	MoverEnumAddName,
	MoverClassRef,
	MoverAddProperty,
	MoverHashAdd,
	MoverHashRemove,
	MoverBrushTrackerUpdate,
	MoverBrushTrackerFlush,
	MoverRenumberBrushPolys,
};

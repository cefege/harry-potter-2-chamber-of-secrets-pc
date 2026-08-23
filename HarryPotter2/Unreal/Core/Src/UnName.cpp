/*=============================================================================
	UnName.cpp: Unreal global name code.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "CorePrivate.h"

/*-----------------------------------------------------------------------------
	FName statics.
-----------------------------------------------------------------------------*/

// Static variables.
UBOOL				FName::Initialized = 0;
FNameEntry*			FName::NameHash[4096];
TArray<FNameEntry*>	FName::Names;
TArray<INT>         FName::Available;

/*-----------------------------------------------------------------------------
	FName implementation.
-----------------------------------------------------------------------------*/

//
// Hardcode a name.
//
void FName::Hardcode( FNameEntry* AutoName )
{
	guard(FName::Hardcode);

	// Add name to name hash.
	INT iHash          = appStrihash(AutoName->Name) & (ARRAY_COUNT(NameHash)-1);
	AutoName->HashNext = NameHash[iHash];
	NameHash[iHash]    = AutoName;

	// Expand the table if needed.
	for( INT i=Names.Num(); i<=AutoName->Index; i++ )
		Names.AddItem( NULL );

	// Add name to table.
	if( Names(AutoName->Index) )
		appErrorf( TEXT("Hardcoded name %i was duplicated"), AutoName->Index );
	Names(AutoName->Index) = AutoName;

	unguard;
}

//
// FName constructor.
//
FName::FName( const TCHAR* Name, EFindName FindType )
{
	guard(FName::FName);
	check(Name);
	if( !Initialized )
		appErrorf( TEXT("FName constructed before FName::StaticInit") );

	// If empty or invalid name was specified, return NAME_None.
	if( !Name[0] )
	{
		Index = 0;
		return;
	}

	// Try to find the name in the hash.
	INT iHash = appStrihash(Name) & (ARRAY_COUNT(NameHash)-1);
	for( FNameEntry* Hash=NameHash[iHash]; Hash; Hash=Hash->HashNext )
	{
		if( appStricmp( Name, Hash->Name )==0 )
		{
			// Found it in the hash.
			Index = Hash->Index;
			if( FindType==FNAME_Define )
			{
				// Define case of existing name.
				checkSlow( appStrlen(Name) == appStrlen(Hash->Name) );

				// Conflicting defines. Uppercase wins.
				if( appStrcmp(Name, Hash->Name) < 0 )
					appStrcpy( Hash->Name, Name );
			}
			return;
		}
	}

	// Didn't find name.
	if( FindType==FNAME_Find )
	{
		// Not found.
		Index = 0;
		return;
	}

	// Find an available entry in the name table.
	INT NewIndex;
	if( Available.Num() )
	{
		NewIndex = Available( Available.Num()-1 );
		Available.Remove( Available.Num()-1 );
	}
	else
	{
		NewIndex = Names.Add();
	}

	// Allocate and set the name.
	Index = NewIndex;
	Names(NewIndex) = NameHash[iHash] = AllocateNameEntry( Name, NewIndex, 0, NameHash[iHash] );
	if( FindType==FNAME_Intrinsic )
		Names(NewIndex)->Flags |= RF_Native;

	unguard;
}

/*-----------------------------------------------------------------------------
	FName subsystem.
-----------------------------------------------------------------------------*/

FNameEntry FName::HardcodedNames[] = 
{
	#define REGISTER_NAME(num,namestr) \
		{ 0, RF_Native, NULL, TEXT(#namestr) },
	#define REG_NAME_HIGH(num,namestr) \
		{ 0, RF_Native|RF_HighlightedName, NULL, TEXT(#namestr) },
	#include "UnNames.h"
};

//
// Initialize the name subsystem.
//
void FName::StaticInit()
{
	guard(FName::StaticInit);
	check(Initialized==0);
	check((ARRAY_COUNT(NameHash)&(ARRAY_COUNT(NameHash)-1)) == 0);

	// Init the name hash.
	{for( INT i=0; i<ARRAY_COUNT(FName::NameHash); i++ )
		NameHash[i] = NULL;}

	// Register all hardcoded names.
	{for( INT i=0; i<ARRAY_COUNT(HardcodedNames); i++ )
	{
		HardcodedNames[i].Index = i;
		checkSlow( appStrlen(HardcodedNames[i].Name) < NAME_SIZE-1 );
		Hardcode(&HardcodedNames[i]);
	}}

	// Verify no duplicate names.
	{for( INT i=0; i<ARRAY_COUNT(NameHash); i++ )
		for( FNameEntry* Hash=NameHash[i]; Hash; Hash=Hash->HashNext )
			for( FNameEntry* Other=Hash->HashNext; Other; Other=Other->HashNext )
				if( appStricmp(Hash->Name,Other->Name)==0 )
					appErrorf( TEXT("Name '%s' was duplicated"), Hash->Name );}

	Initialized = 1;
	debugf( NAME_Init, TEXT("Name subsystem initialized") );
	unguard;
}

//
// Shut down the name subsystem.
//
void FName::StaticExit()
{
	guard(FName::StaticExit);
	check(Initialized);

	// Kill all names.
	for( INT i=ARRAY_COUNT(HardcodedNames); i<Names.Num(); i++ )
		if( Names(i) )
			delete Names(i);

	// Empty tables.
	Names.Empty();
	Available.Empty();
	Initialized = 0;

	debugf( NAME_Exit, TEXT("Name subsystem shut down") );
	unguard;
}

//
// Display the contents of the global name hash.
//
void FName::DisplayHash( FOutputDevice& Ar )
{
	guard(FName::DisplayHash);

	INT UsedBins=0, NameCount=0;
	for( INT i=0; i<ARRAY_COUNT(NameHash); i++ )
	{
		if( NameHash[i] != NULL ) UsedBins++;
		for( FNameEntry *Hash = NameHash[i]; Hash; Hash=Hash->HashNext )
			NameCount++;
	}
	Ar.Logf( TEXT("Hash: %i names, %i/%i hash bins"), NameCount, UsedBins, ARRAY_COUNT(NameHash) );

	unguard;
}

//
// Delete an name permanently; called by garbage collector.
//
void FName::DeleteEntry( INT i )
{
	guard(FName::DeleteEntry);

	// Unhash it.
	FNameEntry* NameEntry = Names(i);
	check(NameEntry);
	check(!(NameEntry->Flags & RF_Native));
	INT iHash = appStrihash(NameEntry->Name) & (ARRAY_COUNT(NameHash)-1);
	FNameEntry** HashLink;
	for( HashLink=&NameHash[iHash]; *HashLink && *HashLink!=NameEntry; HashLink=&(*HashLink)->HashNext );
	if( !*HashLink )
		appErrorf( TEXT("Unhashed name '%s'"), NameEntry->Name );
	*HashLink = (*HashLink)->HashNext;

	// Delete it.
	delete NameEntry;
	Names(i) = NULL;
	Available.AddItem( i );

	unguard;
}

/*-----------------------------------------------------------------------------
	FNameEntry implementation.
-----------------------------------------------------------------------------*/

CORE_API FArchive& operator<<( FArchive& Ar, FNameEntry& E )
{
	guard(FNameEntry<<);
	if( Ar.Ver()<64 )
	{
		check(Ar.IsLoading());
		INT Count;
		for( Count=0; ; Count++ )
		{
			BYTE ACh;
			Ar << ACh;
			E.Name[Count] = FromAnsi(ACh);
			if( ACh == 0 )
				break;
		}
		E.Name[Count] = 0;
	}
	else if( Ar.IsLoading() )
	{
		FString Str;
		Ar << Str;
		appStrcpy( E.Name, *Str );
	}
	else
	{
		FString Str( E.Name );
		Ar << Str;
	}
	return Ar << E.Flags;
	unguard;
}
CORE_API FNameEntry* AllocateNameEntry( const TCHAR* Name, DWORD Index, DWORD Flags, FNameEntry* HashNext )
{
	guard(AllocateNameEntry);

	const SIZE_T NameLength = static_cast<SIZE_T>(appStrlen(Name));
	check( NameLength<NAME_SIZE );
	const SIZE_T EntrySize = sizeof(FNameEntry) - (static_cast<SIZE_T>(NAME_SIZE)-NameLength-1)*sizeof(TCHAR);
	FNameEntry* NameEntry = (FNameEntry*)appMalloc( appCheckedAllocSize(EntrySize), TEXT("NameEntry") );
	NameEntry->Index      = Index;
	NameEntry->Flags      = Flags;
	NameEntry->HashNext   = HashNext;
	appStrcpy( NameEntry->Name, Name );
	return NameEntry;

	unguard;
}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/

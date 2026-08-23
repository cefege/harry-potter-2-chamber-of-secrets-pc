/*=============================================================================
	UnName.h: Unreal global name types.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

/*----------------------------------------------------------------------------
	Definitions.
----------------------------------------------------------------------------*/

// Maximum size of name.
enum {NAME_SIZE	= 128};

// Name index.
typedef INT NAME_INDEX;

// Enumeration for finding name.
enum EFindName
{
	FNAME_Find,			// Find a name; return 0 if it doesn't exist.
	FNAME_Add,			// Find a name or add it if it doesn't exist.
	FNAME_Intrinsic,	// Find a name or add it intrinsically if it doesn't exist.
	FNAME_Define,		// Find or add the name; define case.
};

/*----------------------------------------------------------------------------
	FNameEntry.
----------------------------------------------------------------------------*/

//
// A global name, as stored in the global name table.
//
#pragma pack(push,4)
struct FNameEntry
{
	// Variables.
	NAME_INDEX	Index;				// Index of name in hash.
	DWORD		Flags;				// RF_TagImp, RF_TagExp, RF_Native.
	FNameEntry*	HashNext;			// Pointer to the next entry in this hash bin's linked list.

	// The name string.
	TCHAR		Name[NAME_SIZE];	// Name, variable-sized.

	// Functions.
	CORE_API friend FArchive& operator<<( FArchive& Ar, FNameEntry& E );
	CORE_API friend FNameEntry* AllocateNameEntry( const TCHAR* Name, DWORD Index, DWORD Flags, FNameEntry* HashNext );
};
#pragma pack(pop)
template <> struct TTypeInfo<FNameEntry*> : public TTypeInfoBase<FNameEntry*>
{
	static UBOOL NeedsDestructor() {return 0;}
};

/*----------------------------------------------------------------------------
	FName.
----------------------------------------------------------------------------*/

//
// Public name, available to the world. Names are stored as 32-bit indices
// into the name table and every name in Unreal is stored once
// and only once in that table.  Names are case-insensitive.
//
#define checkName checkSlow
class CORE_API FName 
{
public:
	// Accessors.
	const TCHAR* operator*() const
	{
		return GetEntry()->Name;
	}
	NAME_INDEX GetIndex() const
	{
		return Index;
	}
	DWORD GetFlags() const
	{
		return GetEntry()->Flags;
	}
	void SetFlags( DWORD Set ) const
	{
		GetEntry()->Flags |= Set;
	}
	void ClearFlags( DWORD Clear ) const
	{
		GetEntry()->Flags &= ~Clear;
	}
	UBOOL operator==( const FName& Other ) const
	{
		return Index==Other.Index;
	}
	UBOOL operator!=( const FName& Other ) const
	{
		return Index!=Other.Index;
	}
	UBOOL IsValid() const
	{
		return Index==0 || (Index>0 && Index<Names.Num() && Names(Index)!=NULL);
	}

	// Constructors.
	FName( enum EName N )
	: Index( static_cast<NAME_INDEX>(N) )
	{
		checkSlow(IsValid());
	}
	FName()
	{}
	FName( const TCHAR* Name, EFindName FindType=FNAME_Add );

	// Name subsystem.
	static void StaticInit();
	static void StaticExit();
	static void DeleteEntry( int i );
	static void DisplayHash( class FOutputDevice& Ar );
	static void Hardcode( FNameEntry* AutoName );

	// Name subsystem accessors.
	static const TCHAR* SafeString( EName Index )
	{
		return Initialized ? Names(Index)->Name : TEXT("Uninitialized");
	}
	static UBOOL SafeSuppressed( EName Index )
	{
		return Initialized && (Names(Index)->Flags & 0x00001000);
	}
	static int GetMaxNames()
	{
		return Names.Num();
	}
	static FNameEntry* GetEntry( int i )
	{
		return Names(i);
	}
	static UBOOL GetInitialized()
	{
		return Initialized;
	}

private:
	NAME_INDEX Index;

	FNameEntry* GetEntry() const
	{
		if( Index==0 )
			return &HardcodedNames[0];
		checkSlow( IsValid() );
		return Names(Index);
	}

	// Static subsystem variables.
	static FNameEntry			HardcodedNames[];// Pre-allocated name entries.
	static TArray<FNameEntry*>	Names;			 // Table of all names.
	static TArray<INT>          Available;       // Indices of available names.
	static FNameEntry*			NameHash[4096];  // Hashed names.
	static UBOOL				Initialized;	 // Subsystem initialized.
};
static_assert(sizeof(FName) == sizeof(NAME_INDEX), "FName must retain the legacy 32-bit script ABI");
inline DWORD GetTypeHash( const FName N )
{
	return N.GetIndex();
}

/*----------------------------------------------------------------------------
	The End.
----------------------------------------------------------------------------*/

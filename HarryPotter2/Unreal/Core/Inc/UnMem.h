/*=============================================================================
	UnMem.h: FMemStack class, ultra-fast temporary memory allocation
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

/*-----------------------------------------------------------------------------
	Globals.
-----------------------------------------------------------------------------*/

// Enums for specifying memory allocation type.
enum EMemZeroed {MEM_Zeroed=1};
enum EMemOned   {MEM_Oned  =1};

class FMemStack;
inline void* operator new( SIZE_T Size, FMemStack& Mem, INT Count=1, INT Align=DEFAULT_ALIGNMENT );
inline void* operator new( SIZE_T Size, FMemStack& Mem, EMemZeroed Tag, INT Count=1, INT Align=DEFAULT_ALIGNMENT );
inline void* operator new( SIZE_T Size, FMemStack& Mem, EMemOned Tag, INT Count=1, INT Align=DEFAULT_ALIGNMENT );

/*-----------------------------------------------------------------------------
	FMemStack.
-----------------------------------------------------------------------------*/

//
// Simple linear-allocation memory stack.
// Items are allocated via PushBytes() or the specialized operator new()s.
// Items are freed en masse by using FMemMark to Pop() them.
//
class CORE_API FMemStack
{
public:
	// Get bytes.
	inline BYTE* PushBytes( INT AllocSize, INT Alignment )
	{
		// Debug checks.
		guardSlow(FMemStack::PushBytes);

		#if __PSX2_EE__
		Alignment = Max(Alignment,16);
		#endif

		checkSlow(AllocSize>=0);
		checkSlow(Alignment>0 && (Alignment&(Alignment-1))==0);
		checkSlow((Top==NULL)==(End==NULL));

		BYTE* Result = NULL;
		if( Top )
		{
			checkSlow( reinterpret_cast<UPTRINT>(Top)<=reinterpret_cast<UPTRINT>(End) );
			BYTE* Candidate = Align( Top, Alignment );
			const UPTRINT CandidateAddress = reinterpret_cast<UPTRINT>(Candidate);
			const UPTRINT EndAddress = reinterpret_cast<UPTRINT>(End);
			if( CandidateAddress<=EndAddress && static_cast<UPTRINT>(AllocSize)<=EndAddress-CandidateAddress )
				Result = Candidate;
		}

		if( !Result )
		{
			// The current chunk cannot satisfy the request, so allocate a new one.
			AllocateNewChunk( appCheckedIntSize(static_cast<SIZE_T>(AllocSize)+static_cast<SIZE_T>(Alignment)) );
			Result = Align( Top, Alignment );
			checkSlow( reinterpret_cast<UPTRINT>(Result)<=reinterpret_cast<UPTRINT>(End) );
			checkSlow( static_cast<UPTRINT>(AllocSize)<=reinterpret_cast<UPTRINT>(End)-reinterpret_cast<UPTRINT>(Result) );
		}
		Top = Result + AllocSize;
		return Result;
		unguardSlow;
	}

	// Main functions.
	void Init( INT DefaultChunkSize, const TCHAR* InTag );
	void Exit();
	void Tick();
	INT GetByteCount();

	// Friends.
	friend class FMemMark;
	friend void* operator new( SIZE_T Size, FMemStack& Mem, INT Count, INT Align );
	friend void* operator new( SIZE_T Size, FMemStack& Mem, EMemZeroed Tag, INT Count, INT Align );
	friend void* operator new( SIZE_T Size, FMemStack& Mem, EMemOned Tag, INT Count, INT Align );

	// Types.
	struct FTaggedMemory
	{
		FTaggedMemory* Next;
		INT DataSize;
		BYTE Data[1];
	};

private:
	// Constants.
	enum {MAX_CHUNKS=1024};

	// Variables.
	BYTE*			Top;				// Top of current chunk (Top<=End).
	BYTE*			End;				// End of current chunk.
	INT				DefaultChunkSize;	// Maximum chunk size to allocate.
	FTaggedMemory*	TopChunk;			// Only chunks 0..ActiveChunks-1 are valid.
	const TCHAR*	Tag;				// For mem logging.

	// Static.
	static FTaggedMemory* UnusedChunks;

	// Functions.
	BYTE* AllocateNewChunk( INT MinSize );
	void FreeChunks( FTaggedMemory* NewTopChunk );
};

/*-----------------------------------------------------------------------------
	FMemStack templates.
-----------------------------------------------------------------------------*/

// Operator new for typesafe memory stack allocation.
template <class T> inline T* New( FMemStack& Mem, INT Count=1, INT Align=DEFAULT_ALIGNMENT )
{
	guardSlow(FMemStack::New);
	return (T*)Mem.PushBytes( appCheckedIntSize(static_cast<SIZE_T>(Count)*sizeof(T)), Align );
	unguardSlow;
}
template <class T> inline T* NewZeroed( FMemStack& Mem, INT Count=1, INT Align=DEFAULT_ALIGNMENT )
{
	guardSlow(FMemStack::New);
	const INT ByteCount = appCheckedIntSize( static_cast<SIZE_T>(Count)*sizeof(T) );
	BYTE* Result = Mem.PushBytes( ByteCount, Align );
	appMemzero( Result, ByteCount );
	return (T*)Result;
	unguardSlow;
}
template <class T> inline T* NewOned( FMemStack& Mem, INT Count=1, INT Align=DEFAULT_ALIGNMENT )
{
	guardSlow(FMemStack::New);
	const INT ByteCount = appCheckedIntSize( static_cast<SIZE_T>(Count)*sizeof(T) );
	BYTE* Result = Mem.PushBytes( ByteCount, Align );
	appMemset( Result, 0xff, ByteCount );
	return (T*)Result;
	unguardSlow;
}

/*-----------------------------------------------------------------------------
	FMemStack operator new's.
-----------------------------------------------------------------------------*/

// Operator new for typesafe memory stack allocation.
inline void* operator new( SIZE_T Size, FMemStack& Mem, INT Count, INT Align )
{
	// Get uninitialized memory.
	guardSlow(FMemStack::New1);
	return Mem.PushBytes( appCheckedIntSize(Size*static_cast<SIZE_T>(Count)), Align );
	unguardSlow;
}
inline void* operator new( SIZE_T Size, FMemStack& Mem, EMemZeroed Tag, INT Count, INT Align )
{
	// Get zero-filled memory.
	guardSlow(FMemStack::New2);
	const INT ByteCount = appCheckedIntSize( Size*static_cast<SIZE_T>(Count) );
	BYTE* Result = Mem.PushBytes( ByteCount, Align );
	appMemzero( Result, ByteCount );
	return Result;
	unguardSlow;
}
inline void* operator new( SIZE_T Size, FMemStack& Mem, EMemOned Tag, INT Count, INT Align )
{
	// Get one-filled memory.
	guardSlow(FMemStack::New3);
	const INT ByteCount = appCheckedIntSize( Size*static_cast<SIZE_T>(Count) );
	BYTE* Result = Mem.PushBytes( ByteCount, Align );
	appMemset( Result, 0xff, ByteCount );
	return Result;
	unguardSlow;
}

/*-----------------------------------------------------------------------------
	FMemMark.
-----------------------------------------------------------------------------*/

//
// FMemMark marks a top-of-stack position in the memory stack.
// When the marker is constructed or initialized with a particular memory 
// stack, it saves the stack's current position. When marker is popped, it
// pops all items that were added to the stack subsequent to initialization.
//
class CORE_API FMemMark
{
public:
	// Constructors.
	FMemMark()
	{}
	FMemMark( FMemStack& InMem )
	{
		guardSlow(FMemMark::FMemMark);
		Mem          = &InMem;
		Top          = Mem->Top;
		SavedChunk   = Mem->TopChunk;
		unguardSlow;
	}

	// FMemMark interface.
	void Pop()
	{
		// Check state.
		guardSlow(FMemMark::Pop);

		// Unlock any new chunks that were allocated.
		if( SavedChunk != Mem->TopChunk )
			Mem->FreeChunks( SavedChunk );

		// Restore the memory stack's state.
		Mem->Top = Top;
		unguardSlow;
	}

private:
	// Implementation variables.
	FMemStack* Mem;
	BYTE* Top;
	FMemStack::FTaggedMemory* SavedChunk;
};

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/

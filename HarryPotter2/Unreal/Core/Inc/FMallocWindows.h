/*=============================================================================
	FMallocWindows.h: Windows optimized memory allocator.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
		* Greatly revised by Jasper.
		  No longer uses per-size pools. Less memory waste, just as fast.

	To do:
		* Combine adjacent freed blocks. Probably not necessary, as small blocks
		  are quickly re-used. Overall inefficiency is very small.
		* VirtualFree depleted pools. Probably not necessary, as that would require 
		  the above change in order to be simple, and isn't warranted in a VM system. 
		  The pools remain available for further allocations.

=============================================================================*/

#if 0
#include "FMallocWindows0.h"
#else

#include "UnStat.h"

inline float Seconds( FILETIME ft )
{
	return reinterpret_cast<__int64&>(ft) / 1e7f;
}

//
// Optimized Windows virtual memory allocator.
//
class FMallocWindows : public FMalloc
{
private:
	// Counts.
	enum {
		ALLOC_ALIGN		= 4,
		ALLOC_MIN		= 8,
		POOL_SIZE		= 0x10000,
		FREELIST_MAX	= POOL_SIZE/ALLOC_ALIGN
	};

	// Types.
	struct FFreeBlock
	{
		DWORD		Size: 31,
					Used: 1;
		union
		{
			FFreeBlock*	Next;	// If free, next in FreeList.
			BYTE*		Mem;	// 
		};

		inline FFreeBlock* Following()
		// The one after this.
		{
			return (FFreeBlock*)( (BYTE*)&Mem + Size );
		}
	};

	struct FPoolInfoBase
	{
		BYTE*       Mem;		// Memory base.
		DWORD		AllocBytes;	// How much allocated.
		DWORD		Bytes;		// How much used mem.
		INT			Blocks;		// How many used blocks (if pooled).
	};
	typedef TDoubleLinkedList<FPoolInfoBase> FPoolInfo;

	// Variables.
	FPoolInfo*	PoolIndirect[256];
	FFreeBlock* FreeList[FREELIST_MAX];
	INT         MemInit;
	INT			OsCurrent,OsPeak,UsedCurrent,UsedPeak,PoolCurrent,PoolPeak,CurrentAllocs,TotalAllocs;

	// Implementation.
	void OutOfMemory()
	{
		appErrorf( LocalizeError("OutOfMemory",TEXT("Core")) );
	}
	FPoolInfo* CreateIndirect()
	{
		FPoolInfo* Indirect = (FPoolInfo*)VirtualAlloc( NULL, 256*sizeof(FPoolInfo), MEM_COMMIT, PAGE_READWRITE );
		if( !Indirect )
			OutOfMemory();
		return Indirect;
	}
	FPoolInfo* GetIndirect( void* Ptr )
	{
		return &PoolIndirect[(DWORD)Ptr>>24][((DWORD)Ptr>>16)&255];
	}

	inline FFreeBlock* FreeBlock( void* Ptr )
	{
		return (FFreeBlock*)( (DWORD*)Ptr - 1 );
	}
	inline FFreeBlock* GetFreeBlock( DWORD Size )
	{
		for( DWORD Index = Size/ALLOC_ALIGN; Index < FREELIST_MAX; Index++ )
			if( FreeList[Index] )
			{
				// Return and unlink this.
				FFreeBlock* Free = FreeList[Index];
				FreeList[Index] = Free->Next;
				Free->Used = 1;
				return Free;
			}
		return NULL;
	}
	inline void AddFreeBlock( FFreeBlock* Free )
	{
		// Insert block in free list.
		INT Index = Free->Size/ALLOC_ALIGN;
		checkSlow(Index < FREELIST_MAX);
		Free->Used = 0;
		Free->Next = FreeList[Index];
		FreeList[Index] = Free;
	}
	void RemoveFreeBlock( FFreeBlock* Free )
	{
		// Remove it from list.
		INT Index = Free->Size/ALLOC_ALIGN;
		for( FFreeBlock** List = &FreeList[Index]; *List; List = &(*List)->Next )
			if( *List == Free )
			{
				*List = Free->Next;
				return;
			}
		checkSlow( NULL );
	}

	void ShrinkBlock( FFreeBlock* Free, DWORD NewSize )
	{
		INT Remaining = Free->Size - NewSize - sizeof(DWORD);
		if( Remaining >= ALLOC_MIN )
		{
			// Split up the free block.
			Free->Size = NewSize;
			FFreeBlock* Avail = Free->Following();
			Avail->Size = Remaining;
			AddFreeBlock( Avail );
		}
	}

public:
	// FMalloc interface.
	FMallocWindows()
	:	MemInit( 0 )
	,	OsCurrent		( 0 )
	,	OsPeak			( 0 )
	,	UsedCurrent		( 0 )
	,	UsedPeak		( 0 )
	,	CurrentAllocs	( 0 )
	,	TotalAllocs		( 0 )
	{
		appMemzero( FreeList, sizeof(FreeList) );
		appMemzero( PoolIndirect, sizeof(PoolIndirect) );
	}
	void* Malloc( DWORD Size, const TCHAR* Tag )
	{
		guard(FMallocWindows::Malloc);
		checkSlow(Size>0);
		checkSlow(MemInit);
		Clock(GStat.MemTime);
		STAT(CurrentAllocs++);
		STAT(TotalAllocs++);

		// Round size up to alignment.
		Size = Max<DWORD>( Align(Size,ALLOC_ALIGN), ALLOC_MIN );
		STAT(UsedPeak = Max(UsedPeak,UsedCurrent+=Size));

		DWORD PageSize = Align( Size, GPageSize );
		if( Size < POOL_SIZE && PageSize!=Size )
		{
			// Try to alloc from existing pool.
			FFreeBlock* Free = GetFreeBlock(Size);
			if( Free )
			{
				ShrinkBlock( Free, Size );

				// Maintain stats.
				FPoolInfo* Pool = GetIndirect(Free);
				Pool->Blocks ++;
				Pool->Bytes += Free->Size;
				checkSlow( Pool->Bytes <= Pool->AllocBytes );
				return &Free->Mem;
			}
		}

		// If large, or if perfectly aligned, allocate an OS page.
		FFreeBlock* Free = (FFreeBlock*)VirtualAlloc( NULL, PageSize, MEM_COMMIT, PAGE_READWRITE );
		if( !Free )
			OutOfMemory();
		STAT(OsPeak = Max(OsPeak, OsCurrent+=PageSize));
		STAT(PoolPeak = Max(PoolPeak, ++PoolCurrent));

		// Create indirect.
		FPoolInfo*& Indirect = PoolIndirect[((DWORD)Free>>24)];
		if( !Indirect )
			Indirect = CreateIndirect();
		FPoolInfo* Pool = &Indirect[((DWORD)Free>>16)&255];

		// Init pool.
		Pool->Mem			= (BYTE*)Free;
		Pool->AllocBytes	= PageSize;
		Pool->Bytes			= Size;

		if( Size < POOL_SIZE && PageSize!=Size )
		{
			// Divvy this pool up.
			Pool->Blocks = 1;
			Free->Size = Size;

			// Link remaining space into free list.
			FFreeBlock* Avail = Free->Following();
			INT Remaining = Pool->Mem + PageSize - (BYTE*)&Avail->Mem;
			if( Remaining >= ALLOC_MIN )
			{
				Avail->Size = Remaining;
				AddFreeBlock( Avail );
			}

			return &Free->Mem;
		}
		else
		{
			// Return as is.
			Pool->Blocks = 0;
			return Free;
		}
		unguard;
	}
	void Free( void* Ptr )
	{
		guard(FMallocWindows::Free);
		if( !Ptr )
			return;
		checkSlow(MemInit);
		Clock(GStat.MemTime);
		STAT(CurrentAllocs--);

		// Windows version.
		FPoolInfo* Pool = GetIndirect(Ptr);
		checkSlow(Pool->AllocBytes!=0);
		if( Pool->Blocks > 0 )
		{
			// Insert block in free list.
			FFreeBlock* Free = FreeBlock(Ptr);
			STAT(UsedCurrent -= Free->Size);
			Pool->Blocks--;
			Pool->Bytes -= Free->Size;
			checkSlow( Pool->Bytes <= Pool->AllocBytes );
			AddFreeBlock(Free);
		}
		else
		{
			// Free entire block.
			STAT(UsedCurrent -= Pool->Bytes);
			STAT(OsCurrent   -= Pool->AllocBytes);
			STAT(PoolCurrent--);
			verify( VirtualFree( Ptr, 0, MEM_RELEASE )!=0 );
		}
		unguard;
	}
	void* Realloc( void* Ptr, DWORD NewSize, const TCHAR* Tag )
	{
		guard(FMallocWindows::Realloc);
		checkSlow(MemInit);
		check(NewSize>=0);
		void* NewPtr = Ptr;
		if( Ptr && NewSize )
		{
			checkSlow(MemInit);
			DWORD OldSize;
			FPoolInfo* Pool = GetIndirect(Ptr);
			if( Pool->Blocks > 0 )
			{
				// Allocated from pool.
				FFreeBlock* Free = FreeBlock(Ptr);
				OldSize = Free->Size;

				if( NewSize < OldSize )
				{
					ShrinkBlock( Free, NewSize );
					Pool->Bytes -= OldSize - Free->Size;
					return Ptr;
				}
				else
				{
					// See whether this block can be expanded.
					FFreeBlock* Avail = Free->Following();
					if( (BYTE*)Avail < Pool->Mem + Pool->AllocBytes 
					&& !Avail->Used && OldSize + Avail->Size + sizeof(DWORD) >= NewSize )
					{
						// Expand this block.
						Free->Size += Avail->Size + sizeof(DWORD);
						RemoveFreeBlock(Avail);

						// Shrink it as necessary.
						ShrinkBlock( Free, NewSize );
						Pool->Bytes += Free->Size - OldSize;
						checkSlow( Pool->Bytes <= Pool->AllocBytes );
						return Ptr;
					}
				}
			}
			else
				// Single allocation from OS.
				OldSize = Pool->AllocBytes;

			if( NewSize > OldSize || NewSize < OldSize*3/4 )
			{
				NewPtr = FMallocWindows::Malloc( NewSize, Tag );
				appMemcpy( NewPtr, Ptr, Min(NewSize,OldSize) );
				FMallocWindows::Free( Ptr );
			}
		}
		else if( NewSize )
		{
			NewPtr = FMallocWindows::Malloc( NewSize, Tag );
		}
		else
		{
			if( Ptr )
				FMallocWindows::Free( Ptr );
			NewPtr = NULL;
		}
		return NewPtr;
		unguardf(( TEXT("%08X %i %s"), (INT)Ptr, NewSize, Tag ));
	}
	INT MemSize( void* Ptr )
	{
		guard(FMallocWindows::MemSize);
		checkSlow(MemInit);
		Clock(GStat.MemTime);
		if( !Ptr )
			return 0;

		FPoolInfo* Pool = GetIndirect(Ptr);
		if( Pool->Blocks > 0 )
		{
			// Allocated from pool. Return block size,
			// and average in wasted space.
			checkSlow( Pool->Bytes <= Pool->AllocBytes );
			FFreeBlock* Free = FreeBlock(Ptr);
			DWORD Waste = (Pool->AllocBytes - Pool->Bytes) / Pool->Blocks;
			return Free->Size + Waste;
		}
		else
		{
			// Single allocation from OS.
			return Pool->AllocBytes;
		}
		unguard;
	}
	void DumpAllocs()
	{
		guard(FMallocWindows::DumpAllocs);
		FMallocWindows::HeapCheck();

		INT Size = 0, USize = 0;
		HANDLE Heaps[1024];
		INT NHeaps = GetProcessHeaps(1024, Heaps);
		PROCESS_HEAP_ENTRY HE;
		for_count( h, NHeaps )
		{
			HE.lpData = NULL;
			HE.wFlags = 0;
			while( HeapWalk( Heaps[h], &HE ) )
			{
				if( HE.wFlags & PROCESS_HEAP_REGION )
				{
					Size += HE.Region.dwCommittedSize;
					USize += HE.Region.dwUnCommittedSize;
				}
			}
		}

		FLOAT Megs = 1.f/1024.f/1024.f;
		STAT(debugf( TEXT("Memory Allocation Status") ));
		STAT(debugf( TEXT("Curr Memory % .3f MB Use / % .3f MB Sys (%d Pools)"),
			UsedCurrent*Megs, OsCurrent*Megs, PoolCurrent ));
		STAT(debugf( TEXT("Peak Memory % .3f MB Use / % .3f MB Sys (%d Pools)"),
			UsedPeak   *Megs, OsPeak   *Megs, PoolPeak ));
		STAT(debugf( TEXT("Allocs      % 6i Current / % 6i Total"), CurrentAllocs, TotalAllocs ));
		STAT(debugf( TEXT("Heap Memory % .3f MB + %.3f MB U"), Size*Megs, USize*Megs ));

		FILETIME Creation, Exit, Kernel, User;
		if( GetThreadTimes( GetCurrentThread(), &Creation, &Exit, &Kernel, &User ) )
		{
			debugf( TEXT("Thread User=%f, Kernel=%f, App=%f"), Seconds(User), Seconds(Kernel), appSeconds().GetFloat() );
		}

#if 0 && STATS
		if( ParseParam(appCmdLine(), TEXT("MEMSTAT")) )
		{
			debugf( TEXT("Block Size Num Pools Cur Allocs Total Allocs Mem Used Mem Waste Efficiency") );
			debugf( TEXT("---------- --------- ---------- ------------ -------- --------- ----------") );
			INT TotalPoolCount  = 0;
			INT TotalAllocCount = 0;
			INT TotalMemUsed    = 0;
			INT TotalMemWaste   = 0;
			for( INT i=0; i<POOL_COUNT; i++ )
			{
				FPoolTable* Table = &PoolTable[i];
				INT PoolCount=0;
				INT AllocCount=0;
				INT MemTotal=0;
				for( INT i=0; i<2; i++ )
				{
					for( FPoolInfo* Pool=(i?Table->FirstPool:Table->ExaustedPool); Pool; Pool=Pool->Next )
					{
						PoolCount++;
						AllocCount += Pool->Taken;
						MemTotal += Pool->AllocBytes;
						INT FreeCount=0;
						for( FFreeMem* Free=Pool->FirstMem; Free; Free=Free->Next )
							FreeCount += Free->Blocks;
					}
				}
				INT MemUsed = AllocCount*Table->BlockSize;
				INT MemWaste = MemTotal - MemUsed;
				debugf
				(
					TEXT("% 10i % 9i % 10i % 11iK % 7iK % 8iK % 9.2f%%"),
					Table->BlockSize,
					PoolCount,
					AllocCount,
					0,
					MemUsed /1024,
					MemWaste/1024,
					MemUsed ? 100.0 * MemUsed / (MemUsed+MemWaste) : 100.0
				);
				TotalPoolCount  += PoolCount;
				TotalAllocCount += AllocCount;
				TotalMemUsed    += MemUsed;
				TotalMemWaste   += MemWaste;
			}
			debugf
			(
				TEXT("BlkOverall % 9i % 10i % 11iK % 7iK % 8iK % 9.2f%%"),
				TotalPoolCount,
				TotalAllocCount,
				0,
				TotalMemUsed /1024,
				TotalMemWaste/1024,
				TotalMemUsed ? 100.0 * TotalMemUsed / (TotalMemUsed+TotalMemWaste) : 100.0
			);
		}
#endif
		unguard;
	}
	void HeapCheck()
	{
		guard(FMallocWindows::HeapCheck);
		/*
		for( INT i=0; i<POOL_COUNT; i++ )
		{
			FPoolTable* Table = &PoolTable[i];
			for( FPoolInfo** PoolPtr=&Table->FirstPool; *PoolPtr; PoolPtr=&(*PoolPtr)->Next )
			{
				FPoolInfo* Pool=*PoolPtr;
				check(Pool->PrevLink==PoolPtr);
				check(Pool->FirstMem);
				for( FFreeMem* Free=Pool->FirstMem; Free; Free=Free->Next )
					check(Free->Blocks>0);
			}
			for( PoolPtr=&Table->ExaustedPool; *PoolPtr; PoolPtr=&(*PoolPtr)->Next )
			{
				FPoolInfo* Pool=*PoolPtr;
				check(Pool->PrevLink==PoolPtr);
				check(!Pool->FirstMem);
			}
		}
		*/
		unguard;
	}
	void Init()
	{
		guard(FMallocWindows::Init);
		check(!MemInit);
		MemInit = 1;

		// Get OS page size.
		SYSTEM_INFO SI;
		GetSystemInfo( &SI );
		GPageSize = SI.dwPageSize;
		check(!(GPageSize&(GPageSize-1)));

		unguard;
	}
	void Exit()
	{}
};

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/

#endif
/*=============================================================================
	TMallocDebug.h: Debug memory allocator.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

// Tags.
enum {MEM_PreTag =0xf0ed1cee};
enum {MEM_PostTag=0xdeadf00f};
enum {MEM_Tag    =0xfe      };
enum {MEM_WipeTag=0xcd      };

// Debug memory allocator.
template<class M>
class TMallocDebug : public M
{
private:
	// Structure for memory debugging.
	struct FMemDebugBase
	{
#ifdef TAG
		TCHAR*		Tag;
#endif
		SIZE_T		Size;
		INT			RefCount;
		INT			PreTag;
	};
	typedef TDoubleLinkedList<FMemDebugBase> FMemDebug;

	// Variables.
	FMemDebug* GFirstDebug;

public:
	// FMalloc interface.
	TMallocDebug()
	:	GFirstDebug	( NULL )
	{}
	void* Malloc( DWORD Size, const TCHAR* Tag )
	{
		guard(TMallocDebug::Malloc);
		check((INT)Size>0);
		FMemDebug* Ptr = NULL;
		guard(CallMalloc);
			Ptr = (FMemDebug*)M::Malloc( sizeof(FMemDebug) + Size + sizeof(INT), Tag );
			check(Ptr);
		unguard;
		guard(SetPtr);
			Ptr->RefCount = 1;
			Ptr->Size     = Size;
			Ptr->Next     = GFirstDebug;
			Ptr->PrevLink = &GFirstDebug;
			Ptr->PreTag   = MEM_PreTag;
		unguard;
#ifdef TAG
		guard(DupTag);
			Ptr->Tag = (TCHAR*)M::Malloc( (appStrlen(Tag)+1)*sizeof(TCHAR), TEXT("Tag") );
			appStrcpy( Ptr->Tag, Tag );
		unguard;
#endif
		guard(SetPost);
			*(INT*)((BYTE*)Ptr+sizeof(FMemDebug)+Size) = MEM_PostTag;
		unguard;
		guard(FillMem);
			appMemset( Ptr+1, MEM_Tag, Size );
		unguard;
		guard(DoPrevLink);
			if( GFirstDebug )
			{
				check(GIsCriticalError||GFirstDebug->PrevLink==&GFirstDebug);
				GFirstDebug->PrevLink = &Ptr->Next;
			}
			GFirstDebug = Ptr;
		unguard;
		return Ptr+1;
		unguardf(( TEXT("%i %s"), Size, Tag ));
	}
	void* Realloc( void* InPtr, DWORD NewSize, const TCHAR* Tag )
	{
		guard(TMallocDebug::Realloc);
		if( InPtr && NewSize )
		{
			check(GIsCriticalError||((FMemDebug*)InPtr-1)->RefCount==1);
			check(GIsCriticalError||((FMemDebug*)InPtr-1)->Size>0);
			void* Result = appMalloc( NewSize, Tag );
			appMemcpy( Result, InPtr, Min(((FMemDebug*)InPtr-1)->Size,NewSize) );
			appFree( InPtr );
			return Result;
		}
		else if( NewSize )
		{
			return appMalloc( NewSize, Tag );
		}
		else
		{
			if( InPtr )
				appFree( InPtr );
			return NULL;
		}
		unguardf(( TEXT("%08X %i %s"), (INT)InPtr, NewSize, Tag ));
	}
	void Free( void* InPtr )
	{
		guard(TMallocDebug::Free);
		if( !InPtr )
			return;

		FMemDebug* Ptr = (FMemDebug*)InPtr - 1;
		check(GIsCriticalError||Ptr->Size>0);
		check(GIsCriticalError||Ptr->RefCount==1);
		check(GIsCriticalError||Ptr->PreTag==MEM_PreTag);
		check(GIsCriticalError||*(INT*)((BYTE*)InPtr+Ptr->Size)==MEM_PostTag);
		appMemset( InPtr, MEM_WipeTag, Ptr->Size );
		Ptr->Size = 0;
		Ptr->RefCount = 0;

		check(GIsCriticalError||Ptr->PrevLink);
		check(GIsCriticalError||*Ptr->PrevLink==Ptr);
		*Ptr->PrevLink = Ptr->Next;
		if( Ptr->Next )
			Ptr->Next->PrevLink = Ptr->PrevLink;

#ifdef TAG
		M::Free( Ptr->Tag );
#endif
		M::Free( Ptr );

		unguard;
	}
	void DumpAllocs()
	{
		guard(TMallocDebug::DumpAllocs);
		INT Count=0;
		INT Chunks=0;
		for( FMemDebug* Ptr=GFirstDebug; Ptr; Ptr=Ptr->Next )
		{
#ifdef TAG
			TCHAR Temp[256];
			appStrncpy( Temp, (TCHAR*)(Ptr+1), Min((SIZE_T)255,Ptr->Size) );
			debugf( TEXT("   % 10i %s <%s>"), Ptr->Size, Ptr->Tag, Temp );
#endif
			Count += Ptr->Size;
			Chunks++;
		}
		debugf( TEXT("%.3f MB, %i Chunks still allocated"), Count/1024.f/1024.f, Chunks );
		M::DumpAllocs();
		unguard;
	}
	void HeapCheck()
	{
		guard(TMallocDebug::HeapCheck);
		for( FMemDebug** Link = &GFirstDebug; *Link; Link=&(*Link)->Next )
			check(GIsCriticalError||*(*Link)->PrevLink==*Link);
		M::HeapCheck();
		unguard;
	}
};

// Logging memory allocator.

template<class M>
class TMallocLog : public M
{
private:

	// Structure for summarising stats.
	struct FGroupStat
	{
		FName	Name;
		INT		Chunks, Size, AllocSize;
		INT		DupLevel;

		// Tree structure.
		FGroupStat* Next;
		FGroupStat* Children;
		FGroupStat* Parent;

		FGroupStat( FName InName = NAME_None )
		:	Name(InName), Chunks(0), Size(0), AllocSize(0)
		,	Next(0), Children(0), Parent(0), DupLevel(0)
		{}

		FGroupStat* SubGroup( FName Tag )
		{
			if( Tag == Name )
			{
				DupLevel++;
				return this;
			}
			for( FGroupStat* C = Children; C; C = C->Next )
			{
				if( C->Name == Tag )
					return C;
			}
			
			// Add it to front of list.
			C = new FGroupStat(Tag);
			C->Parent = this;
			C->Next = Children;
			Children = C;
			return C;
		}

		void Zero()
		{
			// Conglomerate child stats.
			AllocSize = Size = Chunks = 0;
			for( FGroupStat* C = Children; C; C = C->Next )
				C->Zero();
		}

		void Sum()
		{
			// Conglomerate child stats.
			for( FGroupStat* C = Children; C; C = C->Next )
			{
				C->Sum();
				AllocSize += C->AllocSize;
				Size += C->Size;
				Chunks += C->Chunks;
			}
		}

		void Log( INT Level=0 )
		{
			const int MIN_STAT_SIZE = 0x2000;

			debugf( TEXT("%.*s %6.3f MB alloc, %6.3f MB wasted, %5i allocs:  %s"), 
				Level, TEXT("++++++++"), AllocSize/1024.f/1024.f, (AllocSize-Size)/1024.f/1024.f, Chunks, *Name );

			// Sort and log children.
			if( Children )
			{
				INT Num = 0;
				for( FGroupStat* C = Children; C; C = C->Next )
					Num++;
				FGroupStat* Array = (FGroupStat*) _alloca( Num * sizeof(FGroupStat) );
				INT i = 0;
				for( C = Children; C; C = C->Next )
					Array[i++] = *C;
				Sort( Array, Num );
				FGroupStat Misc(TEXT("Misc"));
				for( i=0; i<Num; i++ )
				{
					if( Array[i].Name == NAME_None )
					{
						// Do not log None if it's the only contributor.
						if( i==0 && (Num==1 || Array[1].AllocSize < MIN_STAT_SIZE) )
							continue;
						Array[i].Name = Name;
					}
					if( Array[i].AllocSize >= MIN_STAT_SIZE )
					{
						// Significant contributor.
						Array[i].Log(Level+1);
					}
					else
					{
						Misc.AllocSize += Array[i].AllocSize;
						Misc.Size += Array[i].Size;
						Misc.Chunks += Array[i].Chunks;
					}
				}
				if( Misc.AllocSize >= MIN_STAT_SIZE )
					Misc.Log(Level+1);
			}
		}
	};

	friend inline INT Compare( const FGroupStat& A, const FGroupStat& B )
	{
		return B.Size - A.Size;
	}

	// Structure for memory logging by category.
	struct FMemLog
	{
		void*		Mem;
		SIZE_T		Size;
		FGroupStat*	Group;
		FName		Tag;
	};

	// Variables.
	TArray<FMemLog> MemLogList;
	FGroupStat Total, Overhead;
	FGroupStat* CurGroup;
	bool bRecurse, bMakeName;
	INT LastLogPos;					// Removal optimisation.

public:
	// FMalloc interface.
	TMallocLog()
	:	Total		( NAME_None )
	,	Overhead	( NAME_None )
	,	CurGroup	( &Total )
	,	bRecurse	( false )
	,	bMakeName	( false )
	,	LastLogPos	( 0 )
	{}

	void SetTag( const TCHAR* Tag )
	{
		Clock(GStat.StatMemTime);
		guard(TMallocLog::SetTag);
		if( Tag != NULL )
		{
			// Pushing. Make name Intrinsic, so it isn't deleted.
			FGroupStat* SaveGroup = CurGroup;
			CurGroup = &Overhead;
			CurGroup = SaveGroup->SubGroup( FName(Tag, FNAME_Intrinsic) );
		}
		else
		{
			// Popping.
			if( CurGroup->DupLevel > 0 )
				CurGroup->DupLevel--;
			else if( CurGroup->Parent )
				CurGroup = CurGroup->Parent;
		}
		unguard;
	}

	virtual const TCHAR* GetTag() 
	{ 
		return *CurGroup->Name;
	}

	void* Malloc( DWORD Size, FName Tag )
	{
		guard(TMallocLog::Malloc);
		void* Mem = M::Malloc( Size, *Tag );
		if( !bRecurse )
		{
			// Log it.
			bRecurse = true;
			MemLogList.Add();
			FMemLog* Ptr = &MemLogList.Last();
			Ptr->Mem      = Mem;
			Ptr->Size     = Size;
			Ptr->Tag      = Tag;
			Ptr->Group    = CurGroup;
			bRecurse = false;
		}
		return Mem;
		unguard;
	}
	void* Malloc( DWORD Size, const TCHAR* Tag )
	{
		Clock(GStat.StatMemTime);
		FName Name = NAME_None;
		if( *Tag && !bMakeName && FName::GetInitialized() )
		{
			// Use argument.
			bMakeName = true;
			Name = FName(Tag, FNAME_Intrinsic);
			bMakeName = false;
		}
		return Malloc( Size, Name );
	}
	void* Realloc( void* InPtr, DWORD NewSize, const TCHAR* Tag )
	{
		guard(TMallocLog::Realloc);
		if( InPtr && NewSize )
		{
			Clock(GStat.StatMemTime);
			void* Mem = M::Realloc( InPtr, NewSize, Tag );

			// Adjust existing record.
			if( !bRecurse )
			{
				FMemLog* Log = FindLog(InPtr);
				Log->Mem = Mem;
				Log->Size = NewSize;
			}
			return Mem;
		}
		else if( NewSize )
		{
			return Malloc( NewSize, Tag );
		}
		else
		{
			if( InPtr )
				Free( InPtr );
			return NULL;
		}
		unguard;
	}
	void Free( void* InPtr )
	{
		Clock(GStat.StatMemTime);
		guard(TMallocLog::Free);
		if( InPtr )
		{
			// Remove record.
			RemoveLog( InPtr );
			M::Free( InPtr );
		}
		unguard;
	}
	FMemLog* FindLog( void* Mem  )
	{
		for( FMemLog* Ptr = &MemLogList.Last(); Ptr >= MemLogList.GetData(); Ptr-- )
		{
			if( Ptr->Mem == Mem )
				return Ptr;
		}
		checkSlow(NULL);
		return NULL;
	}
	void RemoveLog( void* Mem )
	{
		if( MemLogList.Num() == 0 )
			return;
		LastLogPos -= MemLogList.Num()/256;
		LastLogPos = Min( Max(LastLogPos, 0), MemLogList.Num()-1 );
		for( INT i=LastLogPos; i<MemLogList.Num(); i++ )
			if( MemLogList(i).Mem == Mem )
				break;
		if( i == MemLogList.Num() )
		{
			for( i=0; i<LastLogPos; i++ )
				if( MemLogList(i).Mem == Mem )
					break;
			if( i == LastLogPos )
				// Not found.
				if( Mem != MemLogList.GetData() )
				{
					checkSlow(0);
				}
		}

		// Count contiguous NULL elements, back and forth.
		MemLogList(i).Mem = NULL;
		for( INT s=i; s>0; s-- )
			if( MemLogList(s-1).Mem != NULL )
				break;
		for( INT n=i+1; n<MemLogList.Num(); n++ )
			if( MemLogList(n).Mem != NULL )
				break;
		if( n == MemLogList.Num() )
			// All beyond i are NULL.
			MemLogList.Set(s);
		else if( n-s >= 8 )
		{
			// Remove a chunk.
			appMemcpy
			(
				&MemLogList(s), &MemLogList(n),
				(MemLogList.Num()-n) * sizeof(MemLogList(0))
			);
			MemLogList.Set( MemLogList.Num()-(n-s) );
		}
		LastLogPos = i;
	}
	void DumpAllocs()
	{
		guard(TMallocLog::DumpAllocs);

		if( FName::GetInitialized() )
		{
			// Summarise tags into groups.
			FGroupStat* Save = CurGroup;
			CurGroup = &Overhead;

			Overhead.Name = TEXT("Overhead");
			Overhead.Zero();

			Total.Name = TEXT("Total");
			Total.Zero();

			for_array( i, MemLogList )
			{
				FMemLog* Ptr = &MemLogList(i);
				if( !Ptr->Mem )
					continue;
				FGroupStat* Stat = Ptr->Group;

				// Create subgroup here.
				if( Ptr->Tag != NAME_None && Ptr->Tag != Stat->Name )
					Stat = Stat->SubGroup(Ptr->Tag);
				else
					Stat = Stat->SubGroup(NAME_None);
				Stat->Size += Ptr->Size;
				Stat->AllocSize += M::MemSize( Ptr->Mem );
				Stat->Chunks++;
			}

			// Display overhead.
			Overhead.Chunks++;
			Overhead.Size += MemLogList.Mem();
			Overhead.AllocSize += MemLogList.Mem();
			Overhead.Sum();
			Overhead.Log();

			// Display actual allocs.
			Total.Sum();
			Total.Log();

			CurGroup = Save;
		}

		M::DumpAllocs();
		unguard;
	}
	void HeapCheck()
	{
		guard(TMallocLog::HeapCheck);
		M::HeapCheck();
		unguard;
	}
};

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/

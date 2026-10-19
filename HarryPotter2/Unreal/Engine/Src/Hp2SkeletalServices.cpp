/*=============================================================================
	Hp2SkeletalServices.cpp: GCache, GMath and host hooks for the Rust
	USkeletalMesh and UAnimation port (crates/hp2-engine/src/skeletal.rs).
	The debug hooks hold the DisplayBones and DisplayInfluences bodies that
	GetFrame ran before the port.
=============================================================================*/

#include "EnginePrivate.h"
#include "UnRender.h"
#include "UnSkeletalMesh.h"
#include "Hp2SkeletalServices.h"

static QWORD SkelCacheId( const void* Owner )
{
	return MakeCacheID( CID_SkeletalData, (const AActor*)Owner, NULL );
}

static uint8_t* SkelCacheGet( const void* Owner, void** Item )
{
	FCacheItem* CacheItem = NULL;
	BYTE* Mem = GCache.Get( SkelCacheId(Owner), CacheItem );
	*Item = CacheItem;
	return Mem;
}

static uint8_t* SkelCacheCreate( const void* Owner, size_t Len, void** Item )
{
	FCacheItem* CacheItem = NULL;
	BYTE* Mem = GCache.Create( SkelCacheId(Owner), CacheItem, appCheckedIntSize(Len) );
	*Item = CacheItem;
	return Mem;
}

static void SkelCacheFlush( const void* Owner )
{
	GCache.Flush( SkelCacheId(Owner) );
}

static void SkelCacheUnlock( void* Item )
{
	// Rust passes null for an item that was never locked.
	if( Item )
		((FCacheItem*)Item)->Unlock();
}

static float SkelSinTab( int32_t I )
{
	return GMath.SinTab( I );
}

static float SkelCosTab( int32_t I )
{
	return GMath.CosTab( I );
}

static void SkelWarnMissingLink( const void* Mesh, int32_t Bone )
{
	USkeletalMesh* M = (USkeletalMesh*)Mesh;
	debugf( TEXT("Warning: missing bone/animation link for %s  index %i"), *M->RefSkeleton(Bone).Name, M->RefSkeleton(Bone).Name.GetIndex() );
}

static void SkelRemoveAux( void* Owner, int32_t Index )
{
	AActor* AnimOwner = (AActor*)Owner;
	AnimOwner->GetLevel()->DestroyActor( AnimOwner->AuxAnims(Index) );
	AnimOwner->AuxAnims.Remove( Index );
}

static void SkelDebugPivots( void* Mesh, const void* Bases, int32_t Num )
{
	USkeletalMesh* M = (USkeletalMesh*)Mesh;
	const FCoords* SpaceBases = (const FCoords*)Bases;
	M->DebugPivots.Empty();
	M->DebugPivots.Add( Num );
	M->DebugParents.Empty();
	M->DebugParents.Add( Num );
	for( INT s=0; s<Num; s++ )
	{
		M->DebugParents(s) = M->RefSkeleton(s).ParentIndex;
		M->DebugPivots(s) = SpaceBases[s].Origin;
	}
}

static void SkelDebugInfluences( void* Mesh )
{
	USkeletalMesh* M = (USkeletalMesh*)Mesh;
	M->DebugVerts.Add( M->FrameVerts );
	TArray <INT> VertInfluences;
	VertInfluences.AddZeroed( M->FrameVerts );
	for( INT w=0; w< M->BoneWeightIdx.Num(); w++)
	{
		INT Index  = M->BoneWeightIdx(w).WeightIndex;
		INT Number = M->BoneWeightIdx(w).Number;
		if ( Number > 0)
		{
			for( INT b=Index; b<(Index+Number); b++ )
			{
				INT VertIndex = M->BoneWeights(b).PointIndex;
				if (VertIndex < M->FrameVerts )   // LOD check.
				{
					if ( (FLOAT)M->BoneWeights(b).BoneWeight != 0 )
					VertInfluences(VertIndex) +=1;
				}
			}
		}
	}
	// Fill the debugging vertex influence-counts ( colored vertices in wireframe debugging mode )
	for( INT v=0; v<M->FrameVerts; v++)
	{
		M->DebugVerts(v) = Min(VertInfluences(v),255);
	}
	VertInfluences.Empty();
}

static void SkelFailCheck( const char* Expr )
{
	appFailAssert( Expr, __FILE__, __LINE__ );
}

const Hp2SkeletalServices GHp2SkeletalServices =
{
	SkelSinTab,
	SkelCosTab,
	SkelCacheGet,
	SkelCacheCreate,
	SkelCacheFlush,
	SkelCacheUnlock,
	SkelWarnMissingLink,
	SkelRemoveAux,
	SkelDebugPivots,
	SkelDebugInfluences,
	SkelFailCheck,
};

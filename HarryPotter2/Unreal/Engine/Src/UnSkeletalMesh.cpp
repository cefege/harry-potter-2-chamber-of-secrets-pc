/*=============================================================================
	UnSkeletalMesh.cpp: Unreal mesh animation functions
	Copyright 2000 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Erik de Neve
		 
	    * Remarks
		  - New methods: the explicit ctor, and GetFrame()
		  - Distinguishing Mesh from SkeletalMesh : if( Mesh->IsA(USkeletalMesh::StaticClass()) )
		  - No specific amd3d support.

	To do:
		Skip dummy bones

=============================================================================*/ 

#include "EnginePrivate.h"
#include "UnRender.h"
#include "UnSkeletalMesh.h"

/*-----------------------------------------------------------------------------
	FSkelCache and TCacheItem helper classes.
-----------------------------------------------------------------------------*/

//
// Cached frame header struct for semi-permanently cached meshes:
// Stores current Skeleton <-> Animation(s) Linkup data for this skeletal mesh,
// and all other animation/ current LOD/ etc data for drawing this actor that 
// doesn't explicitly need to be a part of AActor, ie doesn't need replication.
// => Nuisance: Caching dynamic arrays and such => storage needs to be explicitly *in* the cache....

struct USkeletalMesh::CFSkelHeader
{
	UMesh	*CachedMesh;
	UAnimation *CachedAnim;
	FLOAT	CachedFrame;
	FLOAT	CachedAlpha;
	FName	CachedSeq;
	UBOOL   CacheLoaded;
	INT*	CachedLinks;
	FPlace*	CachedPlaces;
	FCoords	MeshCoords;
	FBox	PrevBound;
	FVector PrevRoot;
	FLOAT	PrevRootFrame;
	FVector Movement;
	FVector MovementSurplus;
	bool	bGotRoot;
};

template<class T>
class TCacheItem
{
	T* Mem;
	FMemCache::FCacheItem* CacheItem;

public:
	inline TCacheItem( QWORD CacheID )
	{
		CacheItem = NULL;
		Mem = (T*)GCache.Get( CacheID, CacheItem );
	}

	inline ~TCacheItem()
	{
		if( CacheItem )
			CacheItem->Unlock();
	}

	inline operator bool() { return Mem != NULL; }
	inline T* operator->() { return Mem; }
};

class FSkelCache: public TCacheItem<USkeletalMesh::CFSkelHeader>
{
public:
	inline FSkelCache( const AActor* Owner )
	:	TCacheItem<USkeletalMesh::CFSkelHeader>( MakeCacheID( CID_SkeletalData, Owner, NULL ) )
	{}
};

/*-----------------------------------------------------------------------------
	FAnimVec implementation.
-----------------------------------------------------------------------------*/

void FAnimVec::Init( const FVector& V, float InvScale )
{
	// Scale, and convert to ints.
#ifdef ANIM48
	InvScale *= BASE_SCALE;
	checkSlow( appRound(V.MaxVal()*InvScale) <= BASE_SCALE );
#else
	InvScale *= BASE_SCALE;
	Shift = 0;
	FLOAT MaxVal = V.MaxVal();
	checkSlow( appRound(MaxVal*InvScale) <= BASE_SCALE );
	while( MaxVal*InvScale < 0.5f && Shift <= 2 )
	{
		Shift++;
		InvScale *= 2.f;
	}
#endif
	X = appRound(V.X * InvScale);
	Y = appRound(V.Y * InvScale);
	Z = appRound(V.Z * InvScale);
}

#include <math.h>

FAnimVec::FAnimVec( const FQuat& Q )
{
	FVector V = Q.Vector();

#ifdef ANIM48
	// Convert sin components to angles.
	static float Scale = BASE_SCALE / 1.57079633;
	X = appRound( asin( Min(V.X,1.f) ) * Scale );
	Y = appRound( asin( Min(V.Y,1.f) ) * Scale );
	Z = appRound( asin( Min(V.Z,1.f) ) * Scale );
#else
	FVector S( asin( Min(V.X,1.f) ),
			   asin( Min(V.Y,1.f) ),
			   asin( Min(V.Z,1.f) ) );
	Init( S, 1.f / 1.57079633 );
#endif
}

inline FLOAT AnimVecSin( INT A )
{
	// Use sin tab and interpolation.
	INT T = A>>1;
	FLOAT Interp = (T&3)*0.25f + (A&1)*0.125f;
	return GMath.SinTab(T) * (1.f-Interp) + GMath.SinTab(T+4) * Interp;
}

FQuat FAnimVec::Quat() const
{
	// Take sin of components.
#ifdef ANIM48
	static float Scale = 1.57079633 / BASE_SCALE;
#else
	float Scale = 1.57079633 / (BASE_SCALE<<Shift);
#endif
	return FQuat( sin(X * Scale), sin(Y * Scale), sin(Z * Scale) );
}

/*-----------------------------------------------------------------------------
	UAnimation object implementation.
-----------------------------------------------------------------------------*/

UAnimation::UAnimation()
{
	guard(UnAnimation::UnAnimation);
	//Initialisations.
	CompFactor = 1.0f;
	
	unguardobj;
}

void UAnimation::Serialize( FArchive& Ar )
{
	guard(UAnimation::Serialize);

	Super::Serialize(Ar);

	Ar << RefBones;	
	Ar << Moves;
	Ar << AnimSeqs;
	Ar << MasterTrack;

	if( Ar.IsLoading() )
	{
		// Fix up track pointers.
		INT Q = 0, P = 0, T = 0;
		for_array( m, Moves )
			for_array( a, Moves[m].AnimTracks )
			{
				FAnalogTrack& Track = Moves[m].AnimTracks[a];
				Track.KeyQuat.Set( &MasterTrack.KeyQuat[Q], Track.KeyQuat.Num() );	Q += Track.KeyQuat.Num();
				Track.KeyPos.Set ( &MasterTrack.KeyPos[P],  Track.KeyPos.Num() );	P += Track.KeyPos.Num();
				Track.KeyDelta.Set( &MasterTrack.KeyDelta[T], Track.KeyDelta.Num() );	T += Track.KeyDelta.Num();
			}
	}

	unguardobj;
}
IMPLEMENT_CLASS(UAnimation);


/*-----------------------------------------------------------------------------
	USkeletalMesh object implementation.
-----------------------------------------------------------------------------*/

USkeletalMesh::USkeletalMesh()
{
	guard(USkeletalMesh::UskeletalMesh);
	WeaponAdjust = GMath.UnitCoords;
	unguard;
}

void USkeletalMesh::Serialize( FArchive& Ar )
{
	guard(USkeletalMesh::Serialize);

	// Empty those structures not needed for LOD mesh rendering.
	// Vertlinks and Connects already empty for LOD meshes...
	// [E] Here we need to drop the TEXTURE LOD backward-compatibility

	if( Ar.IsSaving() )
	{
		Tris.Empty();
	}

	// Serialize parent's data.
	Super::Serialize(Ar);

	// Serialize the additional SkeletalMesh data.

	// Mesh specific data - fully floating point UV and vertex vectors.
	Ar << ExtWedges;
	Ar << Points;

	// Skeletal specific data
	Ar << RefSkeleton;     // Reference skeleton
	Ar << BoneWeightIdx;   //  ,, ,, contains Index, number of influences per bone (+ N detail level sizers! ..)
	Ar << BoneWeights;     //  Each element contians weight and vertex number.
	Ar << LocalPoints;     //  Each weighted point in local bone-space
	Ar << SkeletalDepth;   //
	Ar << DefaultAnimation;//
	Ar << WeaponBoneIndex; //  Weapon bone index assigned by #exec in script
	Ar << WeaponAdjust;    //

	if( Ar.IsLoading() && BoundingBoxes.Num() > 0 )
	{
		// Produce average animating box.
		BoundingBoxSum = FBox(0);
		for_array( i, BoundingBoxes )
		{
			BoundingBoxSum.Min += BoundingBoxes(i).Min;
			BoundingBoxSum.Max += BoundingBoxes(i).Max;
		}
		BoundingBoxSum.Min /= BoundingBoxes.Num();
		BoundingBoxSum.Max /= BoundingBoxes.Num();
	}

	unguardobj;
}
IMPLEMENT_CLASS(USkeletalMesh);


void USkeletalMesh::SetScale( FVector NewScale )
{
	guard(USkeletalMesh::SetScale);
	Scale = NewScale;
	// Maximum mesh scaling dimension for LOD gauging. Somewhat arbitrary.
	MeshScaleMax = ( 5.0f * 1.0f / 128.0f ) * BoundingSphere.W * Max(Abs(Scale.X), Max(Abs(Scale.Y), Abs(Scale.Z)));
	unguardobj;
}


void USkeletalMesh::FlipFaces()
{
	guard(USkeletalMesh::FlipFaces);

	// Unreal peculiarity: in general the Y coordinate is always flipped.
	// Physique flipping ('bones') doesn't always correctly flip the face normals - so do it here specifically if needed.	
	for(INT i=0; i<Faces.Num(); i++)
	{
		// Change handedness on faces.
		INT Wedge0 = Faces(i).iWedge[0];
		Faces(i).iWedge[0]=Faces(i).iWedge[1];
		Faces(i).iWedge[1]=Wedge0;
	}
	debugf(TEXT("Flipping all faces for model %s"),GetName());
	unguardobj;
}

FVector USkeletalMesh::MeshAdjust( const AActor* Owner ) const
{
	// Adjust to align bottoms of mesh and collision cylinder.
	if( Owner->bAlignBottom && Owner->bCollideWorld && Owner->Physics != PHYS_None && Owner->CollideType != CT_Shape )
		return FVector( 0, 0, -Owner->CollisionHeight - 2.5 
			+ (Origin.Z - BoundingBox.Min.Z) * Scale.Z * Owner->DrawScale );
	else
		return FVector(0);
}

FCoords USkeletalMesh::GetMeshCoords( const AActor* Owner ) const
{
	FScale SkeletalScale = FScale(Scale * Owner->DrawScale, 0.0f, SHEER_None);
	FLOAT Wideness = Owner->Wideness/128.0f;
	SkeletalScale.Scale.X *= Wideness;
	SkeletalScale.Scale.Y *= -Wideness;		// Unreal coord peculiarity.
	return FCoords(-Origin) << SkeletalScale << RotOrigin
		<< (Owner->PrePivot + MeshAdjust(Owner) + FVector( 0, 0, Owner->SavedPrePivotZ) ) << Owner->Rotation << Owner->Location;
}

// Bounding box helper function.
static FBox AnimBox( const AActor* Owner )
{
	FBox Bound(0);

	// Get bounding box for current animation.
	if( Owner->SkelAnim && Owner->AnimSequence != NAME_None )
	{
		// Ideally, if there is an anim, but it's not found, or the anim isn't the mesh default,
		// the static BB should be grown. 
		//checkSlow( Owner->Mesh->BoundingBoxes.Num() == Owner->SkelAnim->AnimSeqs.Num() );
		for_array( s, Owner->SkelAnim->AnimSeqs )
			if( Owner->AnimSequence == Owner->SkelAnim->AnimSeqs(s).Name )
			{
				Bound = Owner->Mesh->BoundingBoxes(s);
				break;
			}
	}

	// Combine with previous box.
	FSkelCache FrameHdr( Owner );
	if( FrameHdr )
	{
		if( Owner->TweenAlpha != 1.f )
			Bound += FrameHdr->PrevBound;
		FrameHdr->PrevBound = Bound;
	}

	for_array( a, Owner->AuxAnims )
		// Combine with any aux anims.
		Bound += AnimBox( Owner->AuxAnims(a) );

	return Bound;
}

FCoords USkeletalMesh::GetRenderBoundingBox( const AActor* Owner, UBOOL Exact )
{
	guard(USkeletalMesh::GetRenderBoundingBox);

	// Get bounding box for current anim(s).
	ApplyAnim( const_cast<AActor*>(Owner), NULL, true );
	FSkelCache FrameHdr( Owner );

	FBox Bound = AnimBox( Owner );
	if( !Bound.IsValid )
		// Not animating; use static bounding box.
		return BoundingBox << FrameHdr->MeshCoords;

	// Transform box by root joint.
	FCoords Coords;
	if( FrameHdr )
		Coords = FCoords(FrameHdr->CachedPlaces[0]) << FrameHdr->MeshCoords;
	else
		Coords = FCoords(RefSkeleton[0].BonePos.Place) << FrameHdr->MeshCoords;
	return Bound << Coords;
	unguardobj;
}

FBox USkeletalMesh::GetCollisionBoundingBox( const AActor* Owner, bool bWorld ) const
{
	if( bWorld )
		return FBox( BoundingBox, GetMeshCoords(Owner) );
	else
	{
		// Transform only by scale and origin.
		FVector BoxScale = Scale*Owner->DrawScale;
		return FBox( BoxScale*(BoundingBox.Min - Origin), BoxScale*(BoundingBox.Max - Origin) );
	}
}

inline FPlace Interp( const FPlace& P1, FPlace& P2, float Alpha )
{
	FPlace P;
	P.Quat = SlerpQuat( P1.Quat, P2.Quat, Alpha );
	P.Pos = P1.Pos * (1.f-Alpha) + P2.Pos * Alpha;
	return P;
}

/*-----------------------------------------------------------------------------
	USkeletalMesh animation interface.
-----------------------------------------------------------------------------*/

//
// Use simple 'named' animations just like PlayAnim; 
// expand to enable multiple animations to play in weighted, merged style.
//
// * currently not very optimized * 
//
// Traverse influences, stop whenever the point index goes over the requested vertex
// number; accumulate the transformed points, using the weights.
// Total: BONE * matrix-to-matrix transform; world trafo is only ONE in there;
// We end up with [Bonenum] + [total influences] transformations.
// it used to be: [total vertex] transformations.
//
// To do: single assignment linkup per animation does not work for partial-bones animations.

//
// All possible scenarios: (analogous to vertex tweening:)
//  - Playing a regular animation
//  - Tween into an animation.
//  - Tween into an animation without cached positions -> assumes animation start pose and waits out the tween.
//  - Tween into or play a nonexistent animation: freezes to cached positions.
//  - Tween but no animation, no cached positions-> freeze in reference pose.
//

inline FPlace GetPlace( const MotionChunk* ThisMove, INT t, FLOAT FrameTime )
{
	const FAnalogTrack& Track = ThisMove->AnimTracks(t);
	
	// Keytracks can have any number of keys; we need to find the two keys surrounding
	// the "animnum+alpha" time.
	if( Track.KeyDelta.Num() == 1)
	{
		return FPlace( Track.KeyQuat(0).Quat(), Track.GetKeyPos(0) );
	}
	else
	{
		INT KeyA=-1, KeyB=-1;
		FLOAT KeyATime = 0.f, KeyBTime = 0.f;

		// Find the right time into our animation.
		// FrameTime should be no larger than Track.TrackTime
		// if overruns the last key, wrap if necessary.

		KeyATime = 0.f;
		for( KeyB=1; KeyB<Track.KeyDelta.Num(); KeyB++)
		{
			KeyBTime += Track.KeyDelta(KeyB)*Track.TimeScale;
			if( KeyBTime > FrameTime )
				break;
			KeyATime = KeyBTime;
		}
		KeyA = KeyB-1;
		if( KeyATime == FrameTime )
			KeyB = KeyA;
		else if( KeyB >= Track.KeyDelta.Num() )
		{
			// Wrap.
			KeyB = 0;
			KeyBTime = ThisMove->TrackTime;
			KeyA = Track.KeyDelta.Num()-1;
		}

		FPlace ThisPlace;
		if( KeyA == KeyB )
		{
			ThisPlace.Quat = Track.KeyQuat(KeyA).Quat();
			if( Track.KeyPos.Num() == 1 )
				ThisPlace.Pos = Track.GetKeyPos(0);
			else
				ThisPlace.Pos = Track.GetKeyPos(KeyA);
		}
		else
		{
			// Construct Alpha
			FLOAT KeyAlpha = (FrameTime - KeyATime) / (KeyBTime - KeyATime);

			// Slerp the orientations.
			FQuat Quat1 = Track.KeyQuat(KeyA).Quat();
			FQuat Quat2 = Track.KeyQuat(KeyB).Quat();
			ThisPlace.Quat = SlerpQuat( Quat1, Quat2, KeyAlpha ); 

			// Lerp the positions.
			if( Track.KeyPos.Num() == 1 )
				ThisPlace.Pos = Track.GetKeyPos(0);
			else
			{
				FVector ThisPos1 = Track.GetKeyPos(KeyA);
				FVector ThisPos2 = Track.GetKeyPos(KeyB);
				ThisPlace.Pos = Interp( ThisPos1, ThisPos2, KeyAlpha );
			}
		}
		return ThisPlace;
	}
}

void USkeletalMesh::ApplyAnim
(
	AActor*			AnimOwner,
	CFSkelHeader*	ParentHdr,
	bool			bRootOnly
) const
{
	bool bAuxAnim = AnimOwner->AnimBone > 0 && AnimOwner->Physics != PHYS_Trailer;

	// Create or get cache memory.
	FMemCache::FCacheItem* Item = NULL;
	QWORD CacheID    = MakeCacheID( CID_SkeletalData, AnimOwner, NULL );
	BYTE* Mem = GCache.Get( CacheID, Item );
	CFSkelHeader* FrameHdr = (CFSkelHeader*)Mem;

	if( Mem==NULL || (UMesh*)FrameHdr->CachedMesh != (UMesh*)this )
	{
		// Actor's mesh changed?
		if( Mem != NULL )
		{			
			Item->Unlock();
			GCache.Flush(CacheID);
		}
		SIZE_T CacheSpace = (sizeof(INT) + sizeof(FPlace)) * static_cast<SIZE_T>(RefSkeleton.Num()); // Reserve space for linkups and bonepositions
		Mem = GCache.Create( CacheID, Item, appCheckedIntSize(sizeof(CFSkelHeader) + CacheSpace));
		FrameHdr = (CFSkelHeader*)Mem;
		FrameHdr->CachedMesh = (UMesh*)this;
		FrameHdr->CachedLinks = (INT*)((BYTE*)Mem + sizeof(CFSkelHeader));
		FrameHdr->CachedPlaces = (FPlace*)( (BYTE*)FrameHdr->CachedLinks + RefSkeleton.Num()*sizeof(INT) );
		FrameHdr->CacheLoaded = false;
		FrameHdr->Movement = FrameHdr->PrevRoot = FrameHdr->MovementSurplus = FVector(0);
		FrameHdr->CachedAnim = NULL;
		FrameHdr->CachedSeq = NAME_None;
		FrameHdr->CachedFrame = -1.f;
		FrameHdr->CachedAlpha = -1.f;
		FrameHdr->PrevBound = FBox(0);
	}
	
	if( FrameHdr->CachedSeq != AnimOwner->AnimSequence && !bRootOnly )
	{
		FrameHdr->bGotRoot = false;
		FrameHdr->PrevRootFrame = 0.f;
	}

	// Cache composite transform, for other anim functions.
	FrameHdr->MeshCoords = GetMeshCoords(AnimOwner);

	if( bAuxAnim || FrameHdr->CachedSeq != AnimOwner->AnimSequence
	|| FrameHdr->CachedFrame != AnimOwner->AnimFrame 
	|| FrameHdr->CachedAlpha != AnimOwner->TweenAlpha )
	{
		// Anim needs updating.
		const FMeshAnimSeq* Seq = NULL;
		const MotionChunk* ThisMove = NULL;
  
		// If there is no dynamically assigned animation, see if one was associated with the mesh with #exec MESH DEFAULTANIM
		if( AnimOwner->SkelAnim == NULL && DefaultAnimation != NULL )
		{
			AnimOwner->SkelAnim = DefaultAnimation;
			//debugf(TEXT("Linked up Default Animation %s to skeletal mesh %s"),AnimOwner->SkelAnim->GetName(),this->GetName());
		}

		UAnimation* SkAnim = AnimOwner->SkelAnim;
		if ( SkAnim )
		{
			Seq = SkAnim->GetAnimSeq( AnimOwner->AnimSequence );	
			ThisMove = SkAnim->GetMovement( AnimOwner->AnimSequence );
		}

		if( !Seq && bAuxAnim )
		{
			// Non-playing aux channels do not reset bones.
			Item->Unlock();
			return;
		}

		guard(SkLinkup);
		// Particular requested animation may not exist; still we want to link up our bones.
		if (SkAnim && ThisMove )
		{
			// Match up all bones with a uAnim : in a non-dynamic array, inside the cached temp data chunk.
			if ( SkAnim != FrameHdr->CachedAnim )
			{		
				// Handle partial-skeleton animation.
				for( INT p=0; p< RefSkeleton.Num(); p++)			
				{
					FrameHdr->CachedLinks[p] = -1;

					// Skip non-animating bones.
					if( bAuxAnim )
					{
						if( p < AnimOwner->AnimBone || 
							p > AnimOwner->AnimBone + RefSkeleton(AnimOwner->AnimBone).NumChildren )
							continue;
					}

					for( INT b=0; b<ThisMove->AnimTracks.Num(); b++ )
					{				
						if( RefSkeleton(p).Name == SkAnim->RefBones(b).Name )
						{
							FrameHdr->CachedLinks[p] = b;
							//debugf(TEXT(" Linked up %s with %s  index %i "),*SkAnim->RefBones(b).Name,*RefSkeleton(p).Name, SkAnim->RefBones(b).Name.GetIndex() );
							break;
						}
					}
					if( FrameHdr->CachedLinks[p] == -1 )
					{
						debugf(TEXT("Warning: missing bone/animation link for %s  index %i"),*RefSkeleton(p).Name, RefSkeleton(p).Name.GetIndex() );
					}
				}
				FrameHdr->CachedAnim = SkAnim; 
			}
		}
		unguard;

		if( !ParentHdr )
			ParentHdr = FrameHdr;

		INT NumBones = bRootOnly ? 1 : RefSkeleton.Num();
		if( SkAnim == NULL || Seq == NULL ) 
		{
			// Revert to default pose.
			for( INT b=0; b<NumBones; b++ )
			{
				// Render the default pose.
				FrameHdr->CachedPlaces[b] = RefSkeleton(b).BonePos.Place;
				if( FrameHdr != ParentHdr )
					ParentHdr->CachedPlaces[b] = FrameHdr->CachedPlaces[b];
			}
			if( !bRootOnly )
				FrameHdr->CachedAnim = NULL;
		}
		else
		{
			// We have animation keys to slerp between, and possible tweening.
			if( ThisMove->AnimTracks.Num() )
			{
				// Calculate overall tweening alpha (fraction of current pose to blend).
				FLOAT FrameTime = Min(1.f, AnimOwner->AnimFrame) * ThisMove->TrackTime; 
				FLOAT Alpha = FrameHdr->CacheLoaded && AnimOwner->TweenRate ? 
					1.f - AnimOwner->TweenAlpha 
					: 0.0f;

				// We have some compressed animation keys.
				for( INT b=0; b<NumBones; b++ )
				{
					FPlace ThisPlace;

					// Revert to static reference skeleton for all unknown bone names.
					if( FrameHdr->CachedLinks[b] < 0 )
					{
						if( bAuxAnim )
							// For auxilliary anims, ignore non-animating bones.
							continue;

						// Otherwise, render the default pose.
						ThisPlace = RefSkeleton(b).BonePos.Place;
					}
					else
					{
						INT t = FrameHdr->CachedLinks[b]; 
						ThisPlace = GetPlace( ThisMove, t, FrameTime );
						if( b == 0 )
						{
							const FAnalogTrack& Track = ThisMove->AnimTracks(t);

							// Concatenate any parent tracks not represented in skeleton.
							while( SkAnim->RefBones(t).ParentIndex != t )
							{
								t = SkAnim->RefBones(t).ParentIndex;
								ThisPlace = ThisPlace << GetPlace( ThisMove, t, FrameTime );
							}
							if( AnimOwner->bAnimMove )
							{
								// Transfer any translation of the root to FrameHdr->Movement.
								if( !FrameHdr->bGotRoot )
								{
									// Start of sequence.
									FrameHdr->PrevRoot = Track.GetKeyPos(0);
									t = FrameHdr->CachedLinks[b]; 
									while( SkAnim->RefBones(t).ParentIndex != t )
									{
										t = SkAnim->RefBones(t).ParentIndex;
										FrameHdr->PrevRoot <<= GetPlace( ThisMove, t, 0.f );
									}
									FrameHdr->MovementSurplus = FVector(0.f);
									FrameHdr->bGotRoot = true;
								}

								FVector Delta = ThisPlace.Pos - FrameHdr->PrevRoot;
								FrameHdr->PrevRoot = ThisPlace.Pos;

								// Revert the bone pos to the default position.
								ThisPlace.Pos = RefSkeleton(b).BonePos.Place.Pos;

								FrameHdr->Movement += Delta;

								if( FrameHdr->CachedSeq == AnimOwner->AnimSequence && AnimOwner->AnimFrame != FrameHdr->PrevRootFrame )
								{
									// Add a portion of surplus in.
									if( !FrameHdr->MovementSurplus.IsZero() )
									{
										FVector MaxRate = Track.GetKeyPos(Track.KeyPos.Num()-1) - Track.GetKeyPos(0);
										MaxRate.X = Abs(MaxRate.X);  MaxRate.Y = Abs(MaxRate.Y);  MaxRate.Z = Abs(MaxRate.Z);
										float Amount = (AnimOwner->AnimFrame != 0.f ? AnimOwner->AnimFrame : 1.f) - FrameHdr->PrevRootFrame;
										MaxRate *= Amount;
										FVector Adjust = FrameHdr->MovementSurplus;
										Adjust.X = Clamp( Adjust.X, -MaxRate.X, MaxRate.X );
										Adjust.Y = Clamp( Adjust.Y, -MaxRate.Y, MaxRate.Y );
										Adjust.Z = Clamp( Adjust.Z, -MaxRate.Z, MaxRate.Z );
										
										FrameHdr->Movement += Adjust;
										FrameHdr->MovementSurplus -= Adjust;
									}
									FrameHdr->PrevRootFrame = AnimOwner->AnimFrame;
								}
							}
						}
					}

					if( Alpha != 0.f )
						// Tween from current pose to animated value.
						ThisPlace = Interp( ThisPlace, FrameHdr->CachedPlaces[b], Alpha ); 

					// Cache pose for future tweens.
					FrameHdr->CachedPlaces[b] = ThisPlace;
					if( FrameHdr != ParentHdr )
						ParentHdr->CachedPlaces[b] = FrameHdr->CachedPlaces[b];
				}
			}
		}
	}

	if( !bRootOnly )
	{
		// If whole skeleton updated, mark cache, and do aux channels.
		FrameHdr->CacheLoaded = true;
		FrameHdr->CachedSeq = AnimOwner->AnimSequence;
		FrameHdr->CachedFrame = AnimOwner->AnimFrame;
		FrameHdr->CachedAlpha = AnimOwner->TweenAlpha;

		// Apply any aux channels as well.
		for_array( a, AnimOwner->AuxAnims )
		{
			AActor* Chan = AnimOwner->AuxAnims(a);
			ApplyAnim( Chan, FrameHdr );

			// Cull finished anims.
			if( Chan->bAnimTransient && !Chan->bAnimLoop && Chan->AnimFrame >= Chan->AnimLast )
			{
				AnimOwner->GetLevel()->DestroyActor( Chan );
				AnimOwner->AuxAnims.Remove( a );
				a--;
			}
		}
	}
	Item->Unlock(); // unlock cache memory
}

int USkeletalMesh::BoneIndex( FName BoneName ) const
{
	for_array( p, RefSkeleton )
		if( RefSkeleton(p).Name == BoneName )
			return p;
	return -1;
}

FName USkeletalMesh::BoneName( int Index ) const
{
	if( Index >= 0 && Index < RefSkeleton.Num() )
		return RefSkeleton(Index).Name;
	return NAME_None;
}

void USkeletalMesh::GetFrame
(
	FVector*	ResultVerts,
	INT			Size,
	FCoords		Coords,
	AActor*		Owner,
	INT&		LODRequest  
)
{
	guard(USkeletalMesh::GetFrame);

	// Check to see if bAnimByOwner
	AActor*	AnimOwner = NULL;
	if ((Owner->bAnimByOwner) && (Owner->Owner != NULL))
		AnimOwner = Owner->Owner;
	else
		AnimOwner = Owner;
	
	// Determine how many vertices to compute. 
	INT VertexNum = Min(LODRequest, FrameVerts); 

	// Get stuff.	
	ApplyAnim( AnimOwner );

	// Get cache memory again.
	FSkelCache FrameHdr( AnimOwner );
	check( FrameHdr );
	FCoords MeshCoords = FrameHdr->MeshCoords >> Coords;

	// Build the space bases; PLUS add in our world transform at the root bone.
	TArray <FCoords> SpaceBases( RefSkeleton.Num() );
	SpaceBases(0) = FCoords( FrameHdr->CachedPlaces[0] ) << MeshCoords;
	for(INT s=1; s<SpaceBases.Num(); s++ )
	{
		INT Parent = RefSkeleton(s).ParentIndex;
		check(Parent != s);
		SpaceBases(s) = FCoords( FrameHdr->CachedPlaces[s] ) << SpaceBases(Parent);
	}

	// For debugging mode: prepare the wireframe bone line drawing data.
	if ( DisplayBones ) 
	{	
		DebugPivots.Empty();
		DebugPivots.Add(SpaceBases.Num());
		DebugParents.Empty();
		DebugParents.Add(SpaceBases.Num());
		for( INT s=0; s<SpaceBases.Num(); s++ )
		{
			DebugParents(s) = RefSkeleton(s).ParentIndex;
			DebugPivots(s) = SpaceBases(s).Origin;
		}	
	}

	// For debugging mode: fill the vertex-influence colors (needs done only once).
	if ( DisplayInfluences && ( DebugVerts.Num()==0 ) )
	{
		DebugVerts.Add(FrameVerts);
		TArray <INT> VertInfluences;
		VertInfluences.AddZeroed(FrameVerts);
		for( INT w=0; w< BoneWeightIdx.Num(); w++)
		{
			INT Index  = BoneWeightIdx(w).WeightIndex;
			INT Number = BoneWeightIdx(w).Number;
			if ( Number > 0)
			{
				for( INT b=Index; b<(Index+Number); b++ )
				{
					INT VertIndex = BoneWeights(b).PointIndex;
					if (VertIndex < FrameVerts )   // LOD check.
					{					
						if ( (FLOAT)BoneWeights(b).BoneWeight != 0 )
						VertInfluences(VertIndex) +=1;
					}				
				}
			}
		}
		// Fill the debugging vertex influence-counts ( colored vertices in wireframe debugging mode )
		for( INT v=0; v<FrameVerts; v++)
		{			
			DebugVerts(v) = Min(VertInfluences(v),255);			
		}
		VertInfluences.Empty();
	}
	
	// Final multi-bone vertex deformation.
	if( BoneWeightIdx.Num() == 1 )
	{
		// Single-bone skeleton.
		INT Number = Min( (INT)BoneWeightIdx(0).Number, VertexNum );
		for( INT b=0; b<Number; b++ )
		{
			checkSlow( BoneWeights(b).PointIndex == b );
			checkSlow( BoneWeights(b).BoneWeight == 0xFFFF );

			*ResultVerts = LocalPoints(b) << SpaceBases(0);
			*(BYTE**)&ResultVerts += Size;
		}
	}
	else
	{
		TArray <FVector> OutVerts;
	    OutVerts.AddZeroed(VertexNum);

		for( INT w=0; w<BoneWeightIdx.Num(); w++ )
		{
			INT Index  = BoneWeightIdx(w).WeightIndex;
			INT Number = BoneWeightIdx(w).Number;
			for( INT b=Index; b<(Index+Number); b++ )
			{
				INT VertIndex = BoneWeights(b).PointIndex;
				if( VertIndex >= VertexNum ) // LOD check
					break;
				FLOAT Weight = (FLOAT)BoneWeights(b).BoneWeight * ( 1.0f/65535.f );
				OutVerts(VertIndex) += Weight * (LocalPoints(b) << SpaceBases(w));
			}
		}

		// Copy into destination structures.
		for( INT v=0; v<OutVerts.Num(); v++)
		{
			*ResultVerts = OutVerts(v);
			*(BYTE**)&ResultVerts += Size;
		}
	}

	LODRequest = VertexNum; // Return number of verts.

	// Get the weapon bone, if it had been assigned.
	if( WeaponBoneIndex >= 0 )
	{
		// Generate worldspace weapon attach point and orientation.	
		//
		// note that wireframe has a different coordinate system when calling getFrame:
		// bWire ? GMath.UnitCoords : Coords
		// because wires are transformed&clipped late at rendertime.

		FCoords Orientation = WeaponAdjust << SpaceBases(WeaponBoneIndex); 

		Orientation.XAxis = Orientation.XAxis.SafeNormal();
		Orientation.YAxis = (Orientation.XAxis ^ Orientation.ZAxis).SafeNormal();
		Orientation.ZAxis = Orientation.XAxis ^ Orientation.YAxis;
		Orientation.YAxis = - Orientation.YAxis; //Account for Unreal's mirroring peculiarity.

		ClassicWeaponCoords = GMath.UnitCoords * Orientation; 
	}

	unguardobj;
}

FVector USkeletalMesh::GetRootMovement
(
	AActor* Owner
)
{
	// Ensure the animation is current.
	ApplyAnim( Owner, NULL, true );

	// Get cache memory.
	FSkelCache FrameHdr( Owner );
	if( FrameHdr )
	{
		// Transform to world coords (ignore offsets).
		FCoords VCoords = FrameHdr->MeshCoords;  VCoords.Origin = FVector(0);
		FVector Move = FrameHdr->Movement << VCoords;
		FrameHdr->Movement = FVector(0);
		return Move;
	}
	else
		return FVector(0);
}

void USkeletalMesh::AdjustRootMovement
(
	AActor* Owner,
	const FVector& Adjust
)
{
	// Get cache memory.
	if( !Adjust.IsZero() )
	{
		FSkelCache FrameHdr( Owner );
		if( FrameHdr )
			FrameHdr->MovementSurplus += Adjust.TransformVectorBy(FrameHdr->MeshCoords);
	}
}

FVector USkeletalMesh::AnimCycleMovement
(
	AActor*		Owner
)
{
	// Ensure the animation is current.
	ApplyAnim( Owner, NULL, true );

	// Get cache memory.
	FSkelCache FrameHdr( Owner );

	// Examine the animation movement.
	if( FrameHdr && FrameHdr->CachedLinks[0] >= 0 )
	{
		if( Owner->SkelAnim )
		{
			const MotionChunk* ThisMove = Owner->SkelAnim->GetMovement( Owner->AnimSequence );
			if( ThisMove && ThisMove->AnimTracks.Num() )
			{
				const FAnalogTrack& Track = ThisMove->AnimTracks( FrameHdr->CachedLinks[0] );
				return Track.GetKeyPos( Track.KeyPos.Num()-1 ) - Track.GetKeyPos(0);
			}
		}
	}

	return FVector(0);
}

FCoords USkeletalMesh::GetBoneCoords( AActor* Owner, int Bone ) const
{
	// Ensure the animation is current.
	ApplyAnim( Owner, NULL, Bone==0 );

	// Get cache memory.
	FSkelCache FrameHdr( Owner );
	if( FrameHdr )
	{
		if( Bone >= 0 && Bone < RefSkeleton.Num() )
		{
			FCoords Coords(FrameHdr->CachedPlaces[Bone]);
			while( Bone != 0 )
			{
				Bone = RefSkeleton(Bone).ParentIndex;
				checkSlow(Bone >= 0);
				Coords <<= FCoords(FrameHdr->CachedPlaces[Bone]);
			}
			return Coords << FrameHdr->MeshCoords;
		}
	}

	return GetMeshCoords(Owner);
}

/*-----------------------------------------------------------------------------
	The end.
-----------------------------------------------------------------------------*/

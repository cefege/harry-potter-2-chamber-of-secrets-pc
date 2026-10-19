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
#include "Hp2SkeletalServices.h"


/*-----------------------------------------------------------------------------
	FAnimVec implementation.
-----------------------------------------------------------------------------*/

void FAnimVec::Init( const FVector& V, float InvScale )
{
	hp2_rs_engine_skeletal_anim_vec_init( &V, InvScale, this );
}


FAnimVec::FAnimVec( const FQuat& Q )
{
	hp2_rs_engine_skeletal_anim_vec_from_quat( &Q, this );
}

FQuat FAnimVec::Quat() const
{
	FQuat Q;
	hp2_rs_engine_skeletal_anim_vec_quat( this, &Q );
	return Q;
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
	hp2_rs_engine_skeletal_set_scale( this, &NewScale );
	unguardobj;
}


void USkeletalMesh::FlipFaces()
{
	guard(USkeletalMesh::FlipFaces);
	hp2_rs_engine_skeletal_flip_faces( this );
	debugf(TEXT("Flipping all faces for model %s"),GetName());
	unguardobj;
}

FVector USkeletalMesh::MeshAdjust( const AActor* Owner ) const
{
	FVector Adjust;
	hp2_rs_engine_skeletal_mesh_adjust( this, Owner, &Adjust );
	return Adjust;
}

FCoords USkeletalMesh::GetMeshCoords( const AActor* Owner ) const
{
	FCoords Coords;
	hp2_rs_engine_skeletal_get_mesh_coords( &GHp2SkeletalServices, this, Owner, &Coords );
	return Coords;
}

FCoords USkeletalMesh::GetRenderBoundingBox( const AActor* Owner, UBOOL Exact )
{
	guard(USkeletalMesh::GetRenderBoundingBox);
	FCoords Coords;
	hp2_rs_engine_skeletal_get_render_bounding_box( &GHp2SkeletalServices, this, Owner, &Coords );
	return Coords;
	unguardobj;
}

FBox USkeletalMesh::GetCollisionBoundingBox( const AActor* Owner, bool bWorld ) const
{
	FBox Bound;
	hp2_rs_engine_skeletal_get_collision_bounding_box( &GHp2SkeletalServices, this, Owner, bWorld ? 1 : 0, &Bound );
	return Bound;
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


void USkeletalMesh::ApplyAnim
(
	AActor*			AnimOwner,
	CFSkelHeader*	ParentHdr,
	bool			bRootOnly
) const
{
	hp2_rs_engine_skeletal_apply_anim( &GHp2SkeletalServices, this, AnimOwner, ParentHdr, bRootOnly ? 1 : 0 );
}

int USkeletalMesh::BoneIndex( FName BoneName ) const
{
	return hp2_rs_engine_skeletal_bone_index( this, &BoneName );
}

FName USkeletalMesh::BoneName( int Index ) const
{
	FName Name;
	hp2_rs_engine_skeletal_bone_name( this, Index, &Name );
	return Name;
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
	hp2_rs_engine_skeletal_get_frame( &GHp2SkeletalServices, this, ResultVerts, Size, &Coords, Owner, &LODRequest );
	unguardobj;
}

FVector USkeletalMesh::GetRootMovement
(
	AActor* Owner
)
{
	FVector Move;
	hp2_rs_engine_skeletal_get_root_movement( &GHp2SkeletalServices, this, Owner, &Move );
	return Move;
}

void USkeletalMesh::AdjustRootMovement
(
	AActor* Owner,
	const FVector& Adjust
)
{
	hp2_rs_engine_skeletal_adjust_root_movement( &GHp2SkeletalServices, Owner, &Adjust );
}

FVector USkeletalMesh::AnimCycleMovement
(
	AActor*		Owner
)
{
	FVector Move;
	hp2_rs_engine_skeletal_anim_cycle_movement( &GHp2SkeletalServices, this, Owner, &Move );
	return Move;
}

FCoords USkeletalMesh::GetBoneCoords( AActor* Owner, int Bone ) const
{
	FCoords Coords;
	hp2_rs_engine_skeletal_get_bone_coords( &GHp2SkeletalServices, this, Owner, Bone, &Coords );
	return Coords;
}

/*-----------------------------------------------------------------------------
	The end.
-----------------------------------------------------------------------------*/

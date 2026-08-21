/*=============================================================================
	UnSkeletalMesh.h: Unreal mesh objects.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	UAnimation: Unreal Animation object
	Objects containing skeletal or heirarchical animation keys.
	(Classic vertex animation is stored inside UMesh object.)

	Copyright 1999,2000 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* ULodMesh, USkeletalMesh subclassing - Erik 
=============================================================================*/

#include "UnMesh.h"

// A couple options for storing animations.
// Turns out the best compression is achieved with regular relative rotations,
// which require the 48-bit quantised rotations to minimise cumulative error.
#define ABS_ROTx
#define ANIM48

class FArchiveCountMem : public FArchive
{
public:
	FArchiveCountMem()
	:	Num(0), SeekOffset(0)
	{
		ArIsSaving = 1;
	}
	void Serialize( void*, INT Length )
	{
		if( SeekOffset == 0 )
			Num += Length;
	}
	FArchive& operator<<( class FName& N )
	{
		if( SeekOffset == 0 )
			Num += sizeof(N);
		return *this;
	}
	FArchive& operator<<( class UObject*& Obj )
	{
		if( SeekOffset == 0 )
			Num += sizeof(Obj);
		return *this;
	}
	INT Tell()
	{
		return Num;
	}
	void Seek( INT Pos )
	{
		SeekOffset = Pos-Num;
	}
protected:
	INT Num;
	INT SeekOffset;
};

inline INT MemSize( UObject* Obj )
{
	FArchiveCountMem Ar;
	Obj->Serialize(Ar);
	return Ar.Tell();
}

/*-----------------------------------------------------------------------------
	USkeletalMesh.
-----------------------------------------------------------------------------*/

// Note: uses old-style resource loading, then saves it as an USkeletalMesh in the
// .u building phase.  That's why some of these structs are serializable and others aren't.

// A bone: an orientation, and a position, all relative to their parent.
struct VJointPos
{
	FPlace		Place;

	FLOAT       Length;       //  For collision testing / debugging drawing...
	FLOAT       XSize;
	FLOAT       YSize;
	FLOAT       ZSize;

	friend FArchive &operator<<( FArchive& Ar, VJointPos& V )
	{
		return Ar << V.Place << V.Length << V.XSize << V.XSize << V.ZSize;
	}
};

// Reference-skeleton bone, the package-serializable version.
struct FMeshBone
{
	FName 		Name;		  // Bone's name.
	DWORD		Flags;        // reserved
	VJointPos	BonePos;      // reference position
	INT         ParentIndex;  // 0/NULL if this is the root bone.  
	INT 		NumChildren;  // children  // only needed in animation ?
	INT         Depth;        // Number of steps to root in the skeletal hierarcy; root=0.
	friend FArchive &operator<<( FArchive& Ar, FMeshBone& F)
	{
		return Ar << F.Name << F.Flags << F.BonePos << F.NumChildren << F.ParentIndex;
	}
};

// Named bone for the animating skeleton data. 
struct FNamedBone
{
	FName	   Name;  // Bone's fname (== single 32-bit index to name)
	DWORD      Flags; // reserved
	INT        ParentIndex;  // 0/NULL if this is the root bone.  
	friend FArchive &operator<<( FArchive& Ar, FNamedBone& F)
	{
		return Ar << F.Name << F.Flags << F.ParentIndex;
	}
};

// Binary animation info format - used to organize raw animation keys into FAnimSeqs on rebuild
// Similar to MotionChunkDigestInfo..
struct AnimInfoBinary
{
	ANSICHAR Name[64];     // Animation's name
	ANSICHAR Group[64];    // Animation's group name	

	INT TotalBones;           // TotalBones * NumRawFrames is number of animation keys to digest.

	INT RootInclude;          // 0 none 1 included 		
	INT KeyCompressionStyle;  // Reserved: variants in tradeoffs for compression.
	INT KeyQuotum;            // Max key quotum for compression	
	FLOAT KeyReduction;       // desired 
	FLOAT TrackTime;            // explicit - can be overridden by the animation rate
	FLOAT AnimRate;           // frames per second.
	INT StartBone;            // - Reserved: for partial animations.
	INT FirstRawFrame;        //
	INT NumRawFrames;         // NumRawFrames and AnimRate dictate tracktime...
};


// Bone influence blending
struct VBoneInfIndex // ,, ,, contains Index, number of influences per bone (+ N detail level sizers! ..)
{
	_WORD WeightIndex;
	_WORD Number;  // how many to process 
	_WORD DetailA;  // how many to process if we're up to 2 max influences
	_WORD DetailB;  // how many to process if we're up to full 3 max influences 

	friend FArchive &operator<<( FArchive& Ar, VBoneInfIndex& V )
	{
		return Ar << V.WeightIndex << V.Number << V.DetailA << V.DetailB;
	}
};

struct VBoneInfluence // Weight and vertex number
{
	_WORD PointIndex; // 3d vertex
	_WORD BoneWeight; // 0..1 scaled influence

	friend FArchive &operator<<( FArchive& Ar, VBoneInfluence& V )
	{
		return Ar << V.PointIndex << V.BoneWeight;
	}
};

// An animation key.
struct VQuatAnimKey
{
	FVector		Position;           // relative to parent.
	FQuat       Orientation;        // relative to parent.
	FLOAT       Time;				// The duration until the next key (end key wraps to first...)

	friend FArchive &operator<<( FArchive& Ar, VQuatAnimKey& V )
	{
		return Ar << V.Position << V.Orientation << V.Time;
	}
};

//+--------------------------------------------------------------------------
struct ENGINE_API FAnimVec
//
// A normalised vector value (-1..+1) compressed to 10-bits per component.
// Can represent translation vectors, or rotations.
//
{
#ifdef ANIM48
	SWORD X, Y, Z;
#else
	INT X:10, Y:10, Z:10;
	DWORD Shift: 2;
#endif

	// Constructors.

	inline FAnimVec()
	{}

	// Construct from vector, with additional scale.
	FAnimVec( const FVector& v, float InvScale )
	{
		Init( v, InvScale );
	}

	FAnimVec( const FQuat& q );

	// Conversions.

	// Convert back to Vector, with additional scale.
	inline FVector Vector( float Scale ) const
	{
#ifdef ANIM48
		Scale /= BASE_SCALE;
#else
		Scale /= (BASE_SCALE<<Shift);
#endif
		return FVector(X * Scale, Y * Scale, Z * Scale);
	}

	FQuat Quat() const;

	// Functions.

	friend FArchive& operator<<(FArchive& Ar, FAnimVec& A)
	{
#ifdef ANIM48
		return Ar << A.X << A.Y << A.Z;
#else
		DWORD Packed = 0;
		if( !Ar.IsLoading() )
		{
			Packed
				=  (static_cast<DWORD>(A.X) & 0x3ffu)
				| ((static_cast<DWORD>(A.Y) & 0x3ffu) << 10)
				| ((static_cast<DWORD>(A.Z) & 0x3ffu) << 20)
				| ((static_cast<DWORD>(A.Shift) & 0x3u) << 30);
		}

		Ar << Packed;
		if( Ar.IsLoading() )
		{
			const DWORD PackedX = Packed & 0x3ffu;
			const DWORD PackedY = (Packed >> 10) & 0x3ffu;
			const DWORD PackedZ = (Packed >> 20) & 0x3ffu;
			A.X = (PackedX & 0x200u) ? static_cast<INT>(PackedX) - 0x400 : static_cast<INT>(PackedX);
			A.Y = (PackedY & 0x200u) ? static_cast<INT>(PackedY) - 0x400 : static_cast<INT>(PackedY);
			A.Z = (PackedZ & 0x200u) ? static_cast<INT>(PackedZ) - 0x400 : static_cast<INT>(PackedZ);
			A.Shift = (Packed >> 30) & 0x3u;
		}
		return Ar;
#endif
	}

protected:

#ifdef ANIM48
	enum { BASE_SCALE = (1<<15)-1 };
#else
	enum { BASE_SCALE = (1<<9)-1 };
#endif

	void Init( const FVector& v, float InvScale );

	static inline int IntVal(float f)
	{
		int i = appRound(f);
		checkSlow(Abs(i) <= BASE_SCALE);
		return i;
	}
};

#if __INTEL_BYTE_ORDER__
template <> struct TTypeInfo<FAnimVec> : public TTypeInfoBase<FAnimVec>
{
	static UBOOL FastSerialize() {return 1;}
};
#endif

//
// 'Analog' animation key track (for single bone/element.)
// Either KeyPos or KeyQuat can be single/empty? entries to signify no movement at all;
// for N>1 entries there's always N keytimers available.
//
struct FAnalogTrack
{	
	DWORD Flags;       // reserved 
	TRefArray <FAnimVec>	KeyQuat;	// Orientation key track
	TRefArray <FAnimVec>	KeyPos;		// Position key track
	TRefArray <BYTE>		KeyDelta;	// Number of frames since previous key.
	FLOAT PosScale;						// Scale for position track.
	FLOAT TimeScale;					// Scale for time track.

	inline FVector GetKeyPos( int i ) const
	{
		return KeyPos(i).Vector( PosScale );
	}
	friend FArchive &operator<<( FArchive& Ar, FAnalogTrack& A )
	{
		return Ar << A.Flags << A.KeyQuat << A.KeyPos << A.KeyDelta << A.PosScale << A.TimeScale;
	}
};

// Single storage structure for all tracks.
struct FMasterTrack
{	
	TArray <FAnimVec>	KeyQuat;
	TArray <FAnimVec>	KeyPos;
	TArray <BYTE>		KeyDelta;
	friend FArchive &operator<<( FArchive& Ar, FMasterTrack & A )
	{
		return Ar << A.KeyQuat << A.KeyPos << A.KeyDelta;
	}
};

//
// Motion chunks as defined by Animsequences in script and/or exported raw data.
// Used during digestion phase only.
//
struct MotionChunkDigestInfo
{	
	FName	Name;		 // Sequence's name.
	FName	Group;		 // Group.	
	INT     RootInclude; // 0=none, 1=include, 2=store only root motion/
	INT     KeyCompressionStyle;
	INT     KeyQuotum;
	FLOAT   KeyReduction;
	FLOAT   TrackTime;
	FLOAT   AnimRate;
	INT     StartBone;
	INT     FirstRawFrame;
	INT     NumRawFrames;	
};




// Individual animation;  subgroup of bones with compressed animation.
struct MotionChunk
{
	FVector RootSpeed3D;  // Net 3d speed.
	FLOAT   TrackTime;    // Total time (Same for each track.)
	INT     StartBone;    // If we're a partial-hierarchy-movement, this is the lowest bone.
	DWORD   Flags;        // Reserved 

	TArray<INT>				BoneIndices;	// Refbones number of Bone indices (-1 or valid one) to fast-find tracks for a particular bone.
	// Frame-less, compressed animation tracks. NumBones times NumAnims tracks in total 
	TArray<FAnalogTrack>	AnimTracks;		// Compressed key tracks (one for each bone)
	//FAnalogTrack			RootTrack;		// May or may not be used; actual traverse-a-scene root tracks for use
	// with cutscenes / special physics modes, in addition to the regular skeletal root track.

	friend FArchive &operator<<( FArchive& Ar, MotionChunk& M)
	{
		return Ar << M.RootSpeed3D << M.TrackTime << M.StartBone << M.Flags << M.BoneIndices << M.AnimTracks;
	}
};


// Skeletal mesh
class ENGINE_API USkeletalMesh : public ULodMesh
{
	DECLARE_CLASS(USkeletalMesh,ULodMesh,0,Engine)

    // Special skeletal data structures: 
	TArray<FMeshExtWedge>	ExtWedges;     // Extended wedges with floating point UV's      
	TArray<FVector>         Points;        // Floating point vectors directly form our skin vertex 3d points.

	// Skeletal specific data
	TArray<FMeshBone>       RefSkeleton;   // Reference skeleton.

	// Bone weights
	TArray <VBoneInfIndex>  BoneWeightIdx; // ,, ,, contains Index, number of influences per bone.
	TArray <VBoneInfluence> BoneWeights;   // Each element contians weight and vertex number.
	TArray <FVector>        LocalPoints;   // Each weighted point in local bone-space

	FBox					BoundingBoxSum;	// Sum of all anim BBs.

	INT WeaponBoneIndex;   // -1 means it hasn't been assigned.
	FCoords WeaponAdjust;  // Weapon adjustment coordinate system - identity by default.
	INT SkeletalDepth;  // The max hierarchy depth.
	UAnimation* DefaultAnimation; // Link this up when no other animation is available - for backwards compatibility.

	// For debugging only - not serialized.
	TArray <FVector> DebugPivots;
	TArray <INT>	 DebugParents;
	TArray <BYTE>    DebugVerts;
	UBOOL   DisplayBones;
	UBOOL   DisplayInfluences;

	// Runtime weapon coords - not serialized
	FCoords ClassicWeaponCoords;
	
	//  UObject interface.
	USkeletalMesh();
	virtual void Serialize( FArchive& Ar );

	// UPrimitive interface.
	virtual FBox GetCollisionBoundingBox( const AActor* Owner, bool bWorld ) const;

	//  UMesh interface.
	virtual FCoords GetRenderBoundingBox( const AActor* Owner, UBOOL Exact ); 
	virtual void SetScale( FVector NewScale );
	virtual void FlipFaces();

	// Special GetFrame for Skeletal animation
	virtual void GetFrame( FVector* Verts, INT Size, FCoords Coords, AActor* Owner, INT& LODRequest );
	virtual void GetFrame( FVector* Verts, INT Size, FCoords Coords, AActor* Owner )
	{
		INT LODVerts = FrameVerts;
		GetFrame( Verts, Size, Coords, Owner, LODVerts );
	}
	virtual FVector GetRootMovement( AActor* Owner );
	virtual void AdjustRootMovement( AActor* Owner, const FVector& Adjust );
	virtual FVector AnimCycleMovement( AActor* Owner );
	virtual FCoords GetBoneCoords( AActor* Owner, int Bone ) const;
	virtual INT MemFootprint()
	{
		return Super::MemFootprint()
			+ sizeof(*this) - sizeof(Super)
			+ ExtWedges.Mem() + Points.Mem() + RefSkeleton.Mem() 
			+ BoneWeightIdx.Mem() + BoneWeights.Mem() + LocalPoints.Mem();
	}

	// USkeletalMesh interface.
	int BoneIndex( FName BoneName ) const;
	FName BoneName( int Index ) const;

	struct CFSkelHeader;

private:
	void GetBones( TArray<FCoords>& Bones, AActor* Owner ) const;
	void ApplyAnim( AActor* AnimOwner, CFSkelHeader* ParentHdr = NULL, bool bRootOnly = false ) const;
	FCoords GetMeshCoords( const AActor* Owner ) const;
	FVector MeshAdjust( const AActor* Owner ) const;
};

/*-----------------------------------------------------------------------------
	UAnimation definition.
-----------------------------------------------------------------------------*/
//
// UnAnimation, the base class of animating skeletal bones which are linked up by name dynamically to 
// USkeletalMesh skins/reference skeletons. 
//
class ENGINE_API UAnimation : public UObject
{
	DECLARE_CLASS(UAnimation,UObject,0,Engine)

	// Variables.
	// FBox BoundingBox;
	// FSphere BoundingSphere;

	// UObject interface.
	UAnimation();
	void Serialize( FArchive& Ar );

	virtual const FMeshAnimSeq* GetAnimSeq( FName SeqName ) const
	{
		guardSlow(UAnimation::GetAnimSeq);
		for( INT i=0; i<AnimSeqs.Num(); i++ )
		{
			if( SeqName == AnimSeqs(i).Name )
				return &AnimSeqs(i);
		}
		return NULL;
		unguardSlow;
	}

	virtual FMeshAnimSeq* GetAnimSeq( FName SeqName )
	{
		guardSlow(UAnimation::GetAnimSeq);
		for( INT i=0; i<AnimSeqs.Num(); i++ )
		{
			if( SeqName == AnimSeqs(i).Name )
				return &AnimSeqs(i);
		}
		return NULL;
		unguardSlow;
	}

	virtual const MotionChunk* GetMovement( FName SeqName ) const 
	{
		guardSlow(UAnimation::GetAnimSeq);
		for( INT i=0; i<AnimSeqs.Num(); i++ )
		{
			if( SeqName == AnimSeqs(i).Name )
				return &Moves(i);
		}
		return NULL;
		unguardSlow;
	}

	virtual MotionChunk* GetMovement( FName SeqName )
	{
		guardSlow(UAnimation::GetAnimSeq);
		for( INT i=0; i<AnimSeqs.Num(); i++ )
		{
			if( SeqName == AnimSeqs(i).Name )
				return &Moves(i);
		}
		return NULL;
		unguardSlow;
	}

	// Estimates memory footprint in bytes of digested data only
	virtual INT MemFootprint()
	{
		guard(UAnimation::MemFootprint);

		SIZE_T TotalMem = sizeof(*this) + static_cast<SIZE_T>(RefBones.Mem());
		TotalMem += MasterTrack.KeyQuat.Mem();
		TotalMem += MasterTrack.KeyPos.Mem();
		TotalMem += MasterTrack.KeyDelta.Mem();

		TotalMem += Moves.Mem();
		for(INT i=0;i<Moves.Num();i++)
			TotalMem += Moves(i).AnimTracks.Mem();
		TotalMem += AnimSeqs.Mem();
		return appCheckedIntSize(TotalMem);
		unguard;
	}

	// Skeletal animation data. Linked up to a reference skeleton at runtime.
	TArray<FNamedBone>    RefBones;        // Name.
	FMasterTrack		  MasterTrack;     // Serialised track that contains conglomerated data for individual tracks.
	TArray<MotionChunk>   Moves;           // One for every animation - has hierarchy starting point,
	                                       // speed, flags, compression, and actual animation.

	// MeshAnimSeq information; per-animation names/data/notifies.
	TArray<FMeshAnimSeq>  AnimSeqs;        // Classic AnimSeqs.

	// Raw uncompressed animation keys - not to be serialized.
	
	TArray<MotionChunkDigestInfo>  MovesInfo;		  // Moves info from file or script, instructions to build the AnimSeqs.
	INT                     RawNumFrames;			  // Raw number of frames.
	TArray<VQuatAnimKey>    RawAnimKeys;			  // Raw keys (bones * frames), ordered by frames.
	TArray<AnimInfoBinary>  RawAnimSeqInfo;	          // Moves info from file (optional)
	FLOAT                   CompFactor;               // Default global compression factor when digesting animations.
};

/*----------------------------------------------------------------------------
	The End.
----------------------------------------------------------------------------*/

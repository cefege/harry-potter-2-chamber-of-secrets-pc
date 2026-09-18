/*=============================================================================
	UnModel.cpp: Unreal model functions
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"

/*-----------------------------------------------------------------------------
	Struct serializers.
-----------------------------------------------------------------------------*/

ENGINE_API FArchive& operator<<( FArchive& Ar, FBspSurf& Surf )
{
	guard(FBspSurf<<);
	Ar << Surf.Texture;
	Ar << Surf.PolyFlags << AR_INDEX(Surf.pBase) << AR_INDEX(Surf.vNormal);
	Ar << AR_INDEX(Surf.vTextureU) << AR_INDEX(Surf.vTextureV);
	Ar << AR_INDEX(Surf.iLightMap) << AR_INDEX(Surf.iBrushPoly);
	Ar << Surf.PanU << Surf.PanV;
	Ar << Surf.Actor;
	if( Ar.IsLoading() )
		Surf.PolyFlags &= ~PF_Transient;
#ifndef NODECALS
	if( !Ar.IsLoading() && !Ar.IsSaving() )
		Ar << Surf.Decals;
#endif
	return Ar;
	unguard;
}

ENGINE_API FArchive& operator<<( FArchive& Ar, FPoly& Poly )
{
	guard(FPoly<<);
	Ar << AR_INDEX(Poly.NumVertices);
	Ar << Poly.Base << Poly.Normal << Poly.TextureU << Poly.TextureV;
	for( INT i=0; i<Poly.NumVertices; i++ )
		Ar << Poly.Vertex[i];
	Ar << Poly.PolyFlags;
	Ar << Poly.Actor << Poly.Texture << Poly.ItemName;
	Ar << AR_INDEX(Poly.iLink) << AR_INDEX(Poly.iBrushPoly) << Poly.PanU << Poly.PanV;
	if( Ar.IsLoading() )
		Poly.PolyFlags &= ~PF_Transient;
	return Ar;
	unguard;
}

ENGINE_API FArchive& operator<<( FArchive& Ar, FBspNode& N )
{
	guard(FBspNode<<);
	Ar << N.Plane << N.ZoneMask << N.NodeFlags << AR_INDEX(N.iVertPool) << AR_INDEX(N.iSurf);
	Ar << AR_INDEX(N.iChild[0]) << AR_INDEX(N.iChild[1]) << AR_INDEX(N.iChild[2]);
	Ar << AR_INDEX(N.iCollisionBound) << AR_INDEX(N.iRenderBound);
	Ar << N.iZone[0] << N.iZone[1];
	Ar << N.NumVertices;
	Ar << N.iLeaf[0] << N.iLeaf[1];
	if( Ar.IsLoading() )
		N.NodeFlags &= ~(NF_IsNew|NF_IsFront|NF_IsBack);
	return Ar;
	unguard;
}

/*---------------------------------------------------------------------------------------
	Old database implementations.
---------------------------------------------------------------------------------------*/

IMPLEMENT_CLASS(UBspNodes);

IMPLEMENT_CLASS(UBspSurfs);

IMPLEMENT_CLASS(UVectors);

IMPLEMENT_CLASS(UVerts);

/*---------------------------------------------------------------------------------------
	UModel object implementation.
---------------------------------------------------------------------------------------*/

template<class T>
struct TTagObject
{
	T& Object;
	const TCHAR* Tag;

	TTagObject( T& InObj, const TCHAR* InTag )
	:	Object(InObj), Tag(InTag)
	{}

	friend inline FArchive& operator <<( FArchive& Ar, const TTagObject<T>& TagObj )
	{
		FPushMemTag Tag(TagObj.Tag);
		return Ar << TagObj.Object;
	}
};

template<class T>
inline TTagObject<T> TagObject( T& Object, const TCHAR* Tag )
{
	return TTagObject<T>( Object, Tag );
}

#define TagObj(Obj)	TagObject( Obj, TEXT(#Obj) )


void UModel::Serialize( FArchive& Ar )
{
	guard(UModel::Serialize);
	Super::Serialize( Ar );

	if( Ar.Ver()<=61 )
	{
		//oldver
		UBspSurfs* _Surfs   = NULL;
		UVectors*  _Vectors = NULL;
		UVectors*  _Points  = NULL;
		UVerts*    _Verts   = NULL;
		UBspNodes* _Nodes   = NULL;

		Ar << _Vectors << _Points << _Nodes << _Surfs << _Verts;
		Ar << Polys;

		if( _Vectors )
		{
			Ar.Preload( _Vectors );
			ExchangeArray( _Vectors->Element, Vectors );
		}
		if( _Points )
		{
			Ar.Preload( _Points );
			ExchangeArray( _Points->Element, Points );
		}
		if( _Surfs )
		{
			Ar.Preload( _Surfs );
			ExchangeArray( _Surfs->Element, Surfs );
		}
		if( _Verts )
		{
			Ar.Preload( _Verts );
			ExchangeArray( _Verts->Element, Verts );
			NumSharedSides = _Verts->NumSharedSides;
		}
		if( _Nodes )
		{
			Ar.Preload( _Nodes );
			ExchangeArray( _Nodes->Element, Nodes );
			NumZones = _Nodes->_NumZones;
			for( INT i=0; i<NumZones; i++ )
				Zones[i] = _Nodes->_Zones[i];
		}
		if( Polys && !Ar.IsTrans() )
		{
			Ar.Preload( Polys );
		}

		Ar << LightMap << LightBits << Bounds << LeafHulls << Leaves << Lights;

		UObject* Tmp=NULL;
		Ar << Tmp << Tmp;
	}
	else
	{
		Ar << TagObj(Vectors) 
		   << TagObj(Points)
		   << TagObj(Nodes)
		   << TagObj(Surfs)
		   << TagObj(Verts);

		Ar << NumSharedSides 
		   << NumZones;
		{
			FPushMemTag Tag(TEXT("Zones"));
			for( INT i=0; i<NumZones; i++ )
				Ar << Zones[i];
		}
		Ar << TagObj(Polys);
		if( Polys && !Ar.IsTrans() )
		{
			Ar.Preload( Polys );
		}

		Ar << TagObj(LightMap) 
		   << TagObj(LightBits) 
		   << TagObj(Bounds) 
		   << TagObj(LeafHulls) 
		   << TagObj(Leaves) 
		   << TagObj(Lights);

		/*if( !GIsEditor && Ar.IsLoading() )
		{
			// Remove unused Lights and LightBits entries.
			TArray<INT> RemapLights;	RemapLights.AddZeroed(Lights.Num());
			INT FirstBits = LightBits.Num();
			INT i;
			fore_array( i, Leaves )
			{
				if( Leaves[i].iPermeating != INDEX_NONE )
				{
					INT j=Leaves[i].iPermeating;
					if( !Lights[j] )
						Leaves[i].iPermeating = INDEX_NONE;
					else
					{
						for( ; Lights[j]; j++ )
							RemapLights[j] = 1;
						RemapLights[j] = 1;
					}
				}
				if( Leaves[i].iVolumetric != INDEX_NONE )
				{
					INT j=Leaves[i].iVolumetric;
					if( !Lights[j] )
						Leaves[i].iVolumetric = INDEX_NONE;
					else
					{
						for( ; Lights[j]; j++ )
							RemapLights[j] = 1;
						RemapLights[j] = 1;
					}
				}
			}
			INT OldBytes = 0, NewBytes = 0;
			fore_array( i, LightMap )
			{
				if( LightMap[i].iLightActors != INDEX_NONE )
				{
					INT OldSize = ((LightMap[i].UClamp+7) >> 3) * LightMap[i].VClamp;
					INT NewSize = (LightMap[i].UClamp * LightMap[i].VClamp + 7) >> 3;

					INT j=LightMap[i].iLightActors;
					if( !Lights[j] )
						LightMap[i].iLightActors = INDEX_NONE;
					else
					{
						FirstBits = Min( FirstBits, LightMap[i].DataOffset );
						for( ; Lights[j]; j++ )
						{
							RemapLights[j] = 1;
							OldBytes += OldSize;
							NewBytes += NewSize;
						}
						RemapLights[j] = 1;
					}
				}
			}

			// Remap lights.
			INT Used = 0;
			for_array( u, RemapLights )
			{
				if( RemapLights[u] )
				{
					Lights[Used] = Lights[u];
					RemapLights[u] = Used++;
				}
			}
			Lights.Remove(Used, Lights.Num()-Used);

			// Remap bits.
			LightBits.Remove(0, FirstBits);

			fore_array( i, Leaves )
			{
				if( Leaves[i].iPermeating != INDEX_NONE )
					Leaves[i].iPermeating = RemapLights[ Leaves[i].iPermeating ];
				if( Leaves[i].iVolumetric != INDEX_NONE )
					Leaves[i].iVolumetric = RemapLights[ Leaves[i].iVolumetric ];
			}
			fore_array( i, LightMap )
			{
				if( LightMap[i].iLightActors != INDEX_NONE )
				{
					LightMap[i].iLightActors = RemapLights[ LightMap[i].iLightActors ];
					LightMap[i].DataOffset -= FirstBits;
				}
			}
		}*/
	}

	Ar << RootOutside << Linked;

	unguard;
}
void UModel::PostLoad()
{
	guard(UModel::PostLoad);
	for( INT i=0; i<Nodes.Num(); i++ )
		Surfs(Nodes(i).iSurf).Nodes.AddItem(i);
	Super::PostLoad();
	unguard;
}
void UModel::ModifySurf( INT Index, INT UpdateMaster )
{
	guard(UModel::ModifySurf);

	Surfs.ModifyItem( Index );
	FBspSurf& Surf = Surfs(Index);
	if( UpdateMaster && Surf.Actor )
		Surf.Actor->Brush->Polys->Element.ModifyItem( Surf.iBrushPoly );

	unguard;
}
void UModel::ModifyAllSurfs( INT UpdateMaster )
{
	guard(UModel::ModifyAllSurfs);

	for( INT i=0; i<Surfs.Num(); i++ )
		ModifySurf( i, UpdateMaster );

	unguard;
}
void UModel::ModifySelectedSurfs( INT UpdateMaster )
{
	guard(UModel::ModifySelectedSurfs);

	for( INT i=0; i<Surfs.Num(); i++ )
		if( Surfs(i).PolyFlags & PF_Selected )
			ModifySurf( i, UpdateMaster );

	unguard;
}
IMPLEMENT_CLASS(UModel);

/*---------------------------------------------------------------------------------------
	UModel implementation.
---------------------------------------------------------------------------------------*/

//
// Lock a model.
//
void UModel::Modify( UBOOL DoTransArrays )
{
	guard(UModel::Modify);

	// Modify all child objects.
	//warning: Don't modify self because model contains a dynamic array.
	if( Polys   ) Polys->Modify();

	unguard;
}

//
// Empty the contents of a model.
//
void UModel::EmptyModel( INT EmptySurfInfo, INT EmptyPolys )
{
	guard(UModel::EmptyModel);

	// Init arrays.
	Nodes		.Empty();
	Bounds		.Empty();
	LeafHulls	.Empty();
	Leaves		.Empty();
	Lights		.Empty();
	LightMap	.Empty();
	LightBits	.Empty();
	Verts		.Empty();
	if( EmptySurfInfo )
	{
		Vectors.Empty();
		Points.Empty();
		for( INT i=0; i<Surfs.Num(); i++ )
			Surfs(i).Decals.Empty();
		Surfs.Empty();
	}
	if( EmptyPolys )
	{
		Polys = new( GetOuter(), NAME_None, RF_Transactional )UPolys;
	}

	// Init variables.
	NumZones		= 0;
	NumSharedSides	= 4;
	NumZones = 0;
	for( INT i=0; i<FBspNode::MAX_ZONES; i++ )
	{
		Zones[i].ZoneActor    = NULL;
		Zones[i].Connectivity = ((QWORD)1)<<i;
		Zones[i].Visibility   = ~(QWORD)0;
	}	

	unguard;
}


//
// Create a new model and allocate all objects needed for it.
//
UModel::UModel( ABrush* Owner, UBOOL InRootOutside )
:	RootOutside	( InRootOutside )
,	Surfs		( this )
,	Vectors		( this )
,	Points		( this )
,	Verts		( this )
,	Nodes		( this )
{
	guard(UModel::UModel);
	SetFlags( RF_Transactional );
	EmptyModel( 1, 1 );
	if( Owner )
	{
		Owner->Brush = this;
		Owner->InitPosRotScale();
	}
	unguard;
}

//
// Build the model's bounds (min and max).
//
void UModel::BuildBound()
{
	guard(UModel::BuildBound);
	if( Polys && Polys->Element.Num() )
	{
		TArray<FVector> Points;
		for( INT i=0; i<Polys->Element.Num(); i++ )
			for( INT j=0; j<Polys->Element(i).NumVertices; j++ )
				Points.AddItem(Polys->Element(i).Vertex[j]);
		BoundingBox    = FBox( &Points(0), Points.Num() );
		BoundingSphere = FSphere( &Points(0), Points.Num() );
	}
	else BoundingBox = FBox(0);
	unguard;
}

//
// Transform this model by its coordinate system.
//
void UModel::Transform( ABrush* Owner )
{
	guard(UModel::Transform);
	check(Owner);

	Polys->Element.ModifyAllItems();

	FModelCoords Coords;
	FLOAT Orientation = Owner->BuildCoords( &Coords, NULL );
	for( INT i=0; i<Polys->Element.Num(); i++ )
		Polys->Element( i ).Transform( Coords, Owner->PrePivot, Owner->Location, Orientation );

	unguard;
}

//
// Returns whether a BSP leaf is potentially visible from another leaf.
//
UBOOL UModel::PotentiallyVisible( INT iLeaf1, INT iLeaf2 )
{
	// This is the amazing superfast patent-pending 1 cpu cycle potential visibility 
	// algorithm programmed by the great Tim Sweeney!
	return 1;
}

/*---------------------------------------------------------------------------------------
	UModel basic implementation.
---------------------------------------------------------------------------------------*/

//
// Shrink all stuff to its minimum size.
//
void UModel::ShrinkModel()
{
	guard(UModel::ShrinkModel);

	Vectors		.Shrink();
	Points		.Shrink();
	Verts		.Shrink();
	Nodes		.Shrink();
	Surfs		.Shrink();
	if( Polys     ) Polys    ->Element.Shrink();
	Bounds		.Shrink();
	LeafHulls	.Shrink();

	unguard;
}

/*---------------------------------------------------------------------------------------
	The End.
---------------------------------------------------------------------------------------*/

/*=============================================================================
	Hp2DynBspServices.cpp: C++ thunks behind the Rust FMovingBrushTracker.
=============================================================================*/

#include "EnginePrivate.h"
#include "Hp2DynBspServices.h"

/*-----------------------------------------------------------------------------
	Model arrays. Pool: 0 Surfs, 1 Vectors, 2 Points, 3 Nodes, 4 Verts.
-----------------------------------------------------------------------------*/

static int DynBspArrayNum( void* Model, unsigned int Pool )
{
	UModel* M = (UModel*)Model;
	switch( Pool )
	{
		case 0:  return M->Surfs.Num();
		case 1:  return M->Vectors.Num();
		case 2:  return M->Points.Num();
		case 3:  return M->Nodes.Num();
		default: return M->Verts.Num();
	}
}

static void* DynBspArrayData( void* Model, unsigned int Pool )
{
	UModel* M = (UModel*)Model;
	switch( Pool )
	{
		case 0:  return M->Surfs.Data;
		case 1:  return M->Vectors.Data;
		case 2:  return M->Points.Data;
		case 3:  return M->Nodes.Data;
		default: return M->Verts.Data;
	}
}

// TArray::Set followed by appMemzero of the new tail (same as ExpandDb).
template<class T> static void DynBspSetNumZeroed( TArray<T>& Array, INT N )
{
	INT Old = Array.Num();
	Array.Set( N );
	if( N > Old )
		appMemzero( &Array(Old), (N-Old)*sizeof(T) );
}

static void DynBspArraySetNum( void* Model, unsigned int Pool, int N )
{
	UModel* M = (UModel*)Model;
	switch( Pool )
	{
		case 0:  DynBspSetNumZeroed( M->Surfs,   N ); break;
		case 1:  DynBspSetNumZeroed( M->Vectors, N ); break;
		case 2:  DynBspSetNumZeroed( M->Points,  N ); break;
		case 3:  DynBspSetNumZeroed( M->Nodes,   N ); break;
		default: DynBspSetNumZeroed( M->Verts,   N ); break;
	}
}

static void DynBspArrayShrink( void* Model, unsigned int Pool )
{
	UModel* M = (UModel*)Model;
	switch( Pool )
	{
		case 0:  M->Surfs.Shrink();   break;
		case 1:  M->Vectors.Shrink(); break;
		case 2:  M->Points.Shrink();  break;
		case 3:  M->Nodes.Shrink();   break;
		default: M->Verts.Shrink();   break;
	}
}

static void DynBspArrayRemove( void* Model, unsigned int Pool, int Index, int Count )
{
	UModel* M = (UModel*)Model;
	switch( Pool )
	{
		case 0:  M->Surfs.Remove( Index, Count );   break;
		case 1:  M->Vectors.Remove( Index, Count ); break;
		case 2:  M->Points.Remove( Index, Count );  break;
		case 3:  M->Nodes.Remove( Index, Count );   break;
		default: M->Verts.Remove( Index, Count );   break;
	}
}

/*-----------------------------------------------------------------------------
	Level, actors, brushes.
-----------------------------------------------------------------------------*/

static int DynBspLevelActorCount( void* Level )
{
	return ((ULevel*)Level)->Actors.Num();
}

static void* DynBspLevelActor( void* Level, int I )
{
	return ((ULevel*)Level)->Actors(I);
}

static const char* DynBspLevelFullName( void* Level )
{
	static FString Name;
	Name = ((ULevel*)Level)->GetFullName();
	return appToAnsi( *Name );
}

static int DynBspActorIsMovingBrush( void* Actor )
{
	return ((AActor*)Actor)->IsMovingBrush() ? 1 : 0;
}

static int DynBspActorIsMover( void* Actor )
{
	return ((AActor*)Actor)->IsA( AMover::StaticClass() ) ? 1 : 0;
}

static const char* DynBspActorName( void* Actor )
{
	static FString Name;
	Name = ((AActor*)Actor)->GetName();
	return appToAnsi( *Name );
}

static void DynBspActorLocation( void* Actor, void* Out )
{
	*(FVector*)Out = ((AActor*)Actor)->Location;
}

static void DynBspActorRotation( void* Actor, void* Out )
{
	*(FRotator*)Out = ((AActor*)Actor)->Rotation;
}

static void DynBspActorPrePivot( void* Actor, void* Out )
{
	*(FVector*)Out = ((ABrush*)Actor)->PrePivot;
}

static void DynBspActorSavedPos( void* Actor, void* Out )
{
	*(FVector*)Out = ((AMover*)Actor)->SavedPos;
}

static void DynBspSetActorSavedPos( void* Actor, const void* V )
{
	((AMover*)Actor)->SavedPos = *(const FVector*)V;
}

static void DynBspActorSavedRot( void* Actor, void* Out )
{
	*(FRotator*)Out = ((AMover*)Actor)->SavedRot;
}

static void DynBspSetActorSavedRot( void* Actor, const void* V )
{
	((AMover*)Actor)->SavedRot = *(const FRotator*)V;
}

static int DynBspActorAssimilated( void* Actor )
{
	return ((AMover*)Actor)->bAssimilated ? 1 : 0;
}

static void DynBspSetActorAssimilated( void* Actor, int V )
{
	((AMover*)Actor)->bAssimilated = V ? 1 : 0;
}

static int DynBspActorSpecialLit( void* Actor )
{
	return ((AMover*)Actor)->bSpecialLit ? 1 : 0;
}

static void* DynBspActorBrush( void* Actor )
{
	return ((AMover*)Actor)->Brush;
}

// ABrush::BuildCoords(&Coords, NULL): point/vector transforms out, orientation returned.
static float DynBspActorBuildCoords( void* Actor, void* Point, void* Vector )
{
	FModelCoords Coords;
	FLOAT Orientation = ((ABrush*)Actor)->BuildCoords( &Coords, NULL );
	*(FCoords*)Point  = Coords.PointXform;
	*(FCoords*)Vector = Coords.VectorXform;
	return Orientation;
}

static int DynBspBrushMoverLink( void* Brush )
{
	return ((UModel*)Brush)->MoverLink;
}

static void DynBspSetBrushMoverLink( void* Brush, int V )
{
	((UModel*)Brush)->MoverLink = V;
}

static int DynBspBrushPolys( void* Brush, void** Out )
{
	UModel* B = (UModel*)Brush;
	*Out = B->Polys->Element.Data;
	return B->Polys->Element.Num();
}

/*-----------------------------------------------------------------------------
	Archive and diagnostics.
-----------------------------------------------------------------------------*/

static void DynBspArchiveCountBytes( void* Ar, size_t Num, size_t Max )
{
	((FArchive*)Ar)->CountBytes( Num, Max );
}

static void DynBspDebug( unsigned int Kind, const char* Msg )
{
	EName N = Kind == 0 ? NAME_Init : (Kind == 1 ? NAME_Log : NAME_Warning);
	debugf( N, TEXT("%s"), ANSI_TO_TCHAR(Msg) );
}

/*-----------------------------------------------------------------------------
	Table. Field order matches Hp2DynBspServices.h and dynbsp.rs.
-----------------------------------------------------------------------------*/

const Hp2DynBspServices GHp2DynBspServices =
{
	DynBspArrayNum,
	DynBspArrayData,
	DynBspArraySetNum,
	DynBspArrayShrink,
	DynBspArrayRemove,
	DynBspLevelActorCount,
	DynBspLevelActor,
	DynBspLevelFullName,
	DynBspActorIsMovingBrush,
	DynBspActorIsMover,
	DynBspActorName,
	DynBspActorLocation,
	DynBspActorRotation,
	DynBspActorPrePivot,
	DynBspActorSavedPos,
	DynBspSetActorSavedPos,
	DynBspActorSavedRot,
	DynBspSetActorSavedRot,
	DynBspActorAssimilated,
	DynBspSetActorAssimilated,
	DynBspActorSpecialLit,
	DynBspActorBrush,
	DynBspActorBuildCoords,
	DynBspBrushMoverLink,
	DynBspSetBrushMoverLink,
	DynBspBrushPolys,
	DynBspArchiveCountBytes,
	DynBspDebug,
};

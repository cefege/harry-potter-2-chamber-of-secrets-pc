/*=============================================================================
	Hp2DynBspServices.h: C++ services for the Rust FMovingBrushTracker port.

	Consumed by crates/hp2-engine/src/dynbsp.rs (`Hp2DynBspServices`, #[repr(C)]).
	Field order must match the Rust struct exactly. Model arrays are addressed
	by Pool: 0 = Surfs, 1 = Vectors, 2 = Points, 3 = Nodes, 4 = Verts.
	Pointers are void* here and are cast to the real types in
	Hp2DynBspServices.cpp.
=============================================================================*/

#ifndef _INC_HP2DYNBSPSERVICES_H
#define _INC_HP2DYNBSPSERVICES_H

#include <stddef.h>

struct Hp2DynBspServices
{
	int          (*ArrayNum)( void* Model, unsigned int Pool );
	void*        (*ArrayData)( void* Model, unsigned int Pool );
	void         (*ArraySetNum)( void* Model, unsigned int Pool, int N );
	void         (*ArrayShrink)( void* Model, unsigned int Pool );
	void         (*ArrayRemove)( void* Model, unsigned int Pool, int Index, int Count );
	int          (*LevelActorCount)( void* Level );
	void*        (*LevelActor)( void* Level, int I );
	const char*  (*LevelFullName)( void* Level );
	int          (*ActorIsMovingBrush)( void* Actor );
	int          (*ActorIsMover)( void* Actor );
	const char*  (*ActorName)( void* Actor );
	void         (*ActorLocation)( void* Actor, void* Out );
	void         (*ActorRotation)( void* Actor, void* Out );
	void         (*ActorPrePivot)( void* Actor, void* Out );
	void         (*ActorSavedPos)( void* Actor, void* Out );
	void         (*SetActorSavedPos)( void* Actor, const void* V );
	void         (*ActorSavedRot)( void* Actor, void* Out );
	void         (*SetActorSavedRot)( void* Actor, const void* V );
	int          (*ActorAssimilated)( void* Actor );
	void         (*SetActorAssimilated)( void* Actor, int V );
	int          (*ActorSpecialLit)( void* Actor );
	void*        (*ActorBrush)( void* Actor );
	float        (*ActorBuildCoords)( void* Actor, void* Point, void* Vector );
	int          (*BrushMoverLink)( void* Brush );
	void         (*SetBrushMoverLink)( void* Brush, int V );
	int          (*BrushPolys)( void* Brush, void** Out );
	void         (*ArchiveCountBytes)( void* Ar, size_t Num, size_t Max );
	void         (*Debug)( unsigned int Kind, const char* Msg );
};

extern const Hp2DynBspServices GHp2DynBspServices;

// Rust exports (crates/hp2-ffi/src/engine_dynbsp.rs). The tracker handle is
// owned by FMovingBrushTracker and freed in its destructor.
extern "C" void* hp2_rs_dynbsp_new( const Hp2DynBspServices* Svc, void* Level, void* Model );
extern "C" void  hp2_rs_dynbsp_free( void* Tracker );
extern "C" void  hp2_rs_dynbsp_flush( void* Tracker, void* Actor );
extern "C" void  hp2_rs_dynbsp_update( void* Tracker, void* Actor );
extern "C" int   hp2_rs_dynbsp_surf_is_dynamic( void* Tracker, int ISurf );
extern "C" void  hp2_rs_dynbsp_count_bytes( void* Tracker, const Hp2DynBspServices* Svc, void* Ar );

#endif

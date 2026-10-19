/*=============================================================================
	Hp2ActColServices.h: C++ services for the Rust FCollisionHash port.

	Consumed by crates/hp2-engine/src/actcol.rs (`Hp2ActColServices`, #[repr(C)]).
	Field order must match the Rust struct exactly. Pointers are void* here
	and are cast to the real types in Hp2ActColServices.cpp.
=============================================================================*/

#ifndef _INC_HP2ACTCOLSERVICES_H
#define _INC_HP2ACTCOLSERVICES_H

struct Hp2ActColServices
{
	int   (*AppRand)( void );
	void  (*ActorCollisionBoundingBox)( void* Actor, void* OutBox );
	void  (*ActorCylinderExtent)( const void* Actor, void* OutExtent );
	int   (*ActorIsMovingBrush)( const void* Actor );
	int   (*PrimitivePointCheck)( void* Owner, void* Res, const void* Location, const void* Extent );
	int   (*PrimitiveLineCheck)( void* Actor, void* Res, const void* End, const void* Start, const void* Size, unsigned int Flags );
	void* (*AllocCheckResult)( void* Mem );
	void  (*DebugfMovedWithoutHashing)( const void* Actor );
	void  (*CheckFailed)( const char* Expr, const char* File, int Line );
};

extern const Hp2ActColServices GHp2ActColServices;

// Rust exports (crates/hp2-ffi/src/engine_actcol.rs). Check results are
// FCheckResult* list heads (null = no hits).
extern "C" void* hp2_rs_actcol_new( const Hp2ActColServices* Svc );
extern "C" void  hp2_rs_actcol_free( void* Hash );
extern "C" void  hp2_rs_actcol_tick( void* Hash );
extern "C" void  hp2_rs_actcol_add_actor( void* Hash, const Hp2ActColServices* Svc, void* Actor );
extern "C" void  hp2_rs_actcol_remove_actor( void* Hash, const Hp2ActColServices* Svc, void* Actor, int bThorough );
extern "C" void* hp2_rs_actcol_actor_point_check( void* Hash, const Hp2ActColServices* Svc, const void* Location, const void* Extent );
extern "C" void* hp2_rs_actcol_actor_radius_check( void* Hash, const Hp2ActColServices* Svc, const void* Location, float Radius );
extern "C" void* hp2_rs_actcol_actor_encroachment_check( void* Hash, const Hp2ActColServices* Svc, void* Actor, const void* Location, const void* Rotation );
extern "C" void* hp2_rs_actcol_actor_line_check( void* Hash, const Hp2ActColServices* Svc, void* Mem, const void* End, const void* Start, const void* Extent, unsigned int ExtraNodeFlags );

#endif

/*=============================================================================
	Hp2MoverServices.h: C++ services for the Rust AMover port.

	Consumed by crates/hp2-engine/src/mover.rs (`Hp2MoverServices`, #[repr(C)]).
	Field order and the enum values below must match the Rust side exactly.
=============================================================================*/

#ifndef _INC_HP2MOVERSERVICES_H
#define _INC_HP2MOVERSERVICES_H

// Mirrors mover.rs `PropKind`.
enum Hp2MoverPropKind
{
	HP2_MOVER_PROP_BYTE   = 0,
	HP2_MOVER_PROP_FLOAT  = 1,
	HP2_MOVER_PROP_INT    = 2,
	HP2_MOVER_PROP_NAME   = 3,
	HP2_MOVER_PROP_OBJECT = 4,
	HP2_MOVER_PROP_STRUCT = 5,
	HP2_MOVER_PROP_BOOL   = 6,
};

// Mirrors mover.rs `ClassRef`.
enum Hp2MoverClassRef
{
	HP2_MOVER_CLASS_MOVER           = 0,
	HP2_MOVER_CLASS_ACTOR           = 1,
	HP2_MOVER_CLASS_SOUND           = 2,
	HP2_MOVER_CLASS_NAVIGATION_POINT = 3,
	HP2_MOVER_CLASS_PAWN            = 4,
	HP2_MOVER_CLASS_TRIGGER         = 5,
};

struct Hp2MoverServices
{
	void* (*FindStruct)( const char* Name );
	void* (*NewStruct)( const char* Name, int Size );
	void  (*LinkStruct)( void* Struct );
	void* (*NewEnum)( const char* Name );
	void  (*EnumAddName)( void* Enum, const char* Name );
	void* (*ClassRef)( int Id );
	void  (*AddProperty)( void* Owner, int Kind, const char* Name, const char* Category, int Offset, int ArrayDim, unsigned int BitMask, void* RefObj );
	void  (*HashAdd)( void* Hash, void* Actor );
	void  (*HashRemove)( void* Hash, void* Actor );
	void  (*BrushTrackerUpdate)( void* Tracker, void* Actor );
	void  (*BrushTrackerFlush)( void* Tracker, void* Actor );
	void  (*RenumberBrushPolys)( void* Brush );
};

extern const Hp2MoverServices GHp2MoverServices;

// Rust exports (crates/hp2-ffi/src/engine_mover.rs).
extern "C" void hp2_rs_mover_static_constructor( const Hp2MoverServices* Svc );
extern "C" void hp2_rs_mover_spawned( void* Mover, const Hp2MoverServices* Svc );
extern "C" void hp2_rs_mover_post_load( void* Mover, const Hp2MoverServices* Svc );
extern "C" void hp2_rs_mover_post_edit_move( void* Mover, const Hp2MoverServices* Svc );
extern "C" void hp2_rs_mover_post_edit_change( void* Mover, const Hp2MoverServices* Svc );
extern "C" void hp2_rs_mover_pre_raytrace( void* Mover, const Hp2MoverServices* Svc );
extern "C" void hp2_rs_mover_set_world_raytrace_key( void* Mover, const Hp2MoverServices* Svc );
extern "C" void hp2_rs_mover_set_brush_raytrace_key( void* Mover, const Hp2MoverServices* Svc );
extern "C" void hp2_rs_mover_post_raytrace( void* Mover, const Hp2MoverServices* Svc );

#endif

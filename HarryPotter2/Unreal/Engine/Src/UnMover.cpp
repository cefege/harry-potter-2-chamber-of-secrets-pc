/*=============================================================================
	UnMover.cpp: Keyframe mover actor code
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney

	AMover logic and its property table live in crates/hp2-engine/src/mover.rs.
	These are thin shells: base-class calls stay here, in their original order.
=============================================================================*/

#include "EnginePrivate.h"
#include "Hp2MoverServices.h"

/*-----------------------------------------------------------------------------
	AMover implementation.
-----------------------------------------------------------------------------*/

AMover::AMover()
{}
void AMover::StaticConstructor()
{
	guard(AMover::StaticConstructor);
	hp2_rs_mover_static_constructor( &GHp2MoverServices );
	unguard;
}
void AMover::Spawned()
{
	guard(AMover::Spawned);
	ABrush::Spawned();
	hp2_rs_mover_spawned( this, &GHp2MoverServices );
	unguard;
}
void AMover::PostLoad()
{
	guard(AMover::PostLoad);
	AActor::PostLoad();
	hp2_rs_mover_post_load( this, &GHp2MoverServices );
	unguard;
}
void AMover::PostEditMove()
{
	guard(AMover::PostEditMove);
	ABrush::PostEditMove();
	hp2_rs_mover_post_edit_move( this, &GHp2MoverServices );
	unguard;
}
void AMover::PostEditChange()
{
	guard(AMover::PostEditChange);
	ABrush::PostEditChange();
	hp2_rs_mover_post_edit_change( this, &GHp2MoverServices );
	unguard;
}
void AMover::PreRaytrace()
{
	guard(AMover::PreRaytrace);
	ABrush::PreRaytrace();
	hp2_rs_mover_pre_raytrace( this, &GHp2MoverServices );
	unguard;
}
void AMover::SetWorldRaytraceKey()
{
	guard(AMover::SetWorldRaytraceKey);
	hp2_rs_mover_set_world_raytrace_key( this, &GHp2MoverServices );
	unguard;
}
void AMover::SetBrushRaytraceKey()
{
	guard(AMover::SetBrushRaytraceKey);
	hp2_rs_mover_set_brush_raytrace_key( this, &GHp2MoverServices );
	unguard;
}
void AMover::PostRaytrace()
{
	guard(AMover::PostRaytrace);
	ABrush::PostRaytrace();
	hp2_rs_mover_post_raytrace( this, &GHp2MoverServices );
	unguard;
}
IMPLEMENT_CLASS(AMover);

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/

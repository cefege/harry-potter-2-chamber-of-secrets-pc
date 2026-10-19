/*=============================================================================
	UnViewport.cpp: Generic Unreal viewport code
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"
#include "UnRender.h"
#include "UnNet.h"
#include "hp2_rs.h"
#include "Hp2CameraServices.h"

/*-----------------------------------------------------------------------------
	URenderDevice.
-----------------------------------------------------------------------------*/

static void CameraRaiseFault( INT Rc )
{
	// Rust returns fault codes (camera_exec::FAULT_*) instead of raising. The shell raises them
	// so the message and __FILE__/__LINE__ match the original TU (lines 702, 888, 900).
	switch( Rc )
	{
		case 2:
			appErrorf( TEXT("Negative bitmap dimension") );
			break;
		case 3:
			appErrorf( TEXT("Bitmap dimensions exceed the signed 32-bit buffer limit") );
			break;
		case 4:
			appErrorf( TEXT("Bitmap dimensions exceed the 32-bit file format") );
			break;
		case 5:
			appFailAssert( "!appIsNan(ShrinkageValue) && ShrinkageValue>=1.0 && ShrinkageValue<=MAXINT", __FILE__, 702 );
			break;
		case 6:
			appFailAssert( "widthFactor<31", __FILE__, 888 );
			break;
		case 7:
			appFailAssert( "heightFactor<31", __FILE__, 900 );
			break;
		default:
			appErrorf( TEXT("unreachable fault code %i"), Rc );
			break;
	}
}

//
// Init URenderDevice class.
//
void URenderDevice::StaticConstructor()
{
	guard(URenderDevice::StaticConstructor);

	new(GetClass(),TEXT("VolumetricLighting"),RF_Public)UBoolProperty(CPP_PROPERTY(VolumetricLighting), TEXT("Client"), CPF_Config );
	new(GetClass(),TEXT("ShinySurfaces"),     RF_Public)UBoolProperty(CPP_PROPERTY(ShinySurfaces     ), TEXT("Client"), CPF_Config );
	new(GetClass(),TEXT("Coronas"),           RF_Public)UBoolProperty(CPP_PROPERTY(Coronas           ), TEXT("Client"), CPF_Config );
	new(GetClass(),TEXT("HighDetailActors"),  RF_Public)UBoolProperty(CPP_PROPERTY(HighDetailActors  ), TEXT("Client"), CPF_Config );
	new(GetClass(),TEXT("Description"),       RF_Public)UStrProperty(CPP_PROPERTY(Description        ), TEXT("Client"), CPF_Config );
	new(GetClass(),TEXT("DescFlags"),         RF_Public)UIntProperty(CPP_PROPERTY(DescFlags          ), TEXT("Client"), CPF_Config );
	new(GetClass(),TEXT("DetailTextures"),    RF_Public)UBoolProperty(CPP_PROPERTY(DetailTextures     ), TEXT("Client"), CPF_Config );

	DecompFormat = TEXF_P8;

	unguard;
}

//
// Draw a clipped 2D line.
//
void URenderDevice::Draw2DClippedLine
(
	FSceneNode*		Frame,
	FPlane			Color,
	DWORD			LineFlags,
	FVector			P1,
	FVector			P2
)
{
	guard(URenderDevice::Draw2DClippedLine);
	hp2_rs_engine_camera_draw_2d_clipped_line( &GHp2CameraServices, this, Frame, &Color, LineFlags, &P1, &P2 );
	unguard;
}

UBOOL URenderDevice::Exec( const TCHAR* Cmd, FOutputDevice& Ar )
{
	guard(UVRenderDevice::Exec);
	return hp2_rs_engine_camera_render_dev_exec( &GHp2CameraServices, this, Cmd, &Ar );
	unguard;
}

//
// Draw a clipped 3D line.
//
void URenderDevice::Draw3DLine
(
	FSceneNode*		Frame,
	FPlane			Color,
	DWORD			LineFlags,
	FVector			P1,
	FVector			P2
)
{
	guard(URenderDevice::Draw3DLine);
	hp2_rs_engine_camera_draw_3d_line( &GHp2CameraServices, this, Frame, &Color, LineFlags, &P1, &P2 );
	unguard;
}

void URenderDevice::DrawGouraudPolygon( FSceneNode* Frame, FTextureInfo& Info, FTransTexture** Pts, int NumPts, DWORD PolyFlags, FSpanBuffer* SpanBuffer )
{
	// Set up an indexed triangle list for a single polygon.
	DrawTriangles( Frame, Info, Pts, NumPts, NULL, NumPts, PolyFlags, SpanBuffer );
}

/*-----------------------------------------------------------------------------
	UViewport object implementation.
-----------------------------------------------------------------------------*/

void UViewport::Serialize( FArchive& Ar )
{
	guard(UViewport::Serialize);
	Super::Serialize( Ar );
	Ar << Console << MiscRes << Canvas << RenDev << Input;
	unguard;
}
void* UViewport::GetServer()
{
	guard(UViewport::GetServer);
	return NULL;
	unguard;
}
IMPLEMENT_CLASS(UViewport);

/*-----------------------------------------------------------------------------
	Dragging.
-----------------------------------------------------------------------------*/

//
// Set mouse dragging.
// The mouse is dragging when and only when one or more button is held down.
//
UBOOL UViewport::SetDrag( UBOOL NewDrag )
{
	guard(UViewport::SetDrag);
	return hp2_rs_engine_camera_set_drag( &GHp2CameraServices, this, NewDrag );
	unguard;
}

/*-----------------------------------------------------------------------------
	Scene frames.
-----------------------------------------------------------------------------*/

//
// Precompute rendering info.
//
void FSceneNode::ComputeRenderCoords( FVector& Location, FRotator& Rotation )
{
	guard(UViewport::ComputeRenderCoords);
	check(Viewport->Actor);
	hp2_rs_engine_camera_compute_render_coords( &GHp2CameraServices, this, &Location, &Rotation );
	unguard;
}

//
// Precompute sizing info.
//
void FSceneNode::ComputeRenderSize()
{
	guard(FSceneNode::ComputeRenderSize);
	hp2_rs_engine_camera_compute_render_size( &GHp2CameraServices, this );
	unguard;
}

/*-----------------------------------------------------------------------------
	Custom viewport creation and destruction.
-----------------------------------------------------------------------------*/

//
// UViewport constructor.  Creates the viewport object but doesn't open its window.
//
UViewport::UViewport()
{
	guard(UViewport::UViewport);

	// Update viewport array.
	GetOuterUClient()->Viewports.AddItem( this );

	// Create canvas.
	UClass* CanvasClass = StaticLoadClass( UCanvas::StaticClass(), NULL, TEXT("ini:Engine.Engine.Canvas"), NULL, LOAD_NoFail, NULL );
	Canvas = CastChecked<UCanvas>(StaticConstructObject( CanvasClass ));
	Canvas->Init( this );

	// Create input system.
	UClass* InputClass = StaticLoadClass( UInput::StaticClass(), NULL, TEXT("ini:Engine.Engine.Input"), NULL, LOAD_NoFail, NULL );
	Input = (UInput*)StaticConstructObject( InputClass );

	// Set initial time.
	LastUpdateTime = appSeconds();

	unguard;
}

//
// Close a viewport.
//warning: Lots of interrelated stuff is happening here between the viewport code,
// object code, platform-specific viewport manager code, and Windows.
//
void UViewport::Destroy()
{
	guard(UViewport::Destroy);

	// Temporary for editor!!
	if( GetOuterUClient()->Engine->Audio && GetOuterUClient()->Engine->Audio->GetViewport()==this )
		GetOuterUClient()->Engine->Audio->SetViewport( NULL );

	// Close the viewport window.
	guard(CloseWindow);
	CloseWindow();
	unguard;

	#if _MSC_VER

	// Delete the input subsystem.
	guard(ExitInput);
	delete Input;
	unguard;

	// Delete the console.
	guard(DeleteConsole);
	if( Console )
		delete Console;
	unguard;

	// Delete the canvas.
	guard(DeleteCanvas);
	delete Canvas;
	unguard;

	// Shut down rendering.
	guard(DeleteRenDev);
	if( RenDev )
	{
		RenDev->Exit();
		delete RenDev;
	}
	unguard;

	#endif

	// Remove from viewport list.
	GetOuterUClient()->Viewports.RemoveItem( this );

	Super::Destroy();
	unguardobj;
}

/*---------------------------------------------------------------------------------------
	Viewport information functions.
---------------------------------------------------------------------------------------*/

//
// Is this camrea a wireframe view?
//
UBOOL UViewport::IsWire()
{
	guard(UViewport::IsWire);
	return hp2_rs_engine_camera_is_wire( &GHp2CameraServices, this );
	unguard;
}

/*-----------------------------------------------------------------------------
	Viewport locking & unlocking.
-----------------------------------------------------------------------------*/

//
// Lock the viewport for rendering.
//
UBOOL UViewport::Lock( FPlane FlashScale, FPlane FlashFog, FPlane ScreenClear, DWORD RenderLockFlags, BYTE* HitData, INT* HitSize )
{
	guard(UViewport::Lock);
	check(RenDev);
	return hp2_rs_engine_camera_lock( &GHp2CameraServices, this, &FlashScale, &FlashFog, &ScreenClear, RenderLockFlags, HitData, HitSize );
	unguard;
}

//
// Unlock the viewport.
//
void UViewport::Unlock( UBOOL Blit )
{
	guard(UViewport::Unlock);
	check(RenDev);
	check(HitSizes.Num()==0);
	hp2_rs_engine_camera_unlock( &GHp2CameraServices, this, Blit );
	unguard;
}

/*-----------------------------------------------------------------------------
	Command line.
-----------------------------------------------------------------------------*/

//
// UViewport command line.
//
UBOOL UViewport::Exec( const TCHAR* Cmd, FOutputDevice& Ar )
{
	guard(UViewport::Exec);
	check(Actor);
	INT Rc = hp2_rs_engine_camera_exec( &GHp2CameraServices, this, Cmd, &Ar );
	if( Rc >= 2 )
	{
		CameraRaiseFault( Rc );
		return 1;
	}
	return Rc;
	unguard;
}

//
// Execute a macro on this viewport.
//
void UViewport::ExecMacro( const TCHAR* Filename, FOutputDevice& Ar )
{
	guard(UViewport::ExecMacro);
	hp2_rs_engine_camera_exec_macro( &GHp2CameraServices, this, Filename, &Ar );
	unguard;
}

/*-----------------------------------------------------------------------------
	UViewport FArchive interface.
-----------------------------------------------------------------------------*/

//
// Output a message on the viewport's console.
//
void UViewport::Serialize( const TCHAR* Data, EName MsgType )
{
	guard(UViewport::Serialize);

	// Pass to console.
	if( Console )
		Console->Serialize( Data, MsgType );

	unguard;
}

/*-----------------------------------------------------------------------------
	Input.
-----------------------------------------------------------------------------*/

//
// Read input from the viewport.
//
void UViewport::ReadInput( FLOAT DeltaSeconds )
{
	guard(UViewport::ReadInput);
	check(Input);
	hp2_rs_engine_camera_read_input( &GHp2CameraServices, this, DeltaSeconds );
	unguard;
}

/*-----------------------------------------------------------------------------
	Viewport hit testing.
-----------------------------------------------------------------------------*/

//
// Viewport hit-test pushing.
//
void UViewport::PushHit( const HHitProxy& Hit, INT Size )
{
	guard(UViewport::PushHit);
	hp2_rs_engine_camera_push_hit( &GHp2CameraServices, this, (void*)&Hit, Size );
	unguard;
}

//
// Pop the most recently pushed hit.
//
void UViewport::PopHit( UBOOL bForce )
{
	guard(UViewport::PopHit);
	checkSlow(RenDev);
	checkSlow(HitTesting);
	check(HitSizes.Num());
	hp2_rs_engine_camera_pop_hit( &GHp2CameraServices, this, bForce );
	unguard;
}

//
// Execute all hits in the hit buffer.
//
void UViewport::ExecuteHits( const FHitCause& Cause, BYTE* HitData, INT HitSize )
{
	guard(UViewport::ExecuteHits);

	// String together the hit stack and return the innermost hit (Rust sets each Parent and
	// leaves the unlinked remainder in HitSize).
	HHitProxy* TopHit = NULL;
	hp2_rs_engine_camera_link_hits( &GHp2CameraServices, HitData, &HitSize, (void**)&TopHit );
	check(HitSize==0);

	// Call the innermost hit.
	if( TopHit )
		TopHit->Click( Cause );

	unguard;
}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/

/*=============================================================================
	UnViewport.cpp: Generic Unreal viewport code
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"
#include "UnRender.h"
#include "UnNet.h"
#include <cstdint>

/*-----------------------------------------------------------------------------
	URenderDevice.
-----------------------------------------------------------------------------*/

#define LINE_NEAR_CLIP_Z 1.f

static void WriteUInt16LE( FArchive& Ar, std::uint16_t Value )
{
	BYTE Bytes[2] =
	{
		static_cast<BYTE>(Value),
		static_cast<BYTE>(Value >> 8)
	};
	Ar.Serialize( Bytes, 2 );
}

static void WriteUInt32LE( FArchive& Ar, std::uint32_t Value )
{
	BYTE Bytes[4] =
	{
		static_cast<BYTE>(Value),
		static_cast<BYTE>(Value >> 8),
		static_cast<BYTE>(Value >> 16),
		static_cast<BYTE>(Value >> 24)
	};
	Ar.Serialize( Bytes, 4 );
}

static std::uint32_t CheckedBitmapDimension( INT Dimension )
{
	if( Dimension < 0 )
		appErrorf( TEXT("Negative bitmap dimension") );
	return static_cast<std::uint32_t>(Dimension);
}
static INT CheckedBitmapPixelCount( INT Width, INT Height )
{
	if( Width < 0 || Height < 0 || (Width > 0 && Height > MAXINT / Width) )
		appErrorf( TEXT("Bitmap dimensions exceed the signed 32-bit buffer limit") );
	return Width * Height;
}


static void WriteBitmapHeaders( FArchive& Ar, std::uint32_t Width, std::uint32_t Height )
{
	const std::uint32_t FileHeaderSize = 14;
	const std::uint32_t InfoHeaderSize = 40;
	const std::uint64_t PixelBytes64 = UINT64_C(3) * Width * Height;
	const std::uint64_t FileBytes64 = FileHeaderSize + InfoHeaderSize + PixelBytes64;
	if( PixelBytes64 > UINT32_MAX || FileBytes64 > UINT32_MAX )
		appErrorf( TEXT("Bitmap dimensions exceed the 32-bit file format") );
	const std::uint32_t PixelBytes = static_cast<std::uint32_t>(PixelBytes64);

	WriteUInt16LE( Ar, 0x4d42 );
	WriteUInt32LE( Ar, static_cast<std::uint32_t>(FileBytes64) );
	WriteUInt16LE( Ar, 0 );
	WriteUInt16LE( Ar, 0 );
	WriteUInt32LE( Ar, FileHeaderSize + InfoHeaderSize );

	WriteUInt32LE( Ar, InfoHeaderSize );
	WriteUInt32LE( Ar, Width );
	WriteUInt32LE( Ar, Height );
	WriteUInt16LE( Ar, 1 );
	WriteUInt16LE( Ar, 24 );
	WriteUInt32LE( Ar, 0 );
	WriteUInt32LE( Ar, PixelBytes );
	WriteUInt32LE( Ar, 0 );
	WriteUInt32LE( Ar, 0 );
	WriteUInt32LE( Ar, 0 );
	WriteUInt32LE( Ar, 0 );
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

	// X clip it.
	if( P1.X > P2.X )
		Exchange( P1, P2 );
	if( P2.X<0 || P1.X>Frame->FX )
		return;
	if( P1.X<0 )
	{
		if( Abs(P2.X-P1.X)<0.001f )
			return;
		P1.Y += (0-P1.X)*(P2.Y-P1.Y)/(P2.X-P1.X);
		P1.X  = 0;
	}
	if( P2.X>=Frame->FX )
	{
		if( Abs(P2.X-P1.X)<0.001f )
			return;
		P2.Y += ((Frame->FX-1.f)-P2.X)*(P2.Y-P1.Y)/(P2.X-P1.X);
		P2.X  = Frame->FX-1.f;
	}

	// Y clip it.
	if( P1.Y > P2.Y )
		Exchange( P1, P2 );
	if( P2.Y < 0 || P1.Y > Frame->FY )
		return;
	if( P1.Y < 0 )
	{
		if( Abs(P2.Y-P1.Y)<0.001f )
			return;
		P1.X += (0-P1.Y)*(P2.X-P1.X)/(P2.Y-P1.Y);
		P1.Y  = 0;
	}
	if( P2.Y >= Frame->FY )
	{
		if( Abs(P2.Y-P1.Y)<0.001f )
			return;
		P2.X += ((Frame->FY-1.f)-P2.Y)*(P2.X-P1.X)/(P2.Y-P1.Y);
		P2.Y  = Frame->FY-1.f;
	}

	// Assure no coordinates are out of bounds. 
	ClipFloatFromZero(P1.X,Frame->FX);
	ClipFloatFromZero(P2.X,Frame->FX);
	ClipFloatFromZero(P1.Y,Frame->FY);
	ClipFloatFromZero(P2.Y,Frame->FY);

	// Draw it.
	Draw2DLine( Frame, Color, LineFlags, P1, P2 );

	unguard;
}

UBOOL URenderDevice::Exec( const TCHAR* Cmd, FOutputDevice& Ar )
{
	guard(UVRenderDevice::Exec);

	if( ParseCommand(&Cmd,TEXT("GetDetailTextures")) )
	{
		Ar.Logf( TEXT("%i"), DetailTextures );
		return 1;
	} 
	else if( ParseCommand(&Cmd,TEXT("ToggleDetailTextures")) )
	{
		DetailTextures ^= 1;
		SaveConfig();
		return 1;
	}
	else
		return 0;

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
	FLOAT SX2 = Frame->FX2;
	FLOAT SY2 = Frame->FY2;

	// Transform.
	if( !(LineFlags & LINE_PreTransformed) )
	{
		P1 = P1.TransformPointBy( Frame->Coords );
		P2 = P2.TransformPointBy( Frame->Coords );
	}
	if( Frame->Viewport->IsOrtho() )
	{
		// Zoom.
		P1 = P1 / Frame->Zoom + FVector( SX2, SY2, 0 );
		P2 = P2 / Frame->Zoom + FVector( SX2, SY2, 0 );

		// See if points form a line parallel to our line of sight (i.e. line appears as a dot).
		if( Abs(P2.X-P1.X)+Abs(P1.Y-P2.Y)<0.2 )
		{
			// Line is visible as a point.
			if( Frame->Viewport->Actor->OrthoZoom < ORTHO_LOW_DETAIL )
				Draw2DPoint( Frame, Color, LINE_None, P1.X-1, P1.Y-1, P1.X+1, P1.Y+1, 1.f );
			return;
		}
	}
	else
	{
		// Calculate delta, discard line if points are identical.
		FVector D = P2-P1;
		if( D.SizeSquared() < Square(0.01f) )
			return;

		// Clip to near clipping plane.
		if( P1.Z <= LINE_NEAR_CLIP_Z )
		{
			// Clip P1 to NCP.
			if( P2.Z<(LINE_NEAR_CLIP_Z-0.01f) )
				return;
			P1.X +=  (LINE_NEAR_CLIP_Z-P1.Z) * D.X/D.Z;
			P1.Y +=  (LINE_NEAR_CLIP_Z-P1.Z) * D.Y/D.Z;
			P1.Z  =  (LINE_NEAR_CLIP_Z);
		}
		else if( P2.Z<(LINE_NEAR_CLIP_Z-0.01f) )
		{
			// Clip P2 to NCP.
			P2.X += (LINE_NEAR_CLIP_Z-P2.Z) * D.X/D.Z;
			P2.Y += (LINE_NEAR_CLIP_Z-P2.Z) * D.Y/D.Z;
			P2.Z =  (LINE_NEAR_CLIP_Z);
		}

		// Calculate perspective.
		P1.Z = 1.f/P1.Z; P1.X = P1.X * Frame->Proj.Z * P1.Z + SX2; P1.Y = P1.Y * Frame->Proj.Z * P1.Z + SY2;
		P2.Z = 1.f/P2.Z; P2.X = P2.X * Frame->Proj.Z * P2.Z + SX2; P2.Y = P2.Y * Frame->Proj.Z * P2.Z + SY2;
	}

	// Clip it and draw it.
	Draw2DClippedLine( Frame, Color, LineFlags, P1, P2 );

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
	UBOOL Result = Dragging;
	Dragging = NewDrag;
	if( GIsRunning )
	{
		if( Dragging && !Result )
		{
			// First hit.
			GetOuterUClient()->Engine->MouseDelta( this, MOUSE_FirstHit, 0, 0 );
		}
		else if( Result && !Dragging )
		{
			// Last release.
			GetOuterUClient()->Engine->MouseDelta( this, MOUSE_LastRelease, 0, 0 );
		}
	}
	return Result;
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
	Coords =
	(	Viewport->Actor->RendMap==REN_OrthXY ? FCoords(FVector(0,0,0),FVector(+1,0,0),FVector(0,+1,0),FVector(0,0,+1))
	:	Viewport->Actor->RendMap==REN_OrthXZ ? FCoords(FVector(0,0,0),FVector(+1,0,0),FVector(0,0,-1),FVector(0,+1,0))
	:	Viewport->Actor->RendMap==REN_OrthYZ ? FCoords(FVector(0,0,0),FVector(0,+1,0),FVector(0,0,-1),FVector(+1,0,0))
	:								           GMath.ViewCoords / Rotation) / Location;
	Uncoords = Coords.Transpose();
	ComputeRenderSize();
	unguard;
}

//
// Precompute sizing info.
//
void FSceneNode::ComputeRenderSize()
{
	guard(FSceneNode::ComputeRenderSize);

	// Precomputes.
	FX 			= (FLOAT)X;
	FY 			= (FLOAT)Y;
	FX2			= FX * 0.5f;
	FY2			= FY * 0.5f;	
	FX15		= (FX+1.0001f) * 0.5f;
	FY15		= (FY+1.0001f) * 0.5f;
	EffectiveFovAngle = Viewport->Actor->FovAngle;
	if( !Viewport->IsOrtho() )
	{
		const UClient* Client = Viewport->GetOuterUClient();
		EffectiveFovAngle = UClient::GetEffectiveFovAngle( EffectiveFovAngle, X, Y, Client->MaintainVerticalFOV );
	}
	Proj		= FVector( 0.5f-0.5f*FX, 0.5f-0.5f*FY, 0.5f*FX / appTan(EffectiveFovAngle * PI/360.f) );
	RProj		= FVector( 1/Proj.X, 1/Proj.Y, 1/Proj.Z );
	Zoom 		= Viewport->Actor->OrthoZoom / (FX * 15.f);
	PrjXM		= (0  - FX2)*(-RProj.Z);
	PrjXP		= (FX - FX2)*(+RProj.Z);
	PrjYM		= (0  - FY2)*(-RProj.Z);
	PrjYP		= (FY - FY2)*(+RProj.Z);

	// Precompute side info.
	FLOAT TempSigns[2]={-1.f,+1.f};
	for( INT i=0; i<2; i++ )
	{
		for( INT j=0; j<2; j++ )
		{
			ViewSides[i*2+j] = FVector(TempSigns[i] * FX2, TempSigns[j] * FY2, Proj.Z).UnsafeNormal().TransformVectorBy(Uncoords);
		}
		ViewPlanes[i] = FPlane
		(
			Coords.Origin,
			FVector(0,TempSigns[i] / FY2,1.f/Proj.Z).UnsafeNormal().TransformVectorBy(Uncoords)
		);
		ViewPlanes[i+2] = FPlane
		(
			Coords.Origin,
			FVector(TempSigns[i] / FX2,0,1.f/Proj.Z).UnsafeNormal().TransformVectorBy(Uncoords)
		);
	}

	// Tell rendering device.
	Viewport->RenDev->SetSceneNode( this );

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
	return
	Actor &&
	(	Actor->GetLevel()->Model->Nodes.Num()==0
	||	Actor->RendMap==REN_OrthXY
	||	Actor->RendMap==REN_OrthXZ
	||	Actor->RendMap==REN_OrthYZ
	||	Actor->RendMap==REN_Wire );

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

	// Set info.
	CurrentTime = appSeconds();
	HitTesting = HitData!=NULL;
	RenderFlags = RenderLockFlags;
	ExtraPolyFlags = Actor && ((Actor->RendMap==REN_PolyCuts) || (Actor->RendMap==REN_Zones)) ? PF_NoMerge : 0;
	FrameCount++;

	// Lock rendering device.
	RenDev->Lock( FlashScale, FlashFog, ScreenClear, RenderLockFlags, HitData, HitSize );

	// Successfully locked it.
	return 1;
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

	// Unlock rendering device.
	RenDev->Unlock( Blit );

	// Update time.
	if( Blit )
		LastUpdateTime = CurrentTime;

	unguard;
}

UBOOL  UViewport::ScreenShot( const TCHAR* Filename )
{
	FMemMark Mark(GMem);
	FColor* Buf = new(GMem,CheckedBitmapPixelCount(SizeX, SizeY))FColor;
	RenDev->ReadPixels( Buf );
	FArchive* Ar = GFileManager->CreateFileWriter( Filename );
	if( Ar )
	{
		// BMP file and info headers have fixed 14-byte and 40-byte wire layouts.
		WriteBitmapHeaders
		(
			*Ar,
			CheckedBitmapDimension(SizeX),
			CheckedBitmapDimension(SizeY)
		);

		// Colors.
		for( INT i=SizeY-1; i>=0; i-- )
			for( INT j=0; j<SizeX; j++ )
				Ar->Serialize( &Buf[i*SizeX+j], 3 );

		// Success.
		delete Ar;
	}
	Mark.Pop();
	return 1;
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
	if( Input && Input->Exec(Cmd,Ar) )
	{
		return 1;
	}
	else if( RenDev && RenDev->Exec(Cmd,Ar) )
	{
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("GETPING")) )
	{
		Ar.Logf( TEXT("0") );
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("INJECT")) )
	{
		UNetDriver* Driver = Actor->GetLevel()->NetDriver;
		if( Driver && Driver->ServerConnection )
		{
			Ar.Logf(TEXT("Injecting: %s"),Cmd);
			Driver->ServerConnection->Logf( Cmd );
		}
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("NETSPEED")) )
	{
		INT Rate = appAtoi(Cmd);
		UNetDriver* Driver = Actor->GetLevel()->NetDriver;
		GetDefault<UPlayer>()->ConfiguredInternetSpeed = Rate;
		GetDefault<UPlayer>()->SaveConfig();
		if( Rate>=500 && Driver && Driver->ServerConnection )
		{
			if( !Driver->ServerConnection->URL.HasOption(TEXT("LAN")) )
			{
				CurrentNetSpeed = Driver->ServerConnection->CurrentNetSpeed = Clamp( Rate, 500, Driver->MaxClientRate );
				Driver->ServerConnection->Logf( TEXT("NETSPEED %i"), Rate );
			}
		}
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("LANSPEED")) )
	{
		INT Rate = appAtoi(Cmd);
		UNetDriver* Driver = Actor->GetLevel()->NetDriver;
		GetDefault<UPlayer>()->ConfiguredInternetSpeed = Rate;
		GetDefault<UPlayer>()->SaveConfig();
		if( Rate>=500 && Driver && Driver->ServerConnection )
		{
			if( Driver->ServerConnection->URL.HasOption(TEXT("LAN")) )
			{
				CurrentNetSpeed = Driver->ServerConnection->CurrentNetSpeed = Clamp( Rate, 500, Driver->MaxClientRate );
				Driver->ServerConnection->Logf( TEXT("NETSPEED %i"), Rate );
			}
		}
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("SHOWALL")) )
	{
		for( INT i=0; i<Actor->GetLevel()->Actors.Num(); i++ )
			if( Actor->GetLevel()->Actors(i) )
				Actor->GetLevel()->Actors(i)->bHidden = 0;
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("REPORT")) )
	{
		FStringOutputDevice Str;
#if 1 //Logging added by Legend on 4/12/2000
		Str.Logf ( TEXT("Report: %s\r\n"), Cmd );
#else
		Str.Log ( TEXT("Report:\r\n") );
#endif
		Str.Logf( TEXT("   Version: %s %s\r\n"), appFromAnsi(__DATE__), appFromAnsi(__TIME__) );
		Str.Logf( TEXT("   Player class: %s\r\n"), Actor->GetClass()->GetName() );
		Str.Logf( TEXT("   URL: %s\r\n"), *Actor->GetLevel()->URL.String() );
		Str.Logf( TEXT("   Location: %i %i %i\r\n"), appRound(Actor->Location.X), appRound(Actor->Location.Y), appRound(Actor->Location.Z) );
		if( Actor->Level->Game==NULL )
		{
			Str.Logf( TEXT("   Network client\r\n") );
		}
		else
		{
			Str.Logf( TEXT("   Game class: %s\r\n"), Actor->Level->Game->GetClass()->GetName() );
			Str.Logf( TEXT("   Difficulty: %i\r\n"), Actor->Level->Game->Difficulty );
		}
#if 1 //Logging added by Legend on 4/12/2000
		Ar.Logf( *Str, Cmd );
#endif
		appClipboardCopy( *Str );
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("SHOT")) )
	{
		// Screenshot.
		TCHAR File[512];
		if( ParseToken( Cmd, File, ARRAY_COUNT(File), 0 ) )
		{
			// user specified a filename so save in SaveSlotPath
			return ScreenShot( *(GSys->SaveSlotPath * File) );
		}
		else
		{
			// default ScreenShot
			for( INT i=0; i<256; i++ )
			{
				appSprintf( File, TEXT("%sShot%04i.bmp"), appUserDir(), i );
				if( GFileManager->FileSize(File) < 0 )
					break;
			}
			if( GFileManager->FileSize(File) < 0 )
				return ScreenShot( File );
		}
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("SNAP")) )
	{
		// Take a copy of the viewport image, shrink and store.
		//----------------------------------------------------
		INT shrinkFactor = appAtoi(Cmd);
		const DOUBLE ShrinkageValue = appPow(2, shrinkFactor);
		check( !appIsNan(ShrinkageValue) && ShrinkageValue>=1.0 && ShrinkageValue<=MAXINT );
		INT shrinkage = static_cast<INT>(ShrinkageValue);

		snapU = SizeX / shrinkage;
		snapV = SizeY / shrinkage;

		FPushMemTag Tag(TEXT("Snap"));
		FMemMark Mark(GMem);
		FColor* SrcBuff = new(GMem,CheckedBitmapPixelCount(SizeX, SizeY))FColor;
		RenDev->ReadPixels( SrcBuff );

		snapBuff.Empty ();
		snapBuff.Add (CheckedBitmapPixelCount(snapU, snapV));

		FColor * DstBuff = &snapBuff (0);

		for (INT y = 0; y<snapV; ++y)
		{
			FColor * pDstLine = DstBuff + y*snapU;
			FColor * pSrcTopLine = SrcBuff + y*SizeX* shrinkage;
			
			for (INT x = 0; x<snapU; ++x)
			{
				FColor * pSrcTopLeft = pSrcTopLine + x* shrinkage;

				// Calculate Dest pixel by adding all source pixels together
				int dest [4];
				for (INT i=0; i<4; ++i)
					dest[i] = 0;

				for (int srcY = 0; srcY<shrinkage; ++srcY)
				{
					FColor * pSrcLine = pSrcTopLeft + srcY*SizeX;

					for (int srcX = 0; srcX<shrinkage; ++srcX)
					{
						dest[0] += pSrcLine[srcX].R;
						dest[1] += pSrcLine[srcX].G;
						dest[2] += pSrcLine[srcX].B;
						dest[3] += pSrcLine[srcX].A;
					}
				}

				// divide dest by total number of pixels in source block
				for (INT i=0; i<4; ++i)
					dest [i] /= (shrinkage*shrinkage);

				pDstLine[x].R = dest[0];
				pDstLine[x].G = dest[1];
				pDstLine[x].B = dest[2];
				pDstLine[x].A = dest[3];
			}
		}

		Mark.Pop (); // destroy temporary buffer

		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("THUMBNAIL")) )
	{
		// Take a copy of the viewport image, shrink and store.
		//----------------------------------------------------
//		INT shrinkFactor = appAtoi(Cmd);
//		int shrinkage    = appPow(2, shrinkFactor);
		
		INT ThumbSizeX, ThumbSizeY;
		
		if( !Parse(Cmd, TEXT("x="), ThumbSizeX ) )
			ThumbSizeX = 160;
		
		if( !Parse(Cmd, TEXT("y="), ThumbSizeY ) )
			ThumbSizeY = 120;
		
		if( ThumbSizeX == 0 ) ThumbSizeX = 160;
		if( ThumbSizeY == 0 ) ThumbSizeY = 120;
		
		int shrinkage    = SizeX / ThumbSizeX;
		
		snapU = ThumbSizeX;
		snapV = ThumbSizeY;
		
		FPushMemTag Tag(TEXT("Snap"));
		FMemMark Mark(GMem);
		FColor* SrcBuff = new(GMem,CheckedBitmapPixelCount(SizeX, SizeY))FColor;
		RenDev->ReadPixels( SrcBuff );

		snapBuff.Empty ();
		snapBuff.Add (CheckedBitmapPixelCount(snapU, snapV));

		FColor * DstBuff = &snapBuff (0);

		for (INT y = 0; y<snapV; ++y)
		{
			FColor * pDstLine = DstBuff + y*snapU;
			FColor * pSrcTopLine = SrcBuff + y*SizeX* shrinkage;
			
			for (INT x = 0; x<snapU; ++x)
			{
				FColor * pSrcTopLeft = pSrcTopLine + x* shrinkage;

				// Calculate Dest pixel by adding all source pixels together
				int dest [4];
				for (INT i=0; i<4; ++i)
					dest[i] = 0;

				for (int srcY = 0; srcY<shrinkage; ++srcY)
				{
					FColor * pSrcLine = pSrcTopLeft + srcY*SizeX;

					for (int srcX = 0; srcX<shrinkage; ++srcX)
					{
						dest[0] += pSrcLine[srcX].R;
						dest[1] += pSrcLine[srcX].G;
						dest[2] += pSrcLine[srcX].B;
						dest[3] += pSrcLine[srcX].A;
					}
				}

				// divide dest by total number of pixels in source block
				for (INT i=0; i<4; ++i)
					dest [i] /= (shrinkage*shrinkage);

				pDstLine[x].R = dest[0];
				pDstLine[x].G = dest[1];
				pDstLine[x].B = dest[2];
				pDstLine[x].A = dest[3];
			}
		}

		Mark.Pop (); // destroy temporary buffer

		return 1;
	}	
	else if( ParseCommand(&Cmd,TEXT("SAVETHUMBNAIL")) )
	{
		TCHAR File[64];
		if( ParseToken( Cmd, File, ARRAY_COUNT(File), 0 ) && snapBuff.Num () )
		{
			FArchive* Ar = GFileManager->CreateFileWriter( *(GSys->SaveSlotPath * File) );
			if( Ar )
			{
				// USE the REAL proportions of our snapshot
				INT destWidth  = snapU;
				INT destHeight = snapV;
				
				WriteBitmapHeaders
				(
					*Ar,
					CheckedBitmapDimension(destWidth),
					CheckedBitmapDimension(destHeight)
				);

				FColor * Buf = snapBuff.GetData ();

				// Copy the actual data
				for( INT i=snapV-1; i>=0; i-- )
				{
					INT j=0;
					for( ; j<snapU; j++ )
						Ar->Serialize( &Buf[i*snapU+j], 3 );

					// fill extra space to make bitmap of texture proportions
					for( ; j<destWidth; j++ )
						Ar->Serialize(&Buf[0], 3 );
				}

				// Success.
				delete Ar;
			}
		}
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("SAVESNAP")) )
	{
		TCHAR File[64];
		if( ParseToken( Cmd, File, ARRAY_COUNT(File), 0 ) && snapBuff.Num () )
		{
			FArchive* Ar = GFileManager->CreateFileWriter( *(FString(appUserDir()) * File) );
			if( Ar )
			{
				// calculate proportions of final bmp to be suitable as a texture
				INT destWidth = snapU;
				for( INT widthFactor=0; ; widthFactor++ )
				{
					if( !destWidth )
					{
						check( widthFactor<31 );
						destWidth = static_cast<INT>(UINT32_C(1) << widthFactor);
						break;
					}
					destWidth >>= 1;
				}

				INT destHeight = snapV;
				for( INT heightFactor=0; ; heightFactor++ )
				{
					if( !destHeight )
					{
						check( heightFactor<31 );
						destHeight = static_cast<INT>(UINT32_C(1) << heightFactor);
						break;
					}
					destHeight >>= 1;
				}

				WriteBitmapHeaders
				(
					*Ar,
					CheckedBitmapDimension(destWidth),
					CheckedBitmapDimension(destHeight)
				);

				FColor * Buf = snapBuff.GetData ();

				// fill extra space to make bitmap of texture proportions
				for( INT i=destHeight-1; i>=snapV; i-- )
				{
					for( INT j=0; j<destWidth; j++ )
						Ar->Serialize(&Buf[0], 3 );
				}

				// Copy the actual data
				for( INT i=snapV-1; i>=0; i-- )
				{
					INT j=0;
					for( ; j<snapU; j++ )
						Ar->Serialize( &Buf[i*snapU+j], 3 );

					// fill extra space to make bitmap of texture proportions
					for( ; j<destWidth; j++ )
						Ar->Serialize(&Buf[0], 3 );
				}

				// Success.
				delete Ar;
			}
		}
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("SHOWACTORS")) )
	{
		Actor->ShowFlags |= SHOW_Actors;
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("HIDEACTORS")) )
	{
		Actor->ShowFlags &= ~SHOW_Actors;
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("RMODE")) )
	{
		INT Mode = appAtoi(Cmd);
		if( Mode>REN_None && Mode<REN_MAX )
			Actor->RendMap = Mode;
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("EXEC")) )
	{
		TCHAR Filename[64];
		if( ParseToken( Cmd, Filename, ARRAY_COUNT(Filename), 0 ) )
			ExecMacro( Filename, Ar );
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("SHOWBONES")) )
	{
		if (appAtoi(Cmd))
			Actor->ShowFlags |= SHOW_Bones;
		else
			Actor->ShowFlags &= ~SHOW_Bones;
		return 1;
	}
	else if( ParseCommand(&Cmd,TEXT("SHOWBOUNDS")) )
	{
		if (appAtoi(Cmd))
			Actor->ShowFlags |= SHOW_Bounds;
		else
			Actor->ShowFlags &= ~SHOW_Bounds;
		return 1;
	}
	else if( Console && Console->ScriptConsoleExec(Cmd,Ar,Actor) )
	{
		return 1;
	}
	else if( UPlayer::Exec(Cmd,Ar) )
	{
		return 1;
	}
	else return 0;
	unguard;
}

//
// Execute a macro on this viewport.
//
void UViewport::ExecMacro( const TCHAR* Filename, FOutputDevice& Ar )
{
	guard(UViewport::ExecMacro);

	// Create text buffer and prevent garbage collection.
	UTextBuffer* Text = ImportObject<UTextBuffer>( GetTransientPackage(), NAME_None, 0, Filename );
	if( Text )
	{
		Text->AddToRoot();
		debugf( TEXT("Execing %s"), Filename );
		TCHAR Temp[256];
		const TCHAR* Data = *Text->Text;
		while( ParseLine( &Data, Temp, ARRAY_COUNT(Temp) ) )
			Exec( Temp, Ar );
		Text->RemoveFromRoot();
		delete Text;
	}
	else Ar.Logf( NAME_ExecWarning, LocalizeError("FileNotFound",TEXT("Core")), Filename );

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

	// Update input.
	if( DeltaSeconds!=-1.f )
		UpdateInput( 0 );

	// Get input from input system.
	Input->ReadInput( DeltaSeconds, *GLog );

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

	Hit.Size = Size;
	HitSizes.AddItem(Size);
	RenDev->PushHit( (BYTE*)&Hit, Size );

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

	RenDev->PopHit( HitSizes.Pop(), bForce );

	unguard;
}

//
// Execute all hits in the hit buffer.
//
void UViewport::ExecuteHits( const FHitCause& Cause, BYTE* HitData, INT HitSize )
{
	guard(UViewport::ExecuteHits);

	// String together the hit stack.
	HHitProxy* TopHit=NULL;
	while( HitSize>0 )
	{
		HHitProxy* ThisHit = (HHitProxy*)HitData;
		HitData           += ThisHit->Size;
		HitSize           -= ThisHit->Size;
		ThisHit->Parent    = TopHit;
		TopHit             = ThisHit;
	}
	check(HitSize==0);

	// Call the innermost hit.
	if( TopHit )
		TopHit->Click( Cause );

	unguard;
}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/

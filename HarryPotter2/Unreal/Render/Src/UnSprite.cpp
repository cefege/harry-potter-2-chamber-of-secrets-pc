/*=============================================================================
	UnSprite.cpp: Unreal sprite rendering functions.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "Render.h"
#include "UnMesh.h"


/*------------------------------------------------------------------------------
	Dynamics setup and rendering.
------------------------------------------------------------------------------*/

//
// Begin filtering dynamic objects through the top of the Bsp.
//
void URender::SetupDynamics( FSceneNode* Frame, AActor* Exclude )
{
	guard(URender::SetupDynamics);
	if
	(	!(Frame->Level->Model->Nodes.Num())
	||	!(Frame->Viewport->Actor->ShowFlags & SHOW_Actors) )
		return;
	STAT(clock(GStat.FilterTime));
	UBOOL HighDetailActors=Frame->Viewport->RenDev->HighDetailActors;

	// Traverse entire actor list.
	for( INT iActor=0; iActor<Frame->Level->Actors.Num(); iActor++ )
	{
		// Add this actor to dynamics if it's renderable.
		AActor* Actor = Frame->Level->Actors(iActor);
		if
		(	Actor
		&&	(!Actor->bHighDetail || HighDetailActors) 
		&&	(Frame->Recursion!=0 || Frame->Viewport->Actor->bBehindView || Actor!=Frame->Viewport->Actor->ViewTarget) )
		{
			if
			(	(Actor != Exclude)
			&&	(GIsEditor ? !Actor->bHiddenEd : !Actor->bHidden)
			// Call PlayerPawn Render Control Interface (RCI) to assess visible actors.
			&&	( ( GIsEditor && !( Frame->Viewport->Actor->ShowFlags & SHOW_PlayerCtrl ) )
				|| ( Frame->Viewport->Actor->IsA( APlayerPawn::StaticClass() ) 
					&& Frame->Viewport->Actor->IsActorVisible( Actor ) ) )
			// Clip actors that aren't "visible." Used by EARI.
			&&	( (Actor->VisibilityRadius == 0.0 || appSqrt((Actor->Location - Frame->Coords.Origin).SizeSquared2D()) < Actor->VisibilityRadius)
				&&(Actor->VisibilityHeight == 0.0 || Abs    ((Actor->Location - Frame->Coords.Origin).Z              ) < Actor->VisibilityHeight) )
			&&	(!Actor->bOnlyOwnerSee || (Actor->IsOwnedBy(Frame->Viewport->Actor) && !Frame->Viewport->Actor->bBehindView))
			&&	(!Actor->IsOwnedBy(Frame->Viewport->Actor) || !Actor->bOwnerNoSee || (Actor->IsOwnedBy(Frame->Viewport->Actor) && Frame->Viewport->Actor->bBehindView)) )
			{
				// Add the sprite proxy.
				if( !Actor->IsMovingBrush() )
				{
					UClass* ObjectClass = Actor->RenderIteratorClass;
					if( ObjectClass != NULL && (Actor->RenderInterface == NULL || !Actor->RenderInterface->IsValid()) )
					{
						// When StaticConstructObject starts blowing up on you when you try to create a ParticleSprayer, set Padding[XXX] 
						// in RenderIterator.uc to a large number (like 4096), recompile, and try to load the level again.  It will crash, 
						// but it will give you a new value to set XXX to in the log file.
						Actor->RenderInterface = (URenderIterator*)StaticConstructObject( ObjectClass, Actor, NAME_None, 0, ObjectClass->GetDefaultObject() );
					}
					else if( ObjectClass == NULL && Actor->RenderInterface != NULL )
					{
						Actor->RenderInterface->ConditionalDestroy();
						delete Actor->RenderInterface;
						Actor->RenderInterface = NULL;
					}
					URenderIterator* i = Actor->RenderInterface;
					// If the RenderInterface for the actor is NULL, use the default actor rendering behavior
					if( i == NULL )
					{
						new(GDynMem)FDynamicSprite( Frame, 0, Actor );
					}
					else
					{
						// Setup EARI stats. (!! Move to seperate function or define? Messy here.)
						STAT(if( GStat.TotalEARIActors < ARRAY_COUNT(GStat.EARIActorNames) ) STAT(GStat.EARIActorNames[ GStat.TotalEARIActors ] = Actor->GetFName()));
						STAT(if( GStat.TotalEARIActors < ARRAY_COUNT(GStat.EARIActorTags) ) STAT(GStat.EARIActorTags[ GStat.TotalEARIActors ] = Actor->Tag));
						STAT(clock(GStat.TotalEARITime));
						STAT(if( GStat.TotalEARIActors < ARRAY_COUNT(GStat.EARITime) ) clock(GStat.EARITime[ GStat.TotalEARIActors ]));

						i->Init( Frame );
						if( Actor->bFilterByVolume )
						{
							// Filter the entire system by the visibility radius and height.
							FDynamicSysParent* System = new(GDynMem)FDynamicSysParent( Frame, 0, Actor );
							for( i->First(); !i->IsDone(); i->Next() )
							{
								AActor* DisplayOp = i->CurrentItem();
								STAT(GStat.TotalEARISubActors++);
								STAT(if( GStat.TotalEARIActors < ARRAY_COUNT(GStat.EARISubActors) ) GStat.EARISubActors[ GStat.TotalEARIActors ]++);
								STAT(clock(GStat.EARIActorDrawTime));
								FDynamicSysChild* Child = new(GDynMem)FDynamicSysChild( Frame, 0, DisplayOp );
								System->Children.AddItem( Child );
								STAT(if( GStat.TotalEARIActors < ARRAY_COUNT(GStat.EARIDrawTime) ) unclock(GStat.EARIDrawTime[ GStat.TotalEARIActors ]));
								STAT(unclock(GStat.EARIActorDrawTime));
							}
						} else {
							// Filter each iteration individually.
							for( i->First(); !i->IsDone(); i->Next() )
							{
								AActor* DisplayOp = i->CurrentItem();
								STAT(GStat.TotalEARISubActors++);
								STAT(if( GStat.TotalEARIActors < ARRAY_COUNT(GStat.EARISubActors) ) GStat.EARISubActors[ GStat.TotalEARIActors ]++);
								STAT(clock(GStat.EARIActorDrawTime));
								STAT(if( GStat.TotalEARIActors < ARRAY_COUNT(GStat.EARIDrawTime) ) clock(GStat.EARIDrawTime[ GStat.TotalEARIActors ]));
								new(GDynMem)FDynamicSprite( Frame, 0, DisplayOp );
								STAT(if( GStat.TotalEARIActors < ARRAY_COUNT(GStat.EARIDrawTime) ) unclock(GStat.EARIDrawTime[ GStat.TotalEARIActors ]));
								STAT(unclock(GStat.EARIActorDrawTime));
							}
						}
						i->UnInit();

						// Finish EARI stats.
						STAT(if( GStat.TotalEARIActors < ARRAY_COUNT(GStat.EARITime) ) unclock(GStat.EARITime[ GStat.TotalEARIActors ]));
						STAT(unclock(GStat.TotalEARITime));
						STAT(GStat.TotalEARIActors++);
					}

					// Compute shadow for all renderable actors.
					if ( Actor && Actor->Shadow && !Frame->Viewport->IsOrtho() )
					{
						Clock(GStat.DecalUpdateTime);

						// Determine shadow volume bounding box.
						const float ShadowHeight = 8192;
						FBox Bounds
						( 
							Actor->Location - FVector( Actor->CollisionRadius, Actor->CollisionRadius, ShadowHeight ),
							Actor->Location + FVector( Actor->CollisionRadius, Actor->CollisionRadius, 0 )
						);
						FVector Center = (Bounds.Min + Bounds.Max) * 0.5f;
						FVector Size = (Bounds.Max - Bounds.Min) * 0.5f;

						// Setup projection plane.
						float Extent = Abs(Frame->Coords.ZAxis.X * Size.X)
									 + Abs(Frame->Coords.ZAxis.Y * Size.Y)
									 + Abs(Frame->Coords.ZAxis.Z * Size.Z);
						FLOAT Z = ((Center - Frame->Coords.Origin) | Frame->Coords.ZAxis);// - Extent;
						if( Z >= -Extent )
						{
							FScreenBounds ScreenBounds;
							if( BoundVisible( Frame, Bounds.GetCoords(), NULL, ScreenBounds ) )
								Actor->Shadow->eventUpdate(NULL);
						}
					}
				}
				else if( Frame->Level->BrushTracker )
				{
					//bounding box reject!!
					Frame->Level->BrushTracker->Update( Actor );
				}
			}
			if
			(	(Actor->LightType)
			&&	(!(Actor->bStatic || Actor->bNoDelete) || Actor->bDynamicLight)
			&&	(Actor->LightBrightness)
			&&	(Actor->LightRadius) )
			{
				// Add the dynamic light.
				FLOAT MaxRadius = Max( Actor->WorldLightRadius(), Actor->WorldVolumetricRadius() );
				int i;
				for( i=0; i<4; i++ )
					if( Frame->ViewPlanes[i].PlaneDot(Actor->Location) < -MaxRadius )
						break;
				if( i==4 )
				{
					UBOOL IsVolumetric = Actor->Region.Zone->bFogZone && Actor->VolumeRadius && Actor->VolumeBrightness;
					for( i=0; IsVolumetric && i<4; i++ )
						if( Frame->ViewPlanes[i].PlaneDot(Actor->Location) < -Actor->WorldVolumetricRadius() )
							IsVolumetric = 0;
					new(GDynMem)FDynamicLight( 0, Actor, IsVolumetric, 0 );
					STAT(GStat.DynLightActors++);
				}
			}
		}
	}
	STAT(unclock(GStat.FilterTime));
	unguard;
}

/*------------------------------------------------------------------------------
	FDynamicSprite implementation.
------------------------------------------------------------------------------*/

FDynamicSprite::FDynamicSprite( FSceneNode* Frame, INT iNode, AActor* InActor )
:	FDynamicItem	( iNode )
,	Actor			( InActor )
,	SpanBuffer		( NULL )
,	RenderNext		( NULL )
,	Volumetrics		( NULL )
,	LeafLights		( NULL )
{
	guardSlow(FDynamicSprite::FDynamicSprite);

	if( Setup( Frame ) )
	{
		// Add at start of list.
		FilterNext = GRender->Dynamic( iNode, 0 );
		GRender->Dynamic( iNode, 0 ) = this;

		// Compute four projection-plane points from sprite extents and viewport.
		FLOAT FloatX1 = X1; 
		FLOAT FloatX2 = X2;
		FLOAT FloatY1 = Y1; 
		FLOAT FloatY2 = Y2;

		// Move closer to prevent actors from slipping into floor.
		FLOAT PlaneZRD	= MinZ * Frame->RProj.Z;
		FLOAT PlaneX1   = PlaneZRD * (FloatX1 - Frame->FX2);
		FLOAT PlaneX2   = PlaneZRD * (FloatX2 - Frame->FX2);
		FLOAT PlaneY1   = PlaneZRD * (FloatY1 - Frame->FY2);
		FLOAT PlaneY2   = PlaneZRD * (FloatY2 - Frame->FY2);

		// Generate four screen-aligned box vertices.
		ProxyVerts[0].Point = FVector(PlaneX1, PlaneY1, MinZ).TransformPointBy( Frame->Uncoords );
		ProxyVerts[1].Point = FVector(PlaneX2, PlaneY1, MinZ).TransformPointBy( Frame->Uncoords );
		ProxyVerts[2].Point = FVector(PlaneX2, PlaneY2, MinZ).TransformPointBy( Frame->Uncoords );
		ProxyVerts[3].Point = FVector(PlaneX1, PlaneY2, MinZ).TransformPointBy( Frame->Uncoords );

		// Screen coords.
		ProxyVerts[0].ScreenX = FloatX1; ProxyVerts[0].ScreenY = FloatY1;
		ProxyVerts[1].ScreenX = FloatX2; ProxyVerts[1].ScreenY = FloatY1;
		ProxyVerts[2].ScreenX = FloatX2; ProxyVerts[2].ScreenY = FloatY2;
		ProxyVerts[3].ScreenX = FloatX1; ProxyVerts[3].ScreenY = FloatY2;

		// Generate a full rasterization for this box, which we'll filter down the Bsp.
		//!!inefficient when in box
		check(Y1>=0);
		check(Y2<=Frame->Y);
		check(Y1<Y2);
		FRasterPoly* Raster = (FRasterPoly *)New<BYTE>(GDynMem,appCheckedIntSize(sizeof(FRasterPoly) + static_cast<SIZE_T>(Y2-Y1)*sizeof(FRasterSpan)));
		Raster->StartY	    = Y1;
		Raster->EndY	    = Y2;

		FRasterSpan* Line = &Raster->Lines[0];
		for( INT i=Raster->StartY; i<Raster->EndY; i++ )
		{
			Line->X[0] = X1;
			Line->X[1] = X2;
			Line++;
		}

		// Add first sprite chunk at end of dynamics list, and cause it to be filtered, since
		// it's being added at the start.
		new(GDynMem)FDynamicChunk( iNode, this, Raster );
	}
	STAT(GStat.NumSprites++);
	unguardSlow;
}

UBOOL FDynamicSprite::Setup( FSceneNode* Frame )
{
	guardSlow(FDynamicSprite::Setup);

	// Handle the actor based on its type.
	if( Actor->DrawType==DT_Sprite || Actor->DrawType==DT_SpriteAnimOnce || (Frame->Viewport->Actor->ShowFlags & SHOW_ActorIcons)
	|| (Actor->DrawType==DT_Particles && (Frame->Viewport->IsOrtho() || !Frame->Viewport->IsRealtime())) )
	{
		// Make sure we have something to draw.
		FLOAT     DrawScale = Actor->DrawScale;
		UTexture* Texture   = Actor->Texture;

		if( Frame->Viewport->Actor->ShowFlags & SHOW_ActorIcons )
		{
			DrawScale = 1.0;
			if( !Texture )
				Texture = GetDefault<AActor>()->Texture;
		}
		if( !Texture )
			return 0;

		// Setup projection plane.
		MinZ = MaxZ = ((Actor->Location - Frame->Coords.Origin) | Frame->Coords.ZAxis) - Actor->SpriteProjForward;
		if( MinZ<-2*Actor->SpriteProjForward && !Frame->Viewport->IsOrtho() )
			return 0;

		// See if this is occluded.
		if( !GRender->Project( Frame, Actor->Location, ScreenX, ScreenY, &Persp ) )
			return 0;

		// X extent.
		FLOAT XSize = Persp * DrawScale * Texture->USize;//!!expensive
		X1          = appRound(appCeil(ScreenX-XSize/2));
		X2          = appRound(appCeil(ScreenX+XSize/2));
		if( X1 > X2 )
		{
			Exchange( X1, X2 );
		}
		if( X1 < 0 )
		{
			X1 = 0;
			if( X2 < 0 )
				X2 = 0;
		}
		if( X2 > Frame->X )
		{
			X2 = Frame->X;
			if( X1 > Frame->X )
				X1 = Frame->X;
		}
		if( X2<=0 || X1>=Frame->X-1 )
			return 0;

		// Y extent.
		FLOAT YSize = Persp * DrawScale * Texture->VSize;
		Y1          = appRound(appCeil(ScreenY-YSize/2));
		Y2          = appRound(appCeil(ScreenY+YSize/2));
		if( Y1 > Y2 )
		{
			Exchange( Y1, Y2 );
		}
		if( Y1 < 0 )
		{
			Y1 = 0;
			if( Y2 < 0 )
				Y2 = 0;
		}
		if( Y2 > Frame->Y )
		{
			Y2 = Frame->Y;
			if( Y1 > Frame->Y )
				Y1 = Frame->Y;
		}
		if( Y2<=0 || Y1>=Frame->Y || Y1>=Y2 )
			return 0;
		return 1;
	}
	else if( Actor->DrawType==DT_Mesh || Actor->DrawType==DT_Particles )
	{
		AParticleFX* ParticleFX = Cast<AParticleFX>(Actor);
		if( ParticleFX && ParticleFX->bShellOnly ) 
		{
			X1 = 0;
			Y1 = 0;
			X2 = Frame->Viewport->SizeX;
			Y2 = Frame->Viewport->SizeY;
			return 1;
		}

		// Get bounding box.
		FCoords BoxCoords = Actor->GetRenderBoundingBox( 1 );
		if( BoxCoords.XAxis.SizeSquared() == 0.f )
			return 0;

		FScreenBounds ScreenBounds;
		if( !GRender->BoundVisible( Frame, BoxCoords, NULL, ScreenBounds ) )
			return 0;

		X1 = appRound(ScreenBounds.MinX);
		X2 = appRound(ScreenBounds.MaxX);
		Y1 = appRound(ScreenBounds.MinY);
		Y2 = appRound(ScreenBounds.MaxY);

		MinZ = ScreenBounds.MinZ;
		MaxZ = ScreenBounds.MaxZ;
		if( Y1>=Y2 )
			return 0;

		STAT(GStat.SpanPix += (X2+1-X1)*(Y2+1-Y1));
		return 1;
	}
	
	else return 0;
	unguardSlow;
}

/*------------------------------------------------------------------------------
	FDynamicChunk implementation.
------------------------------------------------------------------------------*/

FDynamicChunk::FDynamicChunk( INT iNode, FDynamicSprite* InSprite, FRasterPoly* InRaster )
:	FDynamicItem	( iNode )
,	Raster			( InRaster )
,	Sprite			( InSprite )
{
	guardSlow(FDynamicChunk::FDynamicChunk);

	// Add at start of list.
	FilterNext = GRender->Dynamic( iNode, 0 );
	GRender->Dynamic( iNode, 0 ) = this;

	STAT(GStat.NumChunks++);
	unguardSlow;
}

void FDynamicChunk::Filter( UViewport* Viewport, FSceneNode* Frame, INT iNode, INT Outside )
{
	guardSlow(FDynamicChunk::Filter);
	FBspNode& Node = Frame->Level->Model->Nodes(iNode);

	// Setup.
	FRasterPoly *FrontRaster, *BackRaster;

	// Find point-to-plane distances for all four vertices (side-of-plane classifications).
	INT Front=0, Back=0;
	FLOAT Dist[4];
	INT i;
	for( i=0; i<4; i++ )
	{
		Dist[i] = Node.Plane.PlaneDot( Sprite->ProxyVerts[i].Point );
		Front  += Dist[i] > +0.01;
		Back   += Dist[i] < -0.01;
	}
	if( Front && Back )
	{	
		// Find intersection points.
		FTransform	Intersect[4];
		FTransform* I  = &Intersect	         [0];
		FTransform* V1 = &Sprite->ProxyVerts [3]; 
		FTransform* V2 = &Sprite->ProxyVerts [0];
		FLOAT*      D1 = &Dist			     [3];
		FLOAT*      D2 = &Dist			     [0];
		INT			NumInt = 0;

		for( i=0; i<4; i++ )
		{
			if( (*D1)*(*D2) < 0.0 )
			{	
				// At intersection point.
				FLOAT Alpha = *D1 / (*D1 - *D2);
				I->ScreenX  = V1->ScreenX + Alpha * (V2->ScreenX - V1->ScreenX);
				I->ScreenY  = V1->ScreenY + Alpha * (V2->ScreenY - V1->ScreenY);

				I++;
				NumInt++;
			}
			V1 = V2++;
			D1 = D2++;
		}
		if( NumInt < 2 )
			goto NoSplit;

		// Allocate front and back rasters.
		const SIZE_T Size = sizeof(FRasterPoly) + static_cast<SIZE_T>(Raster->EndY - Raster->StartY) * sizeof(FRasterSpan);
		FrontRaster	= (FRasterPoly *)New<BYTE>(GDynMem,appCheckedIntSize(Size));
		BackRaster	= (FRasterPoly *)New<BYTE>(GDynMem,appCheckedIntSize(Size));

		// Make sure that first intersection point is on top.
		if( Intersect[0].ScreenY > Intersect[1].ScreenY )
			Exchange( Intersect[0], Intersect[1] );
		INT Y0 = Max( appFloor(Intersect[0].ScreenY), Raster->StartY );
		INT Y1 = Min( appFloor(Intersect[1].ScreenY), Raster->EndY   );
		if( Y0>Y1 )
			goto NoSplit;

		// Find TopRaster.
		FRasterPoly* TopRaster = NULL;
		if( Y0 > Raster->StartY )
		{
			if( Dist[0] >= 0 ) TopRaster = FrontRaster;
			else               TopRaster = BackRaster;
		}

		// Find BottomRaster.
		FRasterPoly* BottomRaster = NULL;
		if( Y1 < Raster->EndY )
		{
			if( Dist[2] >= 0 ) BottomRaster = FrontRaster;
			else               BottomRaster = BackRaster;
		}

		// Find LeftRaster and RightRaster.
		FRasterPoly *LeftRaster, *RightRaster;
		if( Intersect[1].ScreenX >= Intersect[0].ScreenX )
		{
			if (Dist[1] >= 0.0) {LeftRaster = BackRaster;  RightRaster = FrontRaster;}
			else	   			{LeftRaster = FrontRaster; RightRaster = BackRaster; };
		}
		else // Intersect[1].ScreenX < Intersect[0].ScreenX
		{
			if (Dist[0] >= 0.0) {LeftRaster = FrontRaster; RightRaster = BackRaster; }
			else                {LeftRaster = BackRaster;  RightRaster = FrontRaster;};
		}

		// Set left and right raster defaults (may be overwritten by TopRaster or BottomRaster).
		checkSlow(Y0>=0);
		checkSlow(Y1<=Frame->Y);
		LeftRaster->StartY = Y0; RightRaster->StartY = Y0;
		LeftRaster->EndY   = Y1; RightRaster->EndY   = Y1;

		// Copy TopRaster section.
		if( TopRaster )
		{
			TopRaster->StartY = Raster->StartY;

			FRasterSpan* SourceLine	= &Raster->Lines    [0];
			FRasterSpan* Line		= &TopRaster->Lines [0];

			for( i=TopRaster->StartY; i<Y0; i++ )
				*Line++ = *SourceLine++;
		}

		// Copy BottomRaster section.
		if( BottomRaster )
		{
			BottomRaster->EndY = Raster->EndY;

			FRasterSpan* SourceLine	= &Raster->Lines       [Y1 - Raster->StartY];
			FRasterSpan* Line       = &BottomRaster->Lines [Y1 - BottomRaster->StartY];

			for( i=Y1; i<BottomRaster->EndY; i++ )
				*Line++ = *SourceLine++;
		}

		// Split middle raster section.
		if( Y1 != Y0 )
		{
			FLOAT	FloatYAdjust	= (FLOAT)Y0 + 1.0f - Intersect[0].ScreenY;
			FLOAT	FloatFixDX 		= 65536.0f * (Intersect[1].ScreenX - Intersect[0].ScreenX) / (Intersect[1].ScreenY - Intersect[0].ScreenY);
			INT		FixDX			= appRound(FloatFixDX);
			INT		FixX			= appRound(65536.0f * Intersect[0].ScreenX + FloatFixDX * FloatYAdjust);

			if( Raster->StartY > Y0 ) 
			{
				FixX   += (Raster->StartY-Y0) * FixDX;
				Y0		= Raster->StartY;
			}
			if( Raster->EndY < Y1 )
			{
				Y1      = Raster->EndY;
			}
			
			FRasterSpan	*SourceLine = &Raster->Lines      [Y0 - Raster->StartY];
			FRasterSpan	*LeftLine   = &LeftRaster->Lines  [Y0 - LeftRaster->StartY];
			FRasterSpan	*RightLine  = &RightRaster->Lines [Y0 - RightRaster->StartY];

			while( Y0++ < Y1 )
			{
				*LeftLine  = *SourceLine;
				*RightLine = *SourceLine;

				INT X = Unfix(FixX);
				if (X < LeftLine->X[1])    LeftLine->X[1] = X;
				if (X > RightLine->X[0]) RightLine->X[0] = X;

				FixX       += FixDX;
				SourceLine ++;
				LeftLine   ++;
				RightLine  ++;
			}
		}

		// Discard any rasters that are completely empty.
		if( BackRaster->EndY <= BackRaster->StartY )
			BackRaster = NULL;
		if( FrontRaster->EndY <= FrontRaster->StartY )
			FrontRaster = NULL;
	}
	else
	{
		// Don't have to split the rasterization.
		NoSplit:
		FrontRaster = BackRaster = Raster;
	}

	// Filter it down.
	INT CSG = Node.IsCsg();
	if( Front && FrontRaster )
	{
		if( Node.iFront != INDEX_NONE )
			new(GDynMem)FDynamicChunk( Node.iFront, Sprite, FrontRaster );
		else if( Outside || CSG )
			new(GDynMem)FDynamicFinalChunk( iNode, Sprite, FrontRaster, 0 );
	}
	if( Back && BackRaster )
	{
		if( Node.iBack != INDEX_NONE  )
			new(GDynMem)FDynamicChunk( Node.iBack, Sprite, BackRaster );
		else if( Outside && !CSG )
			new(GDynMem)FDynamicFinalChunk( iNode, Sprite, BackRaster, 1 );
	}
	unguardSlow;
}

/*------------------------------------------------------------------------------
	FDynamicFinalChunk implementation.
------------------------------------------------------------------------------*/

FDynamicFinalChunk::FDynamicFinalChunk( INT iNode, FDynamicSprite* InSprite, FRasterPoly* InRaster, INT IsBack )
:	FDynamicItem( iNode )
,	Raster( InRaster )
,	Sprite( InSprite )
{
	guardSlow(FDynamicFinalChunk::FDynamicFinalChunk);

	// Set Z.
	MinZ = InSprite->MinZ;
	MaxZ = InSprite->MaxZ;

	// Add into list z-sorted.
	FDynamicItem** Item = &GRender->Dynamic( iNode, IsBack );
	for( ; *Item && (*Item)->MinZ+(*Item)->MaxZ < MinZ+MaxZ; Item=&(*Item)->FilterNext );
	FilterNext = *Item;
	*Item      = this;

	STAT(GStat.NumFinalChunks++);
	unguardSlow;
}

void FDynamicFinalChunk::PreRender( UViewport* Viewport, FSceneNode* Frame, FSpanBuffer* SpanBuffer, INT iNode, FVolActorLink* Volumetrics )
{
	guardSlow(FDynamicFinalChunk::PreRender);
	UBOOL Drawn=0;
	if( !Sprite->SpanBuffer )
	{
		// Creating a new span buffer for this sprite.
		Sprite->SpanBuffer = New<FSpanBuffer>(GDynMem);
		Sprite->SpanBuffer->AllocIndex( Raster->StartY, Raster->EndY, &GDynMem );

		if( Sprite->SpanBuffer->CopyFromRaster( *SpanBuffer, Raster->StartY, Raster->EndY, (FRasterSpan*)Raster->Lines ) )
		{
			// Span buffer is non-empty, so keep it and put it on the to-draw list.
			STAT(GStat.ChunksDrawn++);
			Drawn                = 1;
			FDynamicSprite** Item = &Frame->Sprite;
			for( ; *Item && (*Item)->MinZ+(*Item)->MaxZ > Sprite->MinZ+Sprite->MaxZ; Item=&(*Item)->RenderNext );
			Sprite->RenderNext = *Item;
			*Item      = Sprite;
		}
		else
		{
			// Span buffer is empty, so ditch it.
			Sprite->SpanBuffer->Release();
			Sprite->SpanBuffer = NULL;
		}
	}
	else
	{
		// Merging with the sprite's existing span buffer.
		FMemMark Mark(GMem);
		FSpanBuffer* Span = New<FSpanBuffer>(GMem);
		Span->AllocIndex(Raster->StartY,Raster->EndY,&GMem);
		if( Span->CopyFromRaster( *SpanBuffer, Raster->StartY, Raster->EndY, (FRasterSpan*)Raster->Lines ) )
		{
			// Temporary span buffer is non-empty, so merge it into sprite's.
			Drawn = 1;
			Sprite->SpanBuffer->MergeWith(*Span);
			STAT(GStat.ChunksDrawn++);
		}

		// Release the temporary memory.
		Mark.Pop();
	}

	// Add volumetrics to list.
	if( Drawn )
	{
		for( Volumetrics; Volumetrics; Volumetrics=Volumetrics->Next )
		{
			if( Volumetrics->Volumetric )
			{
				FActorLink* Link;
				for( Link=Sprite->Volumetrics; Link; Link=Link->Next )
					if( Link->Actor==Volumetrics->Actor )
						break;
				if( !Link )
					Sprite->Volumetrics = new(GDynMem)FActorLink(Volumetrics->Actor,Sprite->Volumetrics);
			}
		}
	}

	unguardSlow;
}

/*-----------------------------------------------------------------------------
	FDynamicLight implementation.
-----------------------------------------------------------------------------*/

FDynamicLight::FDynamicLight( INT iNode, AActor* InActor, UBOOL InIsVol, UBOOL InHitLeaf )
:	FDynamicItem( iNode )
,	Actor( InActor )
,	IsVol( InIsVol )
,	HitLeaf( InHitLeaf )
{
	guardSlow(FDynamicLight::FDynamicLight);

	// Add at start of list.
	FilterNext = GRender->Dynamic( iNode, 0 );
	GRender->Dynamic( iNode, 0 ) = this;

	STAT(GStat.NumMovingLights++);
	unguardSlow;
}

void FDynamicLight::Filter( UViewport* Viewport, FSceneNode* Frame, INT iNode, INT Outside )
{
	guardSlow(FDynamicLight::Filter);

	// Filter down.
	FBspNode& Node = Viewport->Actor->GetLevel()->Model->Nodes(iNode);
	FLOAT Dist   = Node.Plane.PlaneDot( Actor->Location );
	FLOAT Radius = Actor->WorldLightRadius();
	if( Dist > -Radius )
	{
		// Filter down front.
		UBOOL ThisHitLeaf=HitLeaf;
		if( !HitLeaf )
		{
			INT iLeaf=Node.iLeaf[1];
			if( iLeaf!=INDEX_NONE )
			{
				if( !GRender->LeafLights[iLeaf] )
					GRender->DynLightLeaves[GRender->NumDynLightLeaves++] = iLeaf;
				GRender->LeafLights[iLeaf] = new( GMem )FVolActorLink( Frame->Coords, Actor, GRender->LeafLights[iLeaf], IsVol && Dist>-Actor->WorldVolumetricRadius() );
				ThisHitLeaf=1;
			}
		}
		if( Node.iFront!=INDEX_NONE )
			new(GDynMem)FDynamicLight( Node.iFront, Actor, IsVol && Dist>-Actor->WorldVolumetricRadius(), ThisHitLeaf );

		// Handle planars.
		if( Dist < Radius )
		{
			for( INT iPlane=iNode; iPlane!=INDEX_NONE; iPlane = Viewport->Actor->GetLevel()->Model->Nodes(iPlane).iPlane )
			{
				FBspNode&       Node  = Viewport->Actor->GetLevel()->Model->Nodes(iPlane);
				FBspSurf&       Surf  = Viewport->Actor->GetLevel()->Model->Surfs(Node.iSurf);
				FLightMapIndex* Index = Viewport->Actor->GetLevel()->Model->GetLightMapIndex(Node.iSurf);

				if
				(	(Index)
				&&	(GRender->NumDynLightSurfs < URender::MAX_DYN_LIGHT_SURFS)
				&&	(Actor->bSpecialLit ? (Surf.PolyFlags&PF_SpecialLit) : !(Surf.PolyFlags&PF_SpecialLit)) )
				{
					// Don't apply a light twice.
					FActorLink* Link;
					for( Link = GRender->SurfLights[Node.iSurf]; Link; Link=Link->Next )
						if( Link->Actor == Actor )
							break;
					if( !Link )
					{
						if( !GRender->SurfLights[Node.iSurf] )
							GRender->DynLightSurfs[GRender->NumDynLightSurfs++] = Node.iSurf;
						GRender->SurfLights[Node.iSurf] = new(GMem)FActorLink( Actor, GRender->SurfLights[Node.iSurf] );
					}
				}
			}
		}
	}
	if( Dist < Radius )
	{
		UBOOL ThisHitLeaf=HitLeaf;
		if( !HitLeaf )
		{
			INT iLeaf=Node.iLeaf[0];
			if( iLeaf!=INDEX_NONE )
			{
				if( !GRender->LeafLights[iLeaf] )
					GRender->DynLightLeaves[GRender->NumDynLightLeaves++] = iLeaf;
				GRender->LeafLights[iLeaf] = new( GMem )FVolActorLink( Frame->Coords, Actor, GRender->LeafLights[iLeaf], IsVol && Dist<Actor->WorldVolumetricRadius() );
				ThisHitLeaf=1;
			}
		}
		if( Node.iBack!=INDEX_NONE )
			new(GDynMem)FDynamicLight( Node.iBack, Actor, IsVol && Dist<Actor->WorldVolumetricRadius(), ThisHitLeaf );
	}
	unguardSlow;
}

/*------------------------------------------------------------------------------
	FDynamicSprite System implementation.
------------------------------------------------------------------------------*/

// A parent is filtered by InActor's visibility box.
FDynamicSysParent::FDynamicSysParent( FSceneNode* Frame, INT iNode, AActor* InActor )
:	FDynamicSprite	( Frame, iNode, InActor ) {}

UBOOL FDynamicSysParent::Setup( FSceneNode* Frame )
{
	guardSlow(FDynamicSysParent::Setup);

	MinZ = MaxZ = ((Actor->Location - Frame->Coords.Origin) | Frame->Coords.ZAxis) - Actor->SpriteProjForward;
	if( MinZ<-2*Actor->SpriteProjForward && !Frame->Viewport->IsOrtho() )
		return 0;

	FScreenBounds ScreenBounds;
	FBox Bounds = Actor->GetVisibilityBox();
	if( !GRender->BoundVisible( Frame, Bounds.GetCoords(), NULL, ScreenBounds ) )
		return 0;

	X1 = appRound(ScreenBounds.MinX);
	X2 = appRound(ScreenBounds.MaxX);
	Y1 = appRound(ScreenBounds.MinY);
	Y2 = appRound(ScreenBounds.MaxY);
	if( Y1>=Y2 )
		return 0;

	return 1;

	unguardSlow;
}

// Children are set up normally, but never filtered.
// They are drawn when the filtered parent is drawn.
// They are not added to the dynamics list.
FDynamicSysChild::FDynamicSysChild( FSceneNode* Frame, INT iNode, AActor* InActor )
:	FDynamicSprite	( InActor )
{
	guardSlow(FDynamicSysChild::FDynamicSysChild);

	if( Setup( Frame ) )
	{
		// Compute four projection-plane points from sprite extents and viewport.
		FLOAT FloatX1 = X1; 
		FLOAT FloatX2 = X2;
		FLOAT FloatY1 = Y1; 
		FLOAT FloatY2 = Y2;

		// Move closer to prevent actors from slipping into floor.
		FLOAT PlaneZRD	= MinZ * Frame->RProj.Z;
		FLOAT PlaneX1   = PlaneZRD * (FloatX1 - Frame->FX2);
		FLOAT PlaneX2   = PlaneZRD * (FloatX2 - Frame->FX2);
		FLOAT PlaneY1   = PlaneZRD * (FloatY1 - Frame->FY2);
		FLOAT PlaneY2   = PlaneZRD * (FloatY2 - Frame->FY2);

		// Generate four screen-aligned box vertices.
		ProxyVerts[0].Point = FVector(PlaneX1, PlaneY1, MinZ).TransformPointBy( Frame->Uncoords );
		ProxyVerts[1].Point = FVector(PlaneX2, PlaneY1, MinZ).TransformPointBy( Frame->Uncoords );
		ProxyVerts[2].Point = FVector(PlaneX2, PlaneY2, MinZ).TransformPointBy( Frame->Uncoords );
		ProxyVerts[3].Point = FVector(PlaneX1, PlaneY2, MinZ).TransformPointBy( Frame->Uncoords );

		// Screen coords.
		ProxyVerts[0].ScreenX = FloatX1; ProxyVerts[0].ScreenY = FloatY1;
		ProxyVerts[1].ScreenX = FloatX2; ProxyVerts[1].ScreenY = FloatY1;
		ProxyVerts[2].ScreenX = FloatX2; ProxyVerts[2].ScreenY = FloatY2;
		ProxyVerts[3].ScreenX = FloatX1; ProxyVerts[3].ScreenY = FloatY2;

		// Generate a full rasterization for this box, which we'll filter down the Bsp.
		//!!inefficient when in box
		check(Y1>=0);
		check(Y2<=Frame->Y);
		check(Y1<Y2);
		FRasterPoly* Raster = (FRasterPoly *)New<BYTE>(GDynMem,appCheckedIntSize(sizeof(FRasterPoly) + static_cast<SIZE_T>(Y2-Y1)*sizeof(FRasterSpan)));
		Raster->StartY	    = Y1;
		Raster->EndY	    = Y2;

		FRasterSpan* Line = &Raster->Lines[0];
		for( INT i=Raster->StartY; i<Raster->EndY; i++ )
		{
			Line->X[0] = X1;
			Line->X[1] = X2;
			Line++;
		}
	}
	STAT(GStat.NumSprites++);
	unguardSlow;
}

/*-----------------------------------------------------------------------------
	Actor drawing.
-----------------------------------------------------------------------------*/

//
// Get inherent poly flags for an actor.
//
static DWORD GetPolyFlags( FSceneNode* Frame, AActor* Owner )
{
	guard(GetPolyFlags);
	DWORD PolyFlags=0;

	if     ( Owner->Style==STY_Masked      ) PolyFlags |= PF_Masked;
	else if( Owner->Style==STY_Translucent ) PolyFlags |= PF_Translucent;
	else if( Owner->Style==STY_Modulated   ) PolyFlags |= PF_Modulated;

	if	   ( Owner->bNoSmooth              ) PolyFlags |= PF_NoSmooth;
	if     ( Owner->bSelected              ) PolyFlags |= PF_Selected;
	if     ( Owner->bMeshEnviroMap         ) PolyFlags |= PF_Environment;
	if     (!Owner->bMeshCurvy             ) PolyFlags |= PF_Flat;
	if     ( Owner->bUnlit || Owner->Region.ZoneNumber==0 || Frame->Viewport->Actor->RendMap!=REN_DynLight || Frame->Viewport->GetOuterUClient()->NoLighting ) PolyFlags |= PF_Unlit;

	return PolyFlags;
	unguard;
}

//
// Draw an actor defined by a FDynamicSprite.
//
void URender::DrawActorSprite( FSceneNode* Frame, FDynamicSprite* Sprite )
{
	guard(URender::DrawActorSprite);
	PUSH_HIT(Frame,HActor(Sprite->Actor->GetHitActor()));
	DWORD PolyFlags = GetPolyFlags(Frame,Sprite->Actor);
	DWORD ShowFlags = Frame->Viewport->Actor->ShowFlags;

									 
	AActor* Owner = Sprite->Actor; //->GetHitActor();  What is hit actor anyway??!!!!

	// Draw the actor.
	if
	(	((Owner->DrawType == DT_Particles && Frame->Viewport->Actor->ShowFlags & SHOW_ActorIcons)  || Sprite->Actor->DrawType==DT_Sprite || Sprite->Actor->DrawType==DT_SpriteAnimOnce || (Frame->Viewport->Actor->ShowFlags & SHOW_ActorIcons)) 
	&&	(Sprite->Actor->Texture) )
	{
		// Sprite.
		guard(DrawSprite);
		FPlane    Color     = (GIsEditor && Sprite->Actor->GetHitActor()->bSelected) ? FPlane(.5f,.9f,.5f,0) : FPlane(1,1,1,0);
		UTexture* Texture   = Sprite->Actor->Texture;
		FLOAT     DrawScale = Sprite->Actor->DrawScale;
		UTexture* SavedNext = NULL;
		UTexture* SavedCur  = NULL;

		
				// guaranteed to be orthoview, i.e. Editor 
	
		
		if( Sprite->Actor->ScaleGlow!=1.0 )
		{
			Color *= Sprite->Actor->ScaleGlow;
			if( Color.X>1.0 ) Color.X=1.0;
			if( Color.Y>1.0 ) Color.Y=1.0;
			if( Color.Z>1.0 ) Color.Z=1.0;
		}
		if( Sprite->Actor->DrawType==DT_SpriteAnimOnce )
		{
			INT Count=1;
			for( UTexture* Test=Texture->AnimNext; Test && Test!=Texture; Test=Test->AnimNext )
				Count++;
			INT Num = Clamp( appFloor(Sprite->Actor->LifeFraction()*Count), 0, Count-1 );
			while( Num-- > 0 )
				Texture = Texture->AnimNext;
			SavedNext         = Texture->AnimNext;//sort of a hack!!
			SavedCur          = Texture->AnimCur;
			Texture->AnimNext = NULL;
			Texture->AnimCur  = NULL;
		}
		if( Frame->Viewport->Actor->ShowFlags & SHOW_ActorIcons )
		{
			DrawScale = 1.0;
			if( !Texture )
				Texture = GetDefault<AActor>()->Texture;
		}
		FLOAT XScale = Sprite->Persp * DrawScale * Texture->USize;
		FLOAT YScale = Sprite->Persp * DrawScale * Texture->VSize;
		if( Texture ) Frame->Viewport->Canvas->DrawIcon
		(
			Texture->Get( Frame->Viewport->CurrentTime ),
			Sprite->ScreenX - XScale/2,
			Sprite->ScreenY - YScale/2,
			XScale,
			YScale,
			Sprite->SpanBuffer,
			Sprite->MinZ,
			Color,
			FPlane(0,0,0,0),
			PolyFlags | PF_TwoSided | Texture->PolyFlags
		);
		if( Sprite->Actor->DrawType==DT_SpriteAnimOnce )
		{
			Texture->AnimNext = SavedNext;
			Texture->AnimCur  = SavedCur;
		}

		if ( Owner->DrawType == DT_Particles  ) 
		{
		//	PolyFlags &= ~(PF_Translucent|PF_Modulated|PF_Highlighted);
		//	PolyFlags |= PF_Masked;
					// Never draw particles in wireframe.
			if ( !Frame->Viewport->IsOrtho() )
				DrawParticleSystem(Frame, Sprite);

		}
		unguard;

	} 
	else if ( Sprite->Actor->DrawType==DT_Particles )
	{
		//STAT(GStat.SystemsAlive++);

		// Never draw particles in wireframe.
		if ( !Frame->Viewport->IsOrtho() )
			DrawParticleSystem(Frame, Sprite);
	}
	else if
	(	Sprite->Actor->DrawType==DT_Mesh
	&&	Sprite->Actor->Mesh )
	{
		// Mesh.
		guard(DrawMesh);
		if( Frame->Viewport->Actor->RendMap==REN_Polys || Frame->Viewport->Actor->RendMap==REN_PolyCuts || Frame->Viewport->Actor->RendMap==REN_Zones || Frame->Viewport->Actor->RendMap==REN_Wire )
			PolyFlags |= PF_FlatShaded;
		DrawMesh
		(
			Frame,
			Sprite,
			Sprite->Actor,
			Frame->Coords,
			PolyFlags
		);
		extern UBOOL HasSpecialCoords;
		extern FCoords SpecialCoords;
		APawn* Pawn = Cast<APawn>(Sprite->Actor);
		if( HasSpecialCoords && Pawn )
		{
			 // store current weapon position and rotation in pawn -- makes this accessable by script
			FCoords Ug = SpecialCoords.Inverse() / Frame->Coords;
			Pawn->WeaponRot =  Ug.OrthoRotation();
			Pawn->WeaponLoc =  Ug.Origin;
			// Draw weapon
			AInventory* Weapon = Pawn->Weapon;
			if( Weapon && Weapon->ThirdPersonMesh && !Weapon->bHidden )
			{
				Exchange( Weapon->ThirdPersonMesh, Weapon->Mesh );
				Exchange( Weapon->ThirdPersonScale, Weapon->DrawScale );
				Weapon->Rotation = FRotator(0,0,0);
				FLOAT Mirror  = Frame->Mirror;
				Frame->Mirror = 1;
				DrawMesh
				(
					Frame,
					Sprite,
					Weapon,
					SpecialCoords / Weapon->Rotation / Weapon->Location,
					GetPolyFlags(Frame,Weapon)
				);
				Exchange( Weapon->ThirdPersonMesh, Weapon->Mesh );
				Exchange( Weapon->ThirdPersonScale, Weapon->DrawScale );
				if ( Weapon->bSteadyFlash3rd )
					Weapon->bSteadyToggle = !Weapon->bSteadyToggle;
				if ( (Weapon->bSteadyFlash3rd && (!Weapon->bToggleSteadyFlash || Weapon->bSteadyToggle)) 
					|| (!Weapon->bFirstFrame && (Weapon->FlashCount != Weapon->OldFlashCount)) )
				{
					if ( Weapon->MuzzleFlashMesh )
					{
						// Hack for drawing 3rd person muzzle flash
						Exchange( Weapon->MuzzleFlashMesh, Weapon->Mesh );
						Exchange( Weapon->MuzzleFlashScale, Weapon->DrawScale );
						Exchange( Weapon->Style, Weapon->MuzzleFlashStyle );
						Exchange( Weapon->Texture, Weapon->MuzzleFlashTexture );
						FName WeapAnim = Weapon->AnimSequence;
						Weapon->AnimSequence = NAME_All;
						INT WeapLit = Weapon->bUnlit;
						FLOAT WeapFrame = Weapon->AnimFrame;
						Weapon->AnimFrame = appFrand();
						Weapon->bUnlit = true;

						DrawMesh
						(
							Frame,
							Sprite,
							Weapon,
							SpecialCoords / Weapon->Rotation / Weapon->Location,
							GetPolyFlags(Frame,Weapon)
						);

						Weapon->AnimSequence = WeapAnim;
						Weapon->bUnlit = WeapLit;
						Weapon->AnimFrame = WeapFrame;
						Exchange( Weapon->Texture, Weapon->MuzzleFlashTexture );
						Exchange( Weapon->MuzzleFlashMesh, Weapon->Mesh );
						Exchange( Weapon->MuzzleFlashScale, Weapon->DrawScale );
						Exchange( Weapon->Style, Weapon->MuzzleFlashStyle );
					}
				}
				Weapon->OldFlashCount = Weapon->FlashCount;
				Weapon->bFirstFrame = 0;
				Frame->Mirror = Mirror;
			}
			// Draw Flag
			if ( Pawn->PlayerReplicationInfo )
			{
				AActor* Flag = Pawn->PlayerReplicationInfo->HasFlag;
				if ( Flag )
				{
					FVector RealLoc = Flag->Location; // save old location and rotation to avoid network replication
					FRotator RealRot = Flag->Rotation;
					Flag->Rotation = Sprite->Actor->Rotation;
					float Dist = Clamp(2.f + 20.f * GMath.SinTab(Flag->Rotation.Pitch),2.f,3.f); 
					Flag->Location = Sprite->Actor->Location 
										- Dist * Sprite->Actor->CollisionRadius * Sprite->Actor->Rotation.Vector()
										+ FVector(0,0,0.7f * Pawn->BaseEyeHeight);
					FLOAT Mirror  = Frame->Mirror;
					Frame->Mirror = 1;
					DrawMesh
					(
						Frame,
						Sprite,
						Flag,
						Frame->Coords,
						GetPolyFlags(Frame,Flag)
					);
					Frame->Mirror = Mirror;
					Flag->Location = RealLoc;
					Flag->Rotation = RealRot;
				}
			}
		}

		unguard;
	}

	if( ((ShowFlags & SHOW_Bounds) && (Owner->bCollideWorld || Owner->bCollideActors) )
	|| (GIsEditor && Owner->bSelected /*&& !Frame->Viewport->IsOrtho()*/) )
	{
		// Draw collision cylinder.
		if( Owner->LightType!=LT_None && Owner->LightBrightness!=0 )
		{
			FLOAT Radius		= Owner->WorldLightRadius();
			FLOAT RadiusInner	= Radius * Max((INT)Owner->LightRadiusInner,4) / 256.0f;
			FLOAT Brightness	= Owner->LightBrightness/255.0f;

			FPlane Color;
			GlobalLighting( 0, Owner, Brightness, Color );
			Color *= Brightness * Owner->Level->Brightness;

			DrawCircle( Frame, Color, 0, Owner->Location, RadiusInner );
			DrawCircle( Frame, Color*0.5f, 0, Owner->Location, Radius );
		}
		else
		{
			FPlane Color(0, 1, 1, 0);
			Owner->GetPrimitive()->DrawCollisionBounds( Owner, this, Frame, Color, LINE_DepthCued );
		}

		// Also draw the axes, eh?
		FCoords Coords = Owner->ToLocal();
		Draw3DLine( Frame, FVector(1,0,0), LINE_DepthCued, 
			Coords.Origin, Coords.Origin + Coords.XAxis * (Owner->CollisionHeight/8) );
		Draw3DLine( Frame, FVector(0,1,0), LINE_DepthCued, 
			Coords.Origin, Coords.Origin + Coords.YAxis * (Owner->CollisionHeight/8) );
		Draw3DLine( Frame, FVector(0,0,1), LINE_DepthCued, 
			Coords.Origin, Coords.Origin + Coords.ZAxis * (Owner->CollisionHeight/8) );
	}

	if (ShowFlags & SHOW_Bounds && !Owner->bDeleteMe)
	{
		// Draw the render bounding box.
		FCoords BoxCoords = Owner->GetRenderBoundingBox( 1 );
		if( BoxCoords.XAxis.SizeSquared() != 0.f )
		{
			FPlane Color = Owner->DrawType == DT_Particles ? FPlane(1,0,0,0) : FPlane(1,1,1,0);
			DrawBox( Frame, Color, LINE_DepthCued, FBox(FVector(0), FVector(1)), BoxCoords );

			/*
			// Draw the sprite extents.
			Color = FPlane(0,0,1,0);
			Frame->Viewport->RenDev->Draw2DClippedLine( Frame, Color, 0, FVector(Sprite->X1,Sprite->Y1,0), FVector(Sprite->X1,Sprite->Y2,0) );
			Frame->Viewport->RenDev->Draw2DClippedLine( Frame, Color, 0, FVector(Sprite->X1,Sprite->Y2,0), FVector(Sprite->X2,Sprite->Y2,0) );
			Frame->Viewport->RenDev->Draw2DClippedLine( Frame, Color, 0, FVector(Sprite->X2,Sprite->Y2,0), FVector(Sprite->X2,Sprite->Y1,0) );
			Frame->Viewport->RenDev->Draw2DClippedLine( Frame, Color, 0, FVector(Sprite->X2,Sprite->Y1,0), FVector(Sprite->X1,Sprite->Y1,0) );
			*/
		}

	}

	// Done.
	POP_HIT(Frame);
	unguard;
}

/*-----------------------------------------------------------------------------
	Wireframe view drawing.
-----------------------------------------------------------------------------*/

//
// Just draw an actor, no span occlusion.
//
void URender::DrawActor( FSceneNode* Frame, AActor* Actor)
{
	guard(URender::DrawActor);
	FDynamicSprite Sprite(Actor);
	if( Sprite.Setup( Frame ) )
		DrawActorSprite( Frame, &Sprite );
	unguard;
}

/*------------------------------------------------------------------------------
	The End.
------------------------------------------------------------------------------*/

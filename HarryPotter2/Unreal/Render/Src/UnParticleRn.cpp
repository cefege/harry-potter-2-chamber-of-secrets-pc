//+--------------------------------------------------------------------------
//
//  Copyright (C) DreamWorks Interactive, 1999, 2000
//
//  File:       UnParticleRn.cpp
//
//  Contents:   Implementation of UnParticleRn.cpp
//
//  History:    
//
//	To do:		
//
//---------------------------------------------------------------------------

#include "Render.h"
#include "UnParticleList.h"
#include "UnParticle.h"
#include "hp2_rs.h"

// Layout shared with hp2-abi render_particle.rs (asserted there by layout tests).
static_assert( sizeof(FTransTexture) == 88, "FTransTexture must match hp2-abi FTransTexture (88 bytes)" );
static_assert( sizeof(FTransform) == 32, "FTransform must match hp2-abi FTransform (32 bytes)" );

// Parameters.

template<class T> inline 
void Swap(T& a, T& b)
//
// Swaps the two arguments.
//
{
	T temp = a;
	a = b;
	b = temp;
}

enum { FVF_OutAll = ( FVF_OutNear | FVF_OutXMin | FVF_OutYMin | FVF_OutXMax | FVF_OutYMax ) };

/*-----------------------------------------------------------------------------
	Particle rendering functions.
-----------------------------------------------------------------------------*/

inline void SetUV( UTexture* Texture, FTransTexture& tt, FLOAT u, FLOAT v )
{
	hp2_rs_render_particle_set_uv( &tt, Texture->USize, Texture->VSize, u, v );
}

// Frame fields the particle exports read, packed for the Rust side.
static inline ParticleFrame ParticleFrameOf( const FSceneNode* Frame )
{
	ParticleFrame pf;
	pf.proj_z  = Frame->Proj.Z;
	pf.rproj_z = Frame->RProj.Z;
	pf.fx15    = Frame->FX15;
	pf.fy15    = Frame->FY15;
	pf.prjxm   = Frame->PrjXM;
	pf.prjxp   = Frame->PrjXP;
	pf.prjym   = Frame->PrjYM;
	pf.prjyp   = Frame->PrjYP;
	return pf;
}

inline void ComputeClipCodes( FSceneNode* Frame, FTransTexture& tt )
{
	ParticleFrame pf = ParticleFrameOf( Frame );
	hp2_rs_render_particle_compute_clip_codes( &pf, &tt );
}

inline void SetTrans
(
	UTexture* Texture,
	FTransTexture& tt,
	FSceneNode* Frame,
	FLOAT screen_x,
	FLOAT screen_y,
	FLOAT screen_z,
	FLOAT u,
	FLOAT v
)
{
	checkSlow(Texture);
	ParticleFrame pf = ParticleFrameOf( Frame );
	hp2_rs_render_particle_set_trans( &tt, &pf, Texture->USize, Texture->VSize, screen_x, screen_y, screen_z, u, v );
}

inline BYTE CombineClipCodes( FTransTexture* Verts, INT N )
{
	return hp2_rs_render_particle_combine_clip_codes( Verts, N );
}

inline void SetTrans2D
(
	FTransTexture& tt,
	FSceneNode* Frame,
	FTransform& Point3D,
	FLOAT delta_x,
	FLOAT delta_y,
	FLOAT u,
	FLOAT v
)
{
	hp2_rs_render_particle_set_trans_2d( &tt, &Point3D, delta_x, delta_y, u, v );
}

inline BYTE ComputeClipCodes2D( FSceneNode* Frame, const FVector& C, FLOAT Dx, FLOAT Dy )
{
	ParticleFrame pf = ParticleFrameOf( Frame );
	return hp2_rs_render_particle_compute_clip_codes_2d( &pf, &C, Dx, Dy );
}

static void* NewTransTexture()
{
	return New<FTransTexture>(GMem);
}

static int ClipPolygon( FSceneNode* Frame, FTransTexture* Pts[], int LocalCount, BYTE code )
{
	ParticleFrame pf = ParticleFrameOf( Frame );
	return hp2_rs_render_particle_clip_polygon( &pf, (void**)Pts, LocalCount, code, NewTransTexture );
}

//
// Get poly flags for a particle system
//
DWORD GetPolyFlags( AParticleFX* ParticleFX )
{
	guardSlow(GetPolyFlags);
	
	DWORD PolyFlags=PF_Unlit;

	if     ( ParticleFX->Style==STY_Masked      ) PolyFlags |= PF_Masked;
	else if( ParticleFX->Style==STY_Translucent ) PolyFlags |= PF_Translucent;
	else if( ParticleFX->Style==STY_Modulated   ) PolyFlags |= PF_Modulated;

	return PolyFlags;
	unguardSlow;
}

class ParticleSystem
{
protected:
	AParticleFX*	ParticleFX;
	FSceneNode*		Frame;
	FSpanBuffer*	SpanBuffer;
	UTexture*		Texture;
	float			MaxParticleSize;
	float			Area, AreaRendered;

public:
	ParticleSystem( FSceneNode* Fr, FDynamicSprite* Sprite, URender* Render ) 
	:	ParticleFX( Cast<AParticleFX>(Sprite->Actor) ), Frame(Fr)
	,	SpanBuffer(Sprite->SpanBuffer), MaxParticleSize( Render->MaxParticleSize * Fr->FX * Fr->FY * 0.5f )
	,	Area(0.f), AreaRendered(0.f)
	{
		checkSlow(ParticleFX);
	}

	void ComputeLighting()
	{
		if( ParticleFX->bUnlit )
		{
			ParticleFX->LightColor = FPlane(1, 1, 1, 1);
		}
		else
		{
			GLightManager->SetupForActor( Frame, ParticleFX, GRender->LeafLights[ParticleFX->Region.iLeaf], NULL );
			GLightManager->LightParticleSystem( Frame, ParticleFX );
			GLightManager->FinishActor();
		}
	}

	FPlane	Color( UParticle* pparticle )
	{
		FPlane PColor( ParticleFX->LightColor.X * pparticle->Color.X,
					   ParticleFX->LightColor.Y * pparticle->Color.Y,
					   ParticleFX->LightColor.Z * pparticle->Color.Z,
					   1.f );
		FLOAT Alpha = pparticle->Alpha * ParticleFX->Opacity;
		if( Alpha < 1.f )
		{
			PColor *= Alpha;
			if( ParticleFX->Style == STY_Modulated )
				// Fade toward grey.
				PColor += FPlane( (1.f - Alpha) * 0.5f );
		}
		return PColor;
	}

	virtual void SetupColors( FTransTexture* verts, UParticle* pparticle, UParticle* )
	{
		verts[0].Light = verts[1].Light = verts[2].Light = verts[3].Light = Color( pparticle );
	}

	virtual BYTE SetupVerts(FTransTexture* verts, UParticle* pparticle, UParticle*) = 0;

	enum { NumPolys = 1 };
	enum { NumVerts = 4 };
	virtual int GetNumPolys() const { return 1; }
	virtual int GetNumVerts() const { return NumVerts; }

	void Render();
};

void ParticleSystem::Render()
{
	checkSlow(ParticleFX);

	Texture = ParticleFX->Textures[0];

	// Abort if required data cannot be found.
	if( !Texture || ParticleFX->ParticleList->Size() == 0 )
		return;
	
	// Lock the texture and get the FTextureInfo object.
	Texture = Texture->Get( Frame->Viewport->CurrentTime );

	FTextureInfo TextureInfo;
	{
		//FIX this means that all particles have the same texture ? 
		//3rd Param is LOD, -1 means auto adjust, otherwise is exact mip level
		Clock(GStat.ParticleRasterTime);
		Texture->Lock( TextureInfo, Frame->Viewport->CurrentTime, 0, Frame->Viewport->RenDev );
	}

	DWORD     PolyFlags = GetPolyFlags(ParticleFX) | Texture->PolyFlags;
	FPlane    Color     = FPlane(1,1,1,0);
	INT		  NumPolys  = GetNumPolys(),
			  NumVerts	= GetNumVerts();

	// Determine optimal buffer size. Create more than enough indices.
	int MaxVerts = Frame->Viewport->RenDev->MaxVertices();
	int MaxIndices = MaxVerts * 3;

	// Allow slop for clipping a single particle.
	int ClipVerts = NumPolys*NumVerts*3;

	FMemMark Mark(GMem);

	FTransTexture* AllVerts = New<FTransTexture>(GMem, MaxVerts);
	FTransTexture** PVerts = New<FTransTexture*>(GMem, MaxVerts);
	_WORD* Indices = New<_WORD>(GMem, MaxIndices);

	int VertCount = 0;
	int IndexCount = 0;

	// Do particle lighting if necessary
	ComputeLighting();

	// Begin rendering the particles.
	UParticle* pparticle = ParticleFX->ParticleList->StartParticle();
	UParticle* pNextParticle = NULL;

	for (; pparticle; pparticle = pNextParticle)
	{
		STAT(GStat.ParticlesUpdated++);

		pNextParticle = ParticleFX->ParticleList->NextParticle();

//		if ((ParticleFX->Distribution != DIST_Uniform) && (ParticleFX->LOD * 9) < pparticle->PriorityTag)
//			continue;

		if( VertCount >= MaxVerts-ClipVerts )
		{
			// Flush when needed.
			Clock(GStat.ParticleRasterTime);
			check( VertCount <= MaxVerts && IndexCount <= MaxIndices );
			Frame->Viewport->RenDev->DrawTriangles
			(
				Frame, TextureInfo,
				PVerts, VertCount, Indices, IndexCount,
				PolyFlags, SpanBuffer
			);
			VertCount = IndexCount = 0;
		}

		FTransTexture* Verts = AllVerts+VertCount;
		BYTE clip = SetupVerts(Verts, pparticle, pNextParticle);
		if( clip == FVF_OutAll )
			continue;

		// Setup vertex light values 
		SetupColors(Verts, pparticle, pNextParticle);

		// Create initial vert-pointer list for this particle.
		for_count( vv, NumVerts*NumPolys )
			PVerts[VertCount+vv] = &Verts[vv];

		for ( int p = 0; p < NumPolys; ++p, Verts+=NumVerts )
		{
			INT Count;
			if (clip)
				Count = ClipPolygon( Frame, PVerts+VertCount, NumVerts, clip );
			else
				Count = NumVerts;

			// Convert polygon to indexed tris.
			for_count( t, Count-2 )
			{
				Indices[IndexCount++] = VertCount;
				Indices[IndexCount++] = VertCount+t+1;
				Indices[IndexCount++] = VertCount+t+2;
			}

			// Advance VertCount by max of number of pre-allocated verts or vert pointers used.
			VertCount += Max(Count, NumVerts);
		}
		STAT(GStat.ParticlesSeen++);
		STAT(GStat.ParticlesRendered++);
	}

	// Finish.
	Clock(GStat.ParticleRasterTime);
	if( IndexCount > 0 )
	{
		checkSlow( VertCount <= MaxVerts && IndexCount <= MaxIndices );
		Frame->Viewport->RenDev->DrawTriangles
		(
			Frame, TextureInfo,
			PVerts, VertCount, Indices, IndexCount,
			PolyFlags, SpanBuffer
		);
	}

	STAT( GStat.ParticleArea += INT(Area) );
	STAT( GStat.ParticleAreaRendered += INT(AreaRendered) );

	Mark.Pop();
	Texture->Unlock(TextureInfo);
}


class ScreenParticles : public ParticleSystem
{

public:

	ScreenParticles(FSceneNode* f, FDynamicSprite* Sprite, URender* Render) : ParticleSystem(f, Sprite, Render) { }

	BYTE SetupVerts(FTransTexture* verts, UParticle* pparticle, UParticle*)
	{
		FLOAT rz = Frame->Proj.Z / pparticle->Position.Z;
		
		int SizeX = int(pparticle->Width*0.5f);
		int SizeY = int(pparticle->Length*0.5f);
		
		SetTrans(Texture, verts[0], Frame, pparticle->Position.X - SizeX , pparticle->Position.Y + SizeY, rz, 0.0f, 1.0f);
		SetTrans(Texture, verts[2], Frame, pparticle->Position.X + SizeX , pparticle->Position.Y - SizeY, rz, 1.0f, 0.0f);
		SetTrans(Texture, verts[1], Frame, pparticle->Position.X - SizeX , pparticle->Position.Y - SizeY, rz, 0.0f, 0.0f);
		SetTrans(Texture, verts[3], Frame, pparticle->Position.X + SizeX , pparticle->Position.Y + SizeY, rz, 1.0f, 1.0f);

		return CombineClipCodes( verts, NumVerts );
	}
};


class ShardParticles : public ParticleSystem
{

public:
	enum { NumVerts = 3 };
	virtual int GetNumVerts() const { return NumVerts; }

	ShardParticles(FSceneNode* f, FDynamicSprite* Sprite, URender* Render) : ParticleSystem(f, Sprite, Render) { }

	BYTE SetupVerts(FTransTexture* verts, UParticle* pparticle, UParticle*)
	{
		ParticleFrame pf = ParticleFrameOf( Frame );
		return hp2_rs_render_particle_shard_setup_verts( GRenderServices, &pf, &Frame->Coords, Texture->USize, Texture->VSize, &pparticle->Position, pparticle->Spin, pparticle->Length, pparticle->Width, verts );
	}
};


class LiquidParticles : public ParticleSystem
{
	FTransform	Center;

public:
	LiquidParticles(FSceneNode* f, FDynamicSprite* Sprite, URender* Render) : ParticleSystem(f, Sprite, Render) { }

	BYTE SetupVerts(FTransTexture* verts, UParticle* pparticle, UParticle*)
	{
		ParticleFrame pf = ParticleFrameOf( Frame );
		return hp2_rs_render_particle_liquid_setup_verts( GRenderServices, &pf, &Frame->Coords, Texture->USize, Texture->VSize, &Center, &pparticle->Position, &pparticle->Velocity, pparticle->Length, pparticle->Width, pparticle->DripTimer, verts );
	}
};


class BillboardParticles : public ParticleSystem
{
	FTransform	Center;

public:
	BillboardParticles(FSceneNode* f, FDynamicSprite* Sprite, URender* Render) : ParticleSystem(f, Sprite, Render) { }

	BYTE SetupVerts(FTransTexture* verts, UParticle* pparticle, UParticle*)
	{
		// Get Particle location in screenspace and use as Center of Quad
		Center.Point = pparticle->Position.TransformPointBy(Frame->Coords);
		if( Center.Point.Z < 1.f )
			return FVF_OutAll;
		Center.Project( Frame );
		
		FLOAT DX = pparticle->Width*0.5f;
		FLOAT DY = pparticle->Length*0.5f;
		FLOAT TX = Texture->USize-1;
		FLOAT TY = Texture->VSize-1;

		// Fill in screenspace coordinates, Texture, Texture uv's, and RZ
		BYTE Clip;
		if ( pparticle->SpinRate != 0.0f )
		{
			FLOAT Cos = GMath.CosFloat(pparticle->Spin), 
				  Sin = GMath.SinFloat(pparticle->Spin);
			SetTrans2D(verts[0], Frame, Center, -DX*Cos + DY*Sin, -DX*Sin - DY*Cos, 0.0f, TY);
			SetTrans2D(verts[1], Frame, Center,  DX*Cos + DY*Sin,  DX*Sin - DY*Cos, TX, TY);
			SetTrans2D(verts[2], Frame, Center,  DX*Cos - DY*Sin,  DX*Sin + DY*Cos, TX, 0.0f);
			SetTrans2D(verts[3], Frame, Center, -DX*Cos - DY*Sin, -DX*Sin + DY*Cos, 0.0f, 0.0f);

			Cos = Abs(Cos);
			Sin = Abs(Sin);
			FLOAT MaxDX = Cos*DX + Sin*DY,
				  MaxDY = Cos*DY + Sin*DX;
			Clip = ComputeClipCodes2D( Frame, Center.Point, MaxDX, MaxDY );
		}
		else
		{
			SetTrans2D(verts[0], Frame, Center, -DX, -DY, 0.0f, TY);
			SetTrans2D(verts[1], Frame, Center,  DX, -DY, TX, TY);
			SetTrans2D(verts[2], Frame, Center,  DX,  DY, TX, 0.0f);
			SetTrans2D(verts[3], Frame, Center, -DX,  DY, 0.0f, 0.0f);

			Clip = ComputeClipCodes2D( Frame, Center.Point, DX, DY );
		}

		if( Clip != FVF_OutAll )
		{
			// Modify alpha by screen-size.
			FLOAT ScrSize = pparticle->Width * pparticle->Length * Square(Center.RZ);
			Area += ScrSize;

			// Randomise screen-size a bit so that all particles of a given size aren't culled.
			FLOAT TestScrSize = ScrSize*(pparticle->PriorityTag+1)*0.25;
			if( TestScrSize > MaxParticleSize )
			{
				// Fade particle out early.
				FLOAT NewAlpha = (2.f*MaxParticleSize - TestScrSize) / MaxParticleSize;
				pparticle->Alpha = Min( pparticle->Alpha, NewAlpha );
				if( pparticle->Alpha <= 0.f )
				{
					STAT(GStat.ParticlesSeen++);
					return FVF_OutAll;
				}
			}
			AreaRendered += ScrSize;
		}
		return Clip;
	}
};

// Particle record passed to hp2_rs_render_particle_tube_setup_verts and the poly-strip export.
// Layout mirrors hp2_render::particle_tube::TubeParticle (20 bytes).
struct TubeParticleRecord
{
	FLOAT X, Y, Z, Width, Length;
};
static_assert( sizeof(TubeParticleRecord) == 20, "TubeParticleRecord must match hp2_render::particle_tube::TubeParticle" );

static inline TubeParticleRecord TubeOf( const UParticle* P )
{
	TubeParticleRecord R = { P->Position.X, P->Position.Y, P->Position.Z, P->Width, P->Length };
	return R;
}


class TriTubeParticles: public ParticleSystem
{
	UParticle*		pPrevParticle;
	FVector			PrevStripVect;

public:
	enum { NumPolys = 3 };
	virtual int GetNumPolys() const { return NumPolys; }

	TriTubeParticles(FSceneNode* f, FDynamicSprite* Sprite, URender* Render) : ParticleSystem(f, Sprite, Render), pPrevParticle(NULL), PrevStripVect(0.f, 0.f, 0.f) { }

	void SetupColors( FTransTexture* verts, UParticle* pCurrParticle, UParticle* pNextParticle )
	{
		if( pNextParticle )
		{
			verts[0].Light = verts[3].Light = verts[4].Light = verts[7].Light = verts[8].Light = verts[11].Light = Color( pCurrParticle );
			verts[1].Light = verts[2].Light = verts[5].Light = verts[6].Light = verts[9].Light = verts[10].Light = Color( pNextParticle );
		}
		else
		{
			verts[0].Light = verts[1].Light = verts[2].Light  = verts[3].Light  = 
			verts[4].Light = verts[5].Light = verts[6].Light  = verts[7].Light  = 
			verts[8].Light = verts[9].Light = verts[10].Light = verts[11].Light = Color( pCurrParticle );
		}
	}

	BYTE SetupVerts( FTransTexture* verts, UParticle* pCurrParticle, UParticle* pNextParticle )
	{
		ParticleFrame pf = ParticleFrameOf( Frame );
		TubeParticleRecord curr = TubeOf( pCurrParticle );
		TubeParticleRecord prev = pPrevParticle ? TubeOf( pPrevParticle ) : TubeParticleRecord();
		TubeParticleRecord next = pNextParticle ? TubeOf( pNextParticle ) : TubeParticleRecord();
		BYTE Clip = hp2_rs_render_particle_tube_setup_verts( GRenderServices, &pf, &Frame->Coords, Texture->USize, Texture->VSize, pPrevParticle ? &prev : NULL, &curr, pNextParticle ? &next : NULL, &PrevStripVect, verts );
		// remember current particle for the next call
		pPrevParticle = pCurrParticle;
		return Clip;
	}
};


class PolyStripParticles : public ParticleSystem
{
	UParticle*		pPrevParticle;
	FVector			PrevEdgeVect;
	FVector			PrevStripVect;

public:

	PolyStripParticles(FSceneNode* f, FDynamicSprite* Sprite, URender* Render) : ParticleSystem(f, Sprite, Render), pPrevParticle(NULL), PrevEdgeVect(0.f, 0.f, 0.f), PrevStripVect(0.f, 0.f, 0.f) { }

	void SetupColors( FTransTexture* verts, UParticle* pCurrParticle, UParticle* pNextParticle )
	{
		if( pNextParticle )
		{
			verts[1].Light = verts[2].Light = Color( pNextParticle );
			verts[0].Light = verts[3].Light = Color( pCurrParticle );
		}
		else
		{
			verts[0].Light = verts[1].Light = verts[2].Light = verts[3].Light = Color( pCurrParticle );
		}
	}

	BYTE SetupVerts( FTransTexture* verts, UParticle* pCurrParticle, UParticle* pNextParticle )
	{
		ParticleFrame pf = ParticleFrameOf( Frame );
		TubeParticleRecord curr = TubeOf( pCurrParticle );
		TubeParticleRecord prev = pPrevParticle ? TubeOf( pPrevParticle ) : TubeParticleRecord();
		TubeParticleRecord next = pNextParticle ? TubeOf( pNextParticle ) : TubeParticleRecord();
		BYTE Clip = hp2_rs_render_particle_poly_strip_setup_verts( GRenderServices, &pf, &Frame->Coords, Texture->USize, Texture->VSize, pPrevParticle ? &prev : NULL, &curr, pNextParticle ? &next : NULL, &PrevStripVect, &PrevEdgeVect, verts );
		// remember current particle for the next call
		pPrevParticle = pCurrParticle;
		return Clip;
	}
};


void URender::DrawParticleSystem( FSceneNode* Frame, FDynamicSprite* Sprite )
{
	guard(URender::DrawParticleSystem);
	checkSlow(Sprite);
	checkSlow(Sprite->Actor);

	if( !Sprite->Actor->GetLevel()->Engine->Client->ParticleDensity )
		return;

	Clock(GStat.ParticleTime);

	AParticleFX* ParticleFX = Cast<AParticleFX>(Sprite->Actor);
	checkSlow(ParticleFX);

	// Move the simulation forward by one timestep.
	if ( !ParticleFX->Update() )
		return;

	// Call the appropriate method for our RenderPrimitive.
	STAT(clock(GStat.ParticleRenderTime));
	switch(ParticleFX->RenderPrimitive)
	{
		case PPRIM_Line: 
		{
			PolyStripParticles System(Frame, Sprite, this);
			System.Render();
		}
		case PPRIM_Shard:
		{
			ShardParticles System(Frame, Sprite, this);
			System.Render();
		}
		case PPRIM_Liquid:
		{
			LiquidParticles System(Frame, Sprite, this);
			System.Render();
		}
		case PPRIM_Billboard:
			if ( !ParticleFX->bShellOnly )
			{
				BillboardParticles System(Frame, Sprite, this);
				System.Render();
			}
			else
			{
				ScreenParticles System(Frame, Sprite, this);
				System.Render();
			}
			break;

		case PPRIM_TriTube:
		{
			TriTubeParticles System(Frame, Sprite, this);
			System.Render();
		}
		default:
			// we have a problem
			check(0);	
			break;
	}
	STAT(unclock(GStat.ParticleRenderTime));

	unguard;
}



/*------------------------------------------------------------------------------
	The End.
------------------------------------------------------------------------------*/

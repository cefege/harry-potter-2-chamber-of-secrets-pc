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

// Parameters.

//#define DEBUG_CLIPPING
#ifdef DEBUG_CLIPPING
	#define ADJUST_EDGE(f)	((f)*0.75)
#else
	#define ADJUST_EDGE(f)	(f)
#endif


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

/*------------------------------------------------------------------------------
	Function prototypes.
------------------------------------------------------------------------------*/

// Set up the particle using the screen space vector.
void ScreenVector
(
	const UParticle& Particle,	// Particle.
	UTexture* Texture,			// Texture.
	FTransTexture* Pts,			// Points. Four should be defined for a quad.
	FSceneNode* Frame			// Scene for projection.
);

/*-----------------------------------------------------------------------------
	Particle rendering functions.
-----------------------------------------------------------------------------*/

inline void ComputeClipCodes( FSceneNode* Frame, FTransTexture& tt )
{
#ifdef DEBUG_CLIPPING
	FLOAT PrjXM = Frame->PrjXM;
	FLOAT PrjXP = Frame->PrjXP;
	FLOAT PrjYM = Frame->PrjYM;
	FLOAT PrjYP = Frame->PrjYP;

	Frame->PrjXM = ADJUST_EDGE(Frame->PrjXM);
	Frame->PrjXP = ADJUST_EDGE(Frame->PrjXP);
	Frame->PrjYM = ADJUST_EDGE(Frame->PrjYM);
	Frame->PrjYP = ADJUST_EDGE(Frame->PrjYP);
#endif

	tt.ComputeOutcode( Frame );

#ifdef DEBUG_CLIPPING
	Frame->PrjXM = PrjXM;
	Frame->PrjXP = PrjXP;
	Frame->PrjYM = PrjYM;
	Frame->PrjYP = PrjYP;
#endif
}

inline void SetUV( UTexture* Texture, FTransTexture& tt, FLOAT u, FLOAT v )
{
	tt.U = FLOAT(Texture->USize - 1) * u;
	tt.V = FLOAT(Texture->VSize - 1) * v;
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
	
	// FTransform:
	tt.ScreenX = screen_x;
	tt.ScreenY = screen_y;
	tt.IntY    = appFloor(tt.ScreenY);
	tt.RZ      = screen_z;

	// FTransTexture:
	SetUV( Texture, tt, u, v );

	tt.Point.Z = Frame->Proj.Z / screen_z;
	tt.Point.X = (screen_x - Frame->FX15) * Frame->RProj.Z * tt.Point.Z;
	tt.Point.Y = (screen_y - Frame->FY15) * Frame->RProj.Z * tt.Point.Z;

	ComputeClipCodes( Frame, tt );
}

inline BYTE CombineClipCodes( FTransTexture* Verts, INT N )
{
	BYTE clip = Verts[0].Flags;
	BYTE cull = Verts[0].Flags;
	for( int c = 1; c < N; ++c )
	{
		cull &= Verts[c].Flags;
		clip |= Verts[c].Flags;
	}
	return cull ? FVF_OutAll : clip;
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
	static_cast<FTransform&>(tt) = Point3D;

	// FOutVector:
	tt.Point.X += delta_x;
	tt.Point.Y += delta_y;

	// FTransform:
	tt.ScreenX += delta_x*tt.RZ;
	tt.ScreenY += delta_y*tt.RZ;
	tt.IntY    = appFloor(tt.ScreenY);

	// FTransTexture:
	tt.U = u;
	tt.V = v;
}

inline BYTE ComputeClipCodes2D( FSceneNode* Frame, const FVector& C, FLOAT Dx, FLOAT Dy )
{
	static const BYTE OutXMinTab [2] = { 0, FVF_OutXMin };
	static const BYTE OutXMaxTab [2] = { 0, FVF_OutXMax };
	static const BYTE OutYMinTab [2] = { 0, FVF_OutYMin };
	static const BYTE OutYMaxTab [2] = { 0, FVF_OutYMax };

	// Test rejection first.
	FLOAT ClipXM = ADJUST_EDGE(Frame->PrjXM) * C.Z + C.X + Dx;
	FLOAT ClipXP = ADJUST_EDGE(Frame->PrjXP) * C.Z - C.X + Dx;
	FLOAT ClipYM = ADJUST_EDGE(Frame->PrjYM) * C.Z + C.Y + Dy;
	FLOAT ClipYP = ADJUST_EDGE(Frame->PrjYP) * C.Z - C.Y + Dy;
	if( IsNegativeFloat(ClipXM) | IsNegativeFloat(ClipXP)
	  | IsNegativeFloat(ClipYM) | IsNegativeFloat(ClipYP) )
	   return FVF_OutAll;

	// Compute composite clip code.
	ClipXM -= 2*Dx;
	ClipXP -= 2*Dx;
	ClipYM -= 2*Dy;
	ClipYP -= 2*Dy;
	return 
	(	OutXMinTab [IsNegativeFloat(ClipXM)]
	+	OutXMaxTab [IsNegativeFloat(ClipXP)]
	+	OutYMinTab [IsNegativeFloat(ClipYM)]
	+	OutYMaxTab [IsNegativeFloat(ClipYP)]);
}



static inline void lerp( FLOAT f, const FVector& a, const FVector& b, FVector& out )
{
	out.X = a.X + f * (b.X - a.X);
	out.Y = a.Y + f * (b.Y - a.Y);
	out.Z = a.Z + f * (b.Z - a.Z);
}

static inline void lerp( FLOAT f, const FPlane& a, const FPlane& b, FPlane& out )
{
	out.X = a.X + f * (b.X - a.X);
	out.Y = a.Y + f * (b.Y - a.Y);
	out.Z = a.Z + f * (b.Z - a.Z);
	out.W = a.W + f * (b.W - a.W);
}

static inline void lerp( FLOAT f, const FTransTexture* a, const FTransTexture* b, FTransTexture* out )
{
	out->U			= a->U			+ f * (b->U 		- a->U);
	out->V			= a->V			+ f * (b->V 		- a->V);
	lerp( f, a->Point, b->Point, out->Point );
	lerp( f, a->Light, b->Light, out->Light );
}

struct ClipToPlane
{
	FSceneNode* Frame;
	
	ClipToPlane(FSceneNode* f) : Frame(f) { }

	void	interpolate( FLOAT f, const FTransTexture* a, const FTransTexture* b, FTransTexture* out ) const
	{
		lerp( f, a, b, out);
		out->Project(Frame);
	}
};

struct ClipNearPlane : public ClipToPlane
{
	enum { Mask = FVF_OutNear };

	FLOAT d;

	ClipNearPlane(FSceneNode* f) : ClipToPlane(f), d(1) { }
	FLOAT 	distance( FTransTexture* pt ) const { return pt->Point.Z - d; }
};

struct ClipLeftPlane : public ClipToPlane
{
	enum { Mask = FVF_OutXMin };

	FLOAT d;

	ClipLeftPlane(FSceneNode* f) : ClipToPlane(f), d(ADJUST_EDGE(Frame->PrjXM)) { }
	FLOAT	distance(FTransTexture* pt) const { return d * pt->Point.Z + pt->Point.X; }
};

struct ClipRightPlane : public ClipToPlane
{
	enum { Mask = FVF_OutXMax };

	FLOAT d;

	ClipRightPlane(FSceneNode* f) : ClipToPlane(f), d(ADJUST_EDGE(Frame->PrjXP)) { }
	FLOAT 	distance(FTransTexture* pt) const { return d * pt->Point.Z - pt->Point.X; }
};

struct ClipTopPlane : public ClipToPlane
{
	enum { Mask = FVF_OutYMin };

	FLOAT d;

	ClipTopPlane(FSceneNode* f) : ClipToPlane(f), d(ADJUST_EDGE(Frame->PrjYM)) { }
	FLOAT 	distance(FTransTexture* pt) const { return d * pt->Point.Z + pt->Point.Y; }
};

struct ClipBottomPlane : public ClipToPlane
{
	enum { Mask = FVF_OutYMax };

	FLOAT d;

	ClipBottomPlane(FSceneNode* f) : ClipToPlane(f), d(ADJUST_EDGE(Frame->PrjYP)) { }
	FLOAT	distance(FTransTexture* pt) const { return d * pt->Point.Z - pt->Point.Y; }
};

template< class Plane >
int Clip( const Plane& edge, FTransTexture** Out, FTransTexture** In, int Count )
{
	if( Count < 3 )
		return 0;

	int dstCount = 0;

	for( int i=0, j=Count-1; i < Count; j=i++ )
	{
		FLOAT dist1 = edge.distance(In[i]);
		FLOAT dist2 = edge.distance(In[j]);
		if ( (dist1 * dist2) < 0 ) // opposite sides of plane?
		{
			FLOAT f = dist1 / ( dist1 - dist2 );
			Out[dstCount] = New<FTransTexture>(GMem);
			edge.interpolate(f, In[i], In[j], Out[dstCount++]);
		}
		if ( dist1 >= 0 )
		{
			Out[dstCount++] = In[i];
		}
	}

	return dstCount;
}

static int ClipPolygon( FSceneNode* Frame, FTransTexture* Pts[], int LocalCount, BYTE code )
{
	FTransTexture* LocalPts[2][64];
	int i = 0;

	for_count( p, LocalCount )
		LocalPts[i][p] = Pts[p];

	if( code & FVF_OutNear )
	{
		LocalCount = Clip( ClipNearPlane(Frame), LocalPts[i^1], LocalPts[i], LocalCount );
		i ^= 1;
		LocalCount = Clip( ClipLeftPlane(Frame), LocalPts[i^1], LocalPts[i], LocalCount );
		i ^= 1;
		LocalCount = Clip( ClipTopPlane(Frame), LocalPts[i^1], LocalPts[i], LocalCount );
		i ^= 1;
		LocalCount = Clip( ClipRightPlane(Frame), LocalPts[i^1], LocalPts[i], LocalCount );
		i ^= 1;
		LocalCount = Clip( ClipBottomPlane(Frame), LocalPts[i^1], LocalPts[i], LocalCount );
		i ^= 1;
	}
	else 
	{
		if( code & FVF_OutXMin )
		{
			LocalCount = Clip( ClipLeftPlane(Frame), LocalPts[i^1], LocalPts[i], LocalCount );
			i ^= 1;
		}
		if( code & FVF_OutYMin )
		{
			LocalCount = Clip( ClipTopPlane(Frame), LocalPts[i^1], LocalPts[i], LocalCount );
			i ^= 1;
		}
		if( code & FVF_OutXMax )
		{
			LocalCount = Clip( ClipRightPlane(Frame), LocalPts[i^1], LocalPts[i], LocalCount );
			i ^= 1;
		}
		if( code & FVF_OutYMax )
		{
			LocalCount = Clip( ClipBottomPlane(Frame), LocalPts[i^1], LocalPts[i], LocalCount );
			i ^= 1;
		}
	}

	if( LocalCount > 2 )
	{
		// Copy back.
		for_count( p, LocalCount )
			Pts[p] = LocalPts[i][p];

		return LocalCount;
	}
	else
		return 0;
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
		FCoords to_world = GMath.UnitCoords * pparticle->Position * FRotator(pparticle->Spin*65535, pparticle->Spin*65535, 0);

		FVector V1_Local(	pparticle->Length*0.5f, pparticle->Width*0.5f, 0.0f );
		FVector V0_Local(	-pparticle->Length*0.5f, -pparticle->Width*0.5f, 0.0f );
		FVector V2_Local(	-pparticle->Length*0.5f, pparticle->Width*0.5f, 0.0f );
		
		FVector V0_World, V1_World, V2_World;
		
		// Transform local space offsets to World Space		
		V0_World = V0_Local.TransformPointBy(to_world);
		V1_World = V1_Local.TransformPointBy(to_world);
		V2_World = V2_Local.TransformPointBy(to_world);

		// Transform to Camera space
		verts[0].Point = V0_World.TransformPointBy(Frame->Coords);
		verts[1].Point = V1_World.TransformPointBy(Frame->Coords);
		verts[2].Point = V2_World.TransformPointBy(Frame->Coords);

		// Project to Screen space
		verts[0].Project(Frame);
		verts[1].Project(Frame);
		verts[2].Project(Frame);

		//
		//        
		//        1           / \
		//       / \           |
		//      /   \          |  
		//     /     \         | L      
		//    /   *   \        |     
		//   /         \       |    
		//  /           \      |    
		// 0-------------2    \ /           
		// 
		// <----- W ----->     
	
		SetUV(Texture, verts[0], 0.0f, 0.0f); 
		SetUV(Texture, verts[1], 1.0f, 0.0f);
		SetUV(Texture, verts[2], 1.0f, 1.0f);

		ComputeClipCodes(Frame, verts[0]); 
		ComputeClipCodes(Frame, verts[1]);
		ComputeClipCodes(Frame, verts[2]);

		return CombineClipCodes( verts, NumVerts );
	}
};


class LiquidParticles : public ParticleSystem
{
	FTransform	Center;

public:
	LiquidParticles(FSceneNode* f, FDynamicSprite* Sprite, URender* Render) : ParticleSystem(f, Sprite, Render) { }

	BYTE SetupVerts(FTransTexture* verts, UParticle* pparticle, UParticle*)
	{
		Center.Point = pparticle->Position.TransformPointBy(Frame->Coords);
		Center.Project(Frame);

		if ( pparticle->DripTimer > 0.0f )
		{
			FLOAT WidthPerspDiv2 = pparticle->Width*0.5f*Center.RZ;
			FLOAT LengthPerspDiv2 = pparticle->Length*0.5f*Center.RZ;
				
			SetTrans(Texture, verts[0], Frame, 
								Center.ScreenX,				 Center.ScreenY,					  Center.RZ, 0.0f, 1.0f); 
			SetTrans(Texture, verts[2], Frame, 
								Center.ScreenX,				 Center.ScreenY+LengthPerspDiv2*2.0f, Center.RZ, 1.0f, 0.0f);
			SetTrans(Texture, verts[3], Frame, 
								Center.ScreenX-WidthPerspDiv2, Center.ScreenY+1.5f*LengthPerspDiv2, Center.RZ, 0.0f, 0.0f);
			SetTrans(Texture, verts[1], Frame, 
								Center.ScreenX+WidthPerspDiv2, Center.ScreenY+1.5f*LengthPerspDiv2, Center.RZ, 1.0f, 1.0f);
		}
		else
		{
			// calculate a screen perspective view of particle with streak
			ScreenVector(*pparticle, Texture, verts, Frame);
		}

		return CombineClipCodes( verts, NumVerts );
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


class TriTubeParticles: public ParticleSystem
{
	UParticle*		pPrevParticle;
	FVector			PrevStripVect;

public:
	enum { NumPolys = 3 };
	virtual int GetNumPolys() const { return NumPolys; }

	TriTubeParticles(FSceneNode* f, FDynamicSprite* Sprite, URender* Render) : ParticleSystem(f, Sprite, Render) { }

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
		// first time through only setup vectors from prev to current for next iteration
		if (!pPrevParticle)
		{
			if (pNextParticle)
			{
				//Vector from next particle back to this particle
				PrevStripVect = pNextParticle->Position - pCurrParticle->Position;
//				PrevStripVect.Normalize();
				PrevStripVect *= PrevStripVect.DivSizeApprox();
			}
			pPrevParticle = pCurrParticle;
			return false;
		}

		//-------------------------------------------------		
		// 
		//             3,4 __________________ 2,5
		//             / \                    . \	
		//            /   \                  .   \ 
		//           /     \                .     \
		//      0,11 ------- 7,8 _____ 1,10 _______ 6,9
		//
		//-------------------------------------------------

		//fix ? Spin is not currently used for TriTube
		FCoords to_world = GMath.UnitCoords * pPrevParticle->Position * PrevStripVect.Rotation();

		//left
		FVector V0_Local( 0.0f ,	-pPrevParticle->Width*0.5f,	-pPrevParticle->Length*0.5f);
		//top
		FVector V3_Local( 0.0f ,	0.0f,						pPrevParticle->Length*0.5f);
		//right
		FVector V7_Local( 0.0f ,	pPrevParticle->Width*0.5f,	-pPrevParticle->Length*0.5f);
		
		FVector V0_World, V3_World, V7_World;
		
		// Transform local space offsets to World Space		
		V0_World = V0_Local.TransformPointBy(to_world);
		V3_World = V3_Local.TransformPointBy(to_world);
		V7_World = V7_Local.TransformPointBy(to_world);

		// Transform to Camera space
		verts[0].Point = V0_World.TransformPointBy(Frame->Coords);
		verts[3].Point = V3_World.TransformPointBy(Frame->Coords);
 		verts[7].Point = V7_World.TransformPointBy(Frame->Coords);
		
		// Project to Screen space
		verts[0].Project(Frame);
		verts[3].Project(Frame);
		verts[7].Project(Frame);

		// 11 & 0 only share position, not UV's
		verts[11] = verts[0];

		FVector StripVect;
		
		// see if we have another particle coming up, if so, we need to orientate our edge with respect to it, not last 
		if ( pNextParticle )
		{
			// recalculate Strip vector
			StripVect = pNextParticle->Position - pCurrParticle->Position;
			//StripVect.Normalize();
			StripVect *= StripVect.DivSizeApprox();
		}
		else
			// keep last orientation current
			StripVect = PrevStripVect;

		//fix ? Spin is not currently used for TriTube
		to_world = GMath.UnitCoords * pCurrParticle->Position * StripVect.Rotation();

		//left
		FVector V1_Local( 0.0f,		-pCurrParticle->Width*0.5f, -pCurrParticle->Length*0.5f);
		//top
		FVector V2_Local( 0.0f,		0.0f,						pCurrParticle->Length*0.5f);
		//right
		FVector V6_Local( 0.0f,		pCurrParticle->Width*0.5f,	-pCurrParticle->Length*0.5f);
		
		FVector V1_World, V2_World, V6_World;
		
		// Transform local space offsets to World Space		
		V1_World = V1_Local.TransformPointBy(to_world);
		V2_World = V2_Local.TransformPointBy(to_world);
		V6_World = V6_Local.TransformPointBy(to_world);

		// Transform to Camera space
		verts[1].Point = V1_World.TransformPointBy(Frame->Coords);
		verts[2].Point = V2_World.TransformPointBy(Frame->Coords);
 		verts[6].Point = V6_World.TransformPointBy(Frame->Coords);

		// Project to Screen space
		verts[1].Project(Frame);
		verts[2].Project(Frame);
		verts[6].Project(Frame);

		// 10 & 1 only share position, not UV's
		verts[10] = verts[1];

		// fill in the FTransTexture structure ( most of it is already correct )
		// back triangle
		SetUV(Texture, verts[0],	0.00f ,	0.00f);
		SetUV(Texture, verts[3],	0.33f,	0.00f);
		SetUV(Texture, verts[7],	0.66f,	0.00f);
		SetUV(Texture, verts[11],	1.00f,	0.00f);

		// front triangle
		SetUV(Texture, verts[1],	0.00f,	1.0f);
		SetUV(Texture, verts[2],	0.33f,	1.0f);
		SetUV(Texture, verts[6],	0.66f,	1.0f);
		SetUV(Texture, verts[10],	1.00f,	1.0f);

		// compute clip codes for all unique verts
		ComputeClipCodes(Frame, verts[0]);
		ComputeClipCodes(Frame, verts[3]);
		ComputeClipCodes(Frame, verts[7]);
		ComputeClipCodes(Frame, verts[11]);
		ComputeClipCodes(Frame, verts[1]);
		ComputeClipCodes(Frame, verts[2]);
		ComputeClipCodes(Frame, verts[6]);
		ComputeClipCodes(Frame, verts[10]);

		// 4,3   8,7   5,2   9,6  are exact matches ( even UV coordinates )
		verts[4]  = verts[3];
		verts[8]  = verts[7];
		
		verts[5]  = verts[2];
		verts[9]  = verts[6];

		FVector Camera(pCurrParticle->Position - Frame->Coords.Origin);
		//Camera.Normalize();
		Camera *= Camera.DivSizeApprox();

		{
		FVector Side(V3_World - V0_World);
		FVector  Top(V0_World - V1_World);

		FVector SurfaceNormal(Top ^ Side);
		//SurfaceNormal.Normalize();
		SurfaceNormal *= SurfaceNormal.DivSizeApprox();

		if ((SurfaceNormal | Camera) <= 0.0f )
			verts[0].Flags = verts[1].Flags = verts[2].Flags  = verts[3].Flags  = FVF_OutAll;
		}

		{
		FVector Side(V7_World - V3_World);
		FVector Top(V3_World-V2_World);

		FVector SurfaceNormal(Top ^ Side);
		//SurfaceNormal.Normalize();
		SurfaceNormal *= SurfaceNormal.DivSizeApprox();

		if ((SurfaceNormal | Camera) <= 0.0f )
			verts[4].Flags = verts[5].Flags = verts[6].Flags  = verts[7].Flags  = FVF_OutAll;
		}
		
		{
		FVector Side(V0_World - V7_World);
		FVector Top(V7_World-V6_World);

		FVector SurfaceNormal(Top ^ Side);
		//SurfaceNormal.Normalize();
		SurfaceNormal *= SurfaceNormal.DivSizeApprox();

		if ((SurfaceNormal | Camera) <= 0.0f )
			verts[8].Flags = verts[9].Flags = verts[10].Flags = verts[11].Flags = FVF_OutAll;
		}

		// remember current orientation
		pPrevParticle = pCurrParticle;
		PrevStripVect = StripVect;

		return CombineClipCodes( verts, NumVerts*NumPolys );
	}
};


class PolyStripParticles : public ParticleSystem
{
	UParticle*		pPrevParticle;
	FVector			PrevEdgeVect;
	FVector			PrevStripVect;

public:

	PolyStripParticles(FSceneNode* f, FDynamicSprite* Sprite, URender* Render) : ParticleSystem(f, Sprite, Render) { }

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
		// first time through only setup vectors from prev to current for next iteration
		if (!pPrevParticle)
		{
			if (pNextParticle)
			{
				//Vector from camera to first particle
				FVector LookVect = pCurrParticle->Position - Frame->Coords.Origin; 
				//LookVect.Normalize();
				LookVect *= LookVect.DivSizeApprox();
		
				//Vector from next particle back to this particle
				PrevStripVect = pNextParticle->Position - pCurrParticle->Position;
				//PrevStripVect.Normalize();
				PrevStripVect *= PrevStripVect.DivSizeApprox();
		
				//Cross product of Look and Strip gives us a vect perpendicular to both, hence maximized with respect to camera
				PrevEdgeVect = LookVect ^ PrevStripVect;
				//PrevEdgeVect.Normalize();
				PrevEdgeVect *= PrevEdgeVect.DivSizeApprox();

			}

			pPrevParticle = pCurrParticle;
			return false;
		}

		//-------------------------------------------
		//
		//			    1-*-2		* 2nd particle
		//			    | |/|
		//			    | /	|	
		//			    |/|	|
		//			    0-*-3		* 1st particle
		//
		//-------------------------------------------
		
		FVector EdgeVect = PrevEdgeVect * pPrevParticle->Width;
		FVector StripVect = PrevStripVect;

		//Calculate 2 edge points
		FVector Edges[4];
		Edges[0] = pPrevParticle->Position - EdgeVect;
		Edges[3] = pPrevParticle->Position + EdgeVect;

		// see if we have another particle coming up, if so, we need to orientate our edge with respect to it, not last 
		if ( pNextParticle )
		{
			// recalculate Strip vector
			StripVect = pNextParticle->Position - pCurrParticle->Position;
			//StripVect.Normalize();
			StripVect *= StripVect.DivSizeApprox();
		}
		
		//Vector from Camera to this particle
		FVector LookVect = pCurrParticle->Position - Frame->Coords.Origin;
		//LookVect.Normalize();
		LookVect *= LookVect.DivSizeApprox();	
		
		//Cross product of Look and Strip gives us a vect perpendicular to both, hence maximized with respect to camera
		EdgeVect = LookVect ^ StripVect;
		//EdgeVect.Normalize();
		EdgeVect *= EdgeVect.DivSizeApprox();

		if ( (EdgeVect | PrevEdgeVect) < 0 )
			EdgeVect *= -1.0f;

		//Calculate 2 edge points
		Edges[2] = pCurrParticle->Position;
		Edges[1] = pCurrParticle->Position;
		
		// see if we have another particle coming up, if so, we need to use the nextparticle's width
		Edges[2] += EdgeVect * pCurrParticle->Width;
		Edges[1] -= EdgeVect * pCurrParticle->Width;
		
		// helpful debugging tool
		//Frame->Viewport->RenDev->Draw3DLine( Frame, FColor(255,0,0).Plane(), LINE_DepthCued, pCurrParticle->Position, pCurrParticle->Position + (Edges[2] - pCurrParticle->Position) * 4.0); 

		// project all the poly locations to screen space
		for ( int i=0; i<4; i++ )
		{
			verts[i].Point = Edges[i].TransformPointBy(Frame->Coords);
			verts[i].Project(Frame);
		}

		// fill in the FTransTexture structure ( most of it is already correct )
		SetUV(Texture, verts[0], 0.0f, 0.0f);
		SetUV(Texture, verts[2], 1.0f, 1.0f);
		SetUV(Texture, verts[3], 1.0f, 0.0f);
		SetUV(Texture, verts[1], 0.0f, 1.0f);
		ComputeClipCodes(Frame, verts[0]);
		ComputeClipCodes(Frame, verts[2]);
		ComputeClipCodes(Frame, verts[3]);
		ComputeClipCodes(Frame, verts[1]);

		pPrevParticle = pCurrParticle;
		PrevStripVect = StripVect;
		PrevEdgeVect = EdgeVect;

		return CombineClipCodes( verts, NumVerts );
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


inline FLOAT Interpolate(FLOAT interpolant, FLOAT x0, FLOAT x1)
{
	return (x1 - x0) * interpolant + x0;
}


void ScreenVector
(
	const UParticle& Particle,	// Particle.
	UTexture* Texture,			// Texture.
	FTransTexture* Pts,			// Points.
	FSceneNode* Frame			// Scene for projection.
)
{
	FLOAT   dx, dy;
	FLOAT   interpolant;
	FVector Streak;
	FLOAT NewRadius;
	FLOAT TailLength;

	if ( Particle.DripTimer > 0.0f )
	{
		Streak = Particle.Velocity.SafeNormal() * 0.5f * Particle.Length;
		NewRadius = Particle.Width * 0.5f;
	}
	else
	{
		// Calculate a volume of a Cone given our current Width/Radius
		FLOAT Volume = 3.14159f/3.0f * Particle.Width * Particle.Width * Particle.Width;
		
		// Streak is the excess that is greater than the radius of the sphere due to streaking
		Streak = Particle.Velocity.SafeNormal() * appSqrt(Particle.Velocity.Size()) * Particle.Length ;

		TailLength = Particle.Width + Streak.Size();
		
		// Volume of a Cone
		// V = ( Pi * r^2 H ) / 3
		// r = sqrt( (3V) / (H Pi) )

		// compute new radius of cone given newly calculated Volume and h 
		// Interestingly enough, the GMath.SqrtApprox() was twice as slow as the standard call on this PIII 600
		NewRadius = appSqrt( (3*Volume) / (TailLength*3.1416f));
	}	

	FTransform Tail;
	FTransform Tip;

	Tail.Point = (Particle.Position - Streak).TransformPointBy(Frame->Coords);
	Tail.Project(Frame);

	Tip.Point = Particle.Position.TransformPointBy(Frame->Coords);
	Tip.Project(Frame);

	FVector Displacement = Tail.Point-Tip.Point;
	FLOAT	ScreenDisplacement = Displacement.Size2D();
	
	if (ScreenDisplacement <= NewRadius ) 
	{
		// simple just draw scaled circle
		TailLength = NewRadius;
	}
	else
	{
		// streaked drop
		FLOAT   s2d = ScreenDisplacement * ScreenDisplacement;
		FLOAT   s3d = Displacement.SizeSquared();

		// BUGBUG: Replace with fast square root.
		//RB I did a quick test and the SqrtApprox was 2 times slower on this PIII 600 ?
		interpolant = appSqrt(s2d / s3d);
		TailLength = Interpolate( interpolant, NewRadius, ScreenDisplacement );
	}
	
	// Get the direction vector and 2D screen space magnitude.
	dx = Tail.ScreenX - Tip.ScreenX;
	dy = Tail.ScreenY - Tip.ScreenY;
	
	// Get the magnitude.
	FLOAT s_sqr = dx * dx + dy * dy;
	FLOAT inverse_s = GMath.DivSqrtApprox(s_sqr);
	dx *= inverse_s;
	dy *= inverse_s;
	
	dx *= Tip.RZ;
	dy *= Tip.RZ;

	float x = Tip.ScreenX - dx * NewRadius;
	float y = Tip.ScreenY - dy * NewRadius;
	
	SetTrans(Texture, Pts[1], Frame, x, y, Tip.RZ, 0.0f, 1.0f);


	x = Tip.ScreenX + dx * TailLength;
	y = Tip.ScreenY + dy * TailLength;
	
	SetTrans(Texture, Pts[3], Frame, x, y, Tip.RZ, 1.0f, 0.0f);
	
	Swap(dx, dy);
	dy *= -1.0f;
	
	x = Tip.ScreenX - dx * NewRadius;
	y = Tip.ScreenY - dy * NewRadius;
		
	SetTrans(Texture, Pts[0], Frame, x, y, Tip.RZ, 0.0f, 0.0f);

	
	x = Tip.ScreenX + dx * NewRadius;
	y = Tip.ScreenY + dy * NewRadius;
	
	SetTrans(Texture, Pts[2], Frame, x, y, Tip.RZ, 1.0f, 1.0f);
}

/*------------------------------------------------------------------------------
	The End.
------------------------------------------------------------------------------*/

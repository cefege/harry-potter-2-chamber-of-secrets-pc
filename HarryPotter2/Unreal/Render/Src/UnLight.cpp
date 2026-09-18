/*=============================================================================
	UnLight.cpp: Unreal global lighting subsystem implementation.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

Description:
	Computes all point lighting information and builds surface light meshes 
	based on light actors and shadow maps.

Definitions:
	attenuation:
		The amount by which light diminishes as it travells from a point source
		outward through space.  Physically correct attenuation is propertional to
		1/(distance*distance), but for speed, Unreal uses a lookup table 
		approximation where all light ceases after a light's predefined radius.
	diffuse lighting:
		Viewpoint-invariant lighting on a surface that is the result of a light's
		brightness and a surface texture's diffuse lighting coefficient.
	dynamic light:
		A light that does not move, but has special effects.
	illumination map:
		A 2D array of floating point or MMX Red-Green-Blue-Unused values which 
		represent the illumination that a light applies to a surface. An illumination
		map is the result of combining a light's spatial effects, attenuation,
		incidence factors, and shadow map.
	incidence:
		The angle at which a ray of light hits a point on a surface. Resulting brightness
		is directly proportional to incidence.
    light:
		Any actor whose LightType member has a value other than LT_None.
	meshel:
		A mesh element; a single point in the rectangular NxM mesh containing lighting or
		shadowing values.
	moving light:
		A light that moves. Moving lights do not cast shadows.
	radiosity:
		The process of determining the surface lighting resulting from 
		propagation of light through an environment, accounting for interreflection
		as well as direct light propagation. Radiosity is a computationally
		expensive preprocessing step but generates physically correct lighting.
	raytracing:
		The process of tracing rays through a level between lights and map lattice points
		to precalculate shadow maps, which are later filtered to provide smoothing. 
		Raytracing generates cool looking though physically unrealistic lighting.
	resultant map:
		The final 2D array of floating point or MMX values which represent the total
		illumination resulting from all of the lights (and hence illumination maps) 
		which apply to a surface.
	shadow map:
		A 2D array of floating point values which represent the amount of shadow
		occlusion between a light and a map lattice point, from 0.0f (fully occluded)
		to 1.0f (fully visible).
	shadow hypervolume:
		The six-dimensional hypervolume of space which is not affected by a volume
		lightsource.
	shadow volume:
		The volume of space which is not affected by a point lightsource. The inverse of light
		volume.
	shadow z-buffer:
		A 2D z-buffer representing a perspective projection depth view of the world from a
		lightsource. Often used in dynamic shadowing computations.
	spatial lighting effect:
		A lighting effect that is a function of a location in space, usually relative to
		a light's location.
	specular lighting:
		Viewpoint-varient lighting on a shiny surface that is the result of a
		light's brightness and a surface texture's specular lighting
		coefficient.
	static illumination map:
		An illumination map that represents the total of all static light illumination
		maps that apply to a surface. Static illumination maps do not change in time
		and thus they can be cached.
	static light:
		A light that is constantly on, does not move, and has no special effects.
	surface map:
		Any 2D map that applies to a surface, such as a shadow map or illumination
		map.  Surface maps are always aligned to the surface's U and V texture
		coordinates and are bilinear filtered across the extent of the surface.
	volumetric lighting:
		Lighting that is visible as a result of light interacting with a volume in
		space due to an interacting media such as fog. Volumetric lighting is view
		variant and cannot be associated with a particular surface.

Design notes:
 *	Uses a multi-tiered system for generating the resultant map for a surface,
	where all known constant intermediate and resulting meshes that may be needed 
	in the future are cached, and all known variable intermediate and resulting
	meshes are allocated temporarily.

Notes:
	No radiosity.
	No dynamic shadows.
	No shadow hypervolumes.
	No shadow volumes.
	No shadow z-buffers.

Revision history:
    9-23-96, Tim: Rewritten from the ground up.
=============================================================================*/

#include "Render.h"
#include <math.h>

#define SHADOW_SMOOTHING 1 /* Smooth shadows (should be 1) */
#define ZERO_FLOAT_LIGHT (FLOAT)((3<<22) + 0x10)

//#define MAX_OVERLIGHT	0.1f

/*------------------------------------------------------------------------------------
	Approximate math implementation.
------------------------------------------------------------------------------------*/

FLOAT SqrtManTbl[2<<APPROX_MAN_BITS];
FLOAT DivSqrtManTbl[1<<APPROX_MAN_BITS],DivManTbl[1<<APPROX_MAN_BITS];
FLOAT DivSqrtExpTbl[1<<APPROX_EXP_BITS],DivExpTbl[1<<APPROX_EXP_BITS];

static INT SavedESP,SavedEBP; 

/*------------------------------------------------------------------------------------
	Utility functions
------------------------------------------------------------------------------------*/

inline BYTE ToByte( FLOAT F )
{
	static DWORD Hecker;
	*(FLOAT*)&Hecker = F + (2<<22);
	return BYTE(Hecker);
}

/*------------------------------------------------------------------------------------
	Subsystem definition
------------------------------------------------------------------------------------*/

// Function pointer types.
typedef void (*LIGHT_SPATIAL_FUNC)( FTextureInfo& Tex, class FLightInfo* Info, BYTE* Src, BYTE* Dest );

// Information about one special lighting effect.
struct FLocalEffectEntry
{
	LIGHT_SPATIAL_FUNC	SpatialFxFunc;		// Function to perform spatial lighting
	INT					IsSpatialDynamic;	// Indicates whether light spatiality changes over time.
	INT					IsMergeDynamic;		// Indicates whether merge function changes over time.
};

// Light classification.
enum ELightKind
{
	ALO_StaticLight		= 0,	// Actor is a non-moving, non-changing lightsource
	ALO_DynamicLight	= 1,	// Actor is a non-moving, changing lightsource
	ALO_MovingLight		= 2,	// Actor is a moving, changing lightsource
	ALO_NotLight		= 3,	// Not a surface light (probably volumetric only).
};

// Information about a lightsource.
class FLightInfo
{
public:
	// For all lights.
	AActor*		Actor;					// All actor drawing info.
	ELightKind	Opt;					// Light type.
	FVector		Location;				// Transformed screenspace location of light.
	FVector		LightDir;				// Unit direction from actor to light (max diffuse reflection).
	FVector		MaxSpecDir;				// Surface normal of max specular reflection.
	FLOAT		Radius;					// Maximum effective radius.
	FLOAT		RRadius;				// 1.0f / Radius.
	FLOAT		RadiusInner;			// Radius of maximum intensity.
	FLOAT		RRadiusMult;			// 1.0f / (Radius - RadiusInner);
	FLOAT		Brightness;				// Center brightness at this instance, 1.0f=max, 0.0f=none.
	FLOAT*		LightDistTab;			// Pointer to light distance lookup table.
	FLOAT		Diffuse;				// Multiplier for lookup table.
	BYTE*		IlluminationMap;		// Temporary illumination map pointer.
	BYTE*		ShadowBits;				// Temporary shadow map.
	UBOOL		IsVolumetric;			// Whether it's volumetric.

	// Clipping region.
	INT MinU, MaxU, MinV, MaxV;

	// For volumetric lights.
	FLOAT		VolRadius;				// Volumetric radius.
	FLOAT		VolRadiusSquared;		// VolRadius*VolRadius.
	FLOAT		VolBrightness;			// Volumetric lighting brightness.
	FLOAT		LocationSizeSquared;	// Location.SizeSqurated().
	FLOAT		RVolRadius;				// 1/Volumetric radius.
	FLOAT		RVolRadiusSquared;		// 1/VolRadius*VolRadius.
	UBOOL       VolInside;               // Viewpoint is inside the sphere.

	// Information about the lighting effect.
	FLocalEffectEntry Effect;

	// Coloring.
	FPlane		FloatColor;				// Incident lighting color.
	FPlane		VolumetricColor;		// Volumetric lighting color.
	FPlane		ScaledColor;			// Scaled by actor relationship.
	FColor*		Palette;				// Brightness scaler.
	FColor*     VolPalette;             // Volumetric color scaler.
	
	// Functions.
	void ComputeFromActor( FTextureInfo* Map, FSceneNode* Frame );
};

//
// Lighting manager definition.
//
class FLightManager : public FLightManagerBase
{
public:
	// FLightManagerBase functions.
	void Init();
	void Exit();
	DWORD SetupForActor( FSceneNode* Frame, AActor* Actor, FVolActorLink* LeafLights, FActorLink* Volumetrics );
	void SetupForSurf( FSceneNode* Frame, FCoords& FacetCoords, FBspDrawList* Draw, FTextureInfo*& LightMap, FTextureInfo*& FogMap, UBOOL Merged );
	void FinishSurf();
	void FinishActor();
	void LightAndFog( FTransSample& Point, DWORD PolyFlags );
	void LightParticleSystem( FSceneNode* Frame, AParticleFX* ParticleFX );

	// Constants and types.
	enum {MAX_LIGHTS=256};
	typedef DWORD FILTER_TAB[4];

	// Spatial lighting functions.
	static void spatial_None		( FTextureInfo& Tex, FLightInfo* Info, BYTE* Src, BYTE* Dest );
	static void spatial_SearchLight	( FTextureInfo& Tex, FLightInfo* Info, BYTE* Src, BYTE* Dest );
	static void spatial_SlowWave	( FTextureInfo& Tex, FLightInfo* Info, BYTE* Src, BYTE* Dest );
	static void spatial_FastWave	( FTextureInfo& Tex, FLightInfo* Info, BYTE* Src, BYTE* Dest );
	static void spatial_Shock		( FTextureInfo& Tex, FLightInfo* Info, BYTE* Src, BYTE* Dest );
	static void spatial_Disco		( FTextureInfo& Tex, FLightInfo* Info, BYTE* Src, BYTE* Dest );
	static void spatial_Interference( FTextureInfo& Tex, FLightInfo* Info, BYTE* Src, BYTE* Dest );
	static void spatial_Cylinder	( FTextureInfo& Tex, FLightInfo* Info, BYTE* Src, BYTE* Dest );
	static void spatial_Rotor		( FTextureInfo& Tex, FLightInfo* Info, BYTE* Src, BYTE* Dest );
	static void spatial_Spotlight	( FTextureInfo& Tex, FLightInfo* Info, BYTE* Src, BYTE* Dest );
	static void spatial_NonIncidence( FTextureInfo& Tex, FLightInfo* Info, BYTE* Src, BYTE* Dest );
	static void spatial_Shell       ( FTextureInfo& Tex, FLightInfo* Info, BYTE* Src, BYTE* Dest );
	static void spatial_Test		( FTextureInfo& Tex, FLightInfo* Info, BYTE* Src, BYTE* Dest );

	// FLightManager functions.
	static void Merge( FTextureInfo& Tex, BYTE LightEffect, INT Key, FLightInfo* Light, DWORD* Stream, DWORD* Dest );
	static FLOAT Volumetric( FLightInfo* Info, FVector& Vertex );
	void ShadowMapGen( FTextureInfo& Tex, BYTE* SrcBits, BYTE* Dest1 );
	UBOOL AddLight( AActor* Actor, AActor* Other );

	// Variables.
	static FCoords			*MapCoords, MapUncoords;
	static FVector			VertexBase, VertexDU, VertexDV;
	static FSceneNode*		Frame;
	static ULevel*			Level;
	static FMemMark			Mark;
	static INT				ShadowMaskU, ShadowMaskSpace, ShadowSkip;
	static INT				StaticLights, DynamicLights, MovingLights, StaticLightingChanged;
	static FTextureInfo		LightMap, FogMap;
	static FMipmap			LightMip, FogMip;
	static FLightInfo*		LastLight, **LastVtric;
	static FLightInfo*const FinalLight;
	static FLightInfo		FirstLight[MAX_LIGHTS], *FirstVtric[MAX_LIGHTS];
	static ALevelInfo*		LevelInfo;
	static AZoneInfo*		Zone;
	static FPlane			AmbientVector;
	static FLOAT			Diffuse, Specular, SpecMinCos;
	static INT              TemporaryTablesBuilt;
	static FLOAT			BackdropBrightness;
	static FLOAT            LightDist[4096], LightDistInc[4096];
	static FILTER_TAB		FilterTab[128];
	static BYTE				ByteFog[0x4000];
	static AActor*			Actor;
	static const FLocalEffectEntry Effects[LE_MAX];

	// Memory cache info.
	enum {MAX_UNLOCKED_ITEMS=256};
	static FMemCache::FCacheItem* ItemsToUnlock[MAX_UNLOCKED_ITEMS];
	static FMemCache::FCacheItem** TopItemToUnlock;
};

FCoords*						FLightManager::MapCoords;
FCoords							FLightManager::MapUncoords;
FVector							FLightManager::VertexBase;
FVector							FLightManager::VertexDU;
FVector							FLightManager::VertexDV;
FSceneNode*						FLightManager::Frame;
ULevel*							FLightManager::Level;
FMemMark						FLightManager::Mark;
INT								FLightManager::ShadowMaskU;
INT								FLightManager::ShadowMaskSpace;
INT								FLightManager::ShadowSkip;
INT								FLightManager::StaticLights;
INT								FLightManager::DynamicLights;
INT								FLightManager::MovingLights;
INT								FLightManager::StaticLightingChanged;
FTextureInfo					FLightManager::LightMap;
FTextureInfo					FLightManager::FogMap;
FMipmap							FLightManager::LightMip;
FMipmap							FLightManager::FogMip;
FLightInfo*						FLightManager::LastLight;
FLightInfo**					FLightManager::LastVtric;
FLightInfo*const				FLightManager::FinalLight = &FirstLight[MAX_LIGHTS];
FLightInfo						FLightManager::FirstLight[MAX_LIGHTS];
FLightInfo*						FLightManager::FirstVtric[MAX_LIGHTS];
FLightManager::FILTER_TAB		FLightManager::FilterTab[128];
ALevelInfo*						FLightManager::LevelInfo;
AZoneInfo*						FLightManager::Zone;
FPlane							FLightManager::AmbientVector;
FLOAT							FLightManager::Diffuse;
FLOAT							FLightManager::Specular;
FLOAT							FLightManager::SpecMinCos;
BYTE							FLightManager::ByteFog[0x4000];
AActor*							FLightManager::Actor;
INT								FLightManager::TemporaryTablesBuilt;
FLOAT							FLightManager::BackdropBrightness;
FLOAT							FLightManager::LightDist[4096];
FLOAT							FLightManager::LightDistInc[4096];
FMemCache::FCacheItem*						FLightManager::ItemsToUnlock[MAX_UNLOCKED_ITEMS];
FMemCache::FCacheItem**					FLightManager::TopItemToUnlock;

const FLocalEffectEntry FLightManager::Effects[LE_MAX] =
{
// LE_ tag			Spatial func			SpacDyn MergeDyn
// ----------------	-----------------------	------- --------
{/* None         */	spatial_None,			0,		0        },
{/* TorchWaver   */	spatial_None,			0,		1        },
{/* FireWaver    */	spatial_None,			0,		1        },
{/* WateryShimmer*/	spatial_None,			0,		1        },
{/* Searchlight  */	spatial_SearchLight,	1,		0        },
{/* SlowWave     */	spatial_SlowWave,		1,		0        },
{/* FastWave     */	spatial_FastWave,		1,		0        },
{/* CloudCast    */	spatial_None,			1,		0        },		// Unimplemented.
{/* StaticSpot   */	spatial_Spotlight,		0,		0        },
{/* Shock        */	spatial_Shock,			1,		0        },
{/* Disco        */	spatial_Disco,			1,		0        },
{/* Warp         */	spatial_None,			0,		0        },
{/* Spotlight    */	spatial_Spotlight,		0,		0        },
{/* NonIncidence */	spatial_NonIncidence,	0,		0        },
{/* Shell        */	spatial_Shell,			0,		0        },
{/* Satellite    */	spatial_None,			0,		0        },
{/* Interference */	spatial_Interference,	1,		0        },
{/* Cylinder     */	spatial_Cylinder,		0,		0        },
{/* Rotor        */	spatial_Rotor,			1,		0        },
{/* Unused		 */	spatial_None,			0,		0        },
};

/*------------------------------------------------------------------------------------
	Init & Exit.
------------------------------------------------------------------------------------*/

//
// Set up the tables required for fast square root computation.
//
static void SetupTable( FLOAT* ManTbl, FLOAT* ExpTbl, FLOAT Power )
{
	union {FLOAT F; DWORD D;} Temp;

	Temp.F = 1.0f;
	DWORD i;
	for( i=0; i<(1<<APPROX_EXP_BITS); i++ )
	{
		Temp.D = (Temp.D & 0x007fffff ) + (i << (32-APPROX_EXP_BITS));
		ExpTbl[ i ] = appPow( Abs(Temp.F), Power );
		if( appIsNan(ExpTbl[ i ]) )
			ExpTbl[ i ]=0.0f;
		//debugf("exp [%f] %i = %f",Power,i,ExpTbl[i]);
	}

	Temp.F = 1.0f;
	for( i=0; i<(1<<APPROX_MAN_BITS); i++ )
	{
		Temp.D = (Temp.D & 0xff800000 ) + (i << (32-APPROX_EXP_BITS-APPROX_MAN_BITS));
		ManTbl[ i ] = appPow( Abs(Temp.F), Power );
		if( appIsNan(ManTbl[ i ]) )
			ManTbl[ i ]=0.0f;
		//debugf("man [%f] %i = %f",i,Power,ManTbl[i]);
	}
}

//
// Initialize the global lighting subsystem.
//
void FLightManager::Init()
{
	guard(FLightManager::Init);
	appMemset( &LightMap, 0, sizeof(LightMap) );
	appMemset( &FogMap,   0, sizeof(FogMap  ) );

	// Mutual occlusion blending.
	INT i;
	for( i=0; i<128; i++ )
		for( INT j=0; j<128; j++ )
			ByteFog[i*128+j] = (127-i)*j/127;

	// Filtering table.
	INT FilterWeight[8][8] = 
	{
		{ 0,24,40,24,0,0,0,0},
		{ 0,40,64,40,0,0,0,0},
		{ 0,24,40,24,0,0,0,0},
		{ 0, 0, 0, 0,0,0,0,0},
		{ 0, 0, 0, 0,0,0,0,0},
		{ 0, 0, 0, 0,0,0,0,0},
		{ 0, 0, 0, 0,0,0,0,0},
		{ 0, 0, 0, 0,0,0,0,0}
	};

	// Setup square root tables.
	for( DWORD D=0; D< (1<< APPROX_MAN_BITS ); D++ )
	{
		union {FLOAT F; DWORD D;} Temp;
		Temp.F = 1.0f;
		Temp.D = (Temp.D & 0xff800000 ) + (D << (23 - APPROX_MAN_BITS));
		Temp.F = appSqrt(Temp.F);
		Temp.D = (Temp.D - ( 64 << 23 ) );   // exponent bias re-adjust
		SqrtManTbl[ D ] = (FLOAT)(Temp.F * appSqrt(2.0f)); // for odd exponents
		SqrtManTbl[ D + (1 << APPROX_MAN_BITS) ] =  (FLOAT) (Temp.F * 2.0f);
	}
	SetupTable(DivSqrtManTbl,DivSqrtExpTbl,-0.5f);
	SetupTable(DivManTbl,    DivExpTbl,    -1.0f);
	
	// Init square roots.
	for( i=0; i<ARRAY_COUNT(LightDist); i++ )
	{
		FLOAT S = appSqrt((FLOAT)(i+1) * (1.0f/ARRAY_COUNT(LightDist)));

		// This function gives a more luminous, specular look to the lighting.
		FLOAT Temp = (2*S*S*S-3*S*S+1); // Or 1.0f-S.

		// This function makes surfaces look more matte.
		//FLOAT Temp = (1.0f-S);
		LightDist[i] = Temp;
		LightDistInc[i] = Temp/S;
	}

	// Generate filter lookup table
	INT FilterSum=0;
	for( i=0; i<8; i++ )
		for( int j=0; j<8; j++ )
			FilterSum += FilterWeight[i][j];

	// Iterate through all filter table indices 0x00-0x3f.
	for( i=0; i<128; i++ )
	{
		// Iterate through all vertical filter weights 0-3.
		for( int j=0; j<4; j++ )
		{
			// Handle all four packed values.
			FilterTab[i][j] = 0;
			for( INT Pack=0; Pack<4; Pack++ )
			{
				// Accumulate filter weights in FilterTab[i][j] according to which bits are set in i.
				INT Acc = 0;
				for( INT Bit=0; Bit<8; Bit++ )
					if( i & (1<<(Pack + Bit)) )
						Acc += FilterWeight[j][Bit];

				// Add to sum.
				DWORD Result = (Acc * 255) / FilterSum;
				check(Result>=0 && Result<=255);
				FilterTab[i][j] += (Result << (Pack*8));
			}
		}
	}

	// Cache items.
	TopItemToUnlock = &ItemsToUnlock[0];

	// Success.
	debugf( NAME_Init, TEXT("Lighting subsystem initialized") );
	unguard;
}

//
// Shut down the global lighting system.
//
void FLightManager::Exit()
{
	guard(FLightManager::Exit);

	debugf( NAME_Exit, TEXT("Lighting subsystem shut down") );
	unguard;
}

/*------------------------------------------------------------------------------------
	Intermediate map generation code
------------------------------------------------------------------------------------*/

//
// Generate the shadow map for one lightsource that applies to a Bsp surface.
//
void FLightManager::ShadowMapGen( FTextureInfo& Tex, BYTE* SrcBits, BYTE* Dest1 )
{
	guardSlow(FLightManager::ShadowMapGen);
	checkSlow((reinterpret_cast<UPTRINT>(Dest1) & UPTRINT(3)) == 0);

	// If no source, fill it.
	if( !SrcBits )
	{
		appMemset( Dest1, 127, ShadowMaskSpace*8 );
		return;
	}

	// Generate smooth shadow map by convolving the shadow bitmask with a smoothing filter.
	INT Size4 = (ShadowMaskU*8)/4;
	appMemzero( Dest1, ShadowMaskSpace*8 );
	DWORD* Dests[3] = { (DWORD*)Dest1, (DWORD*)Dest1, (DWORD*)Dest1 + Size4 };
	for( INT V=0; V<Tex.VClamp; V++ )
	{
		// Get initial bits, with low bit shifted in.
		BYTE* Src = SrcBits;

		// Offset of shadow map relative to convolution filter left edge.
		DWORD D = (DWORD)*Src++ << (8+2);
		if( D & 0x400 ) D |= 0x300;

		// Filter everything.
		for( INT U=0; U<ShadowMaskU; U++ )
		{
			D = D >> 8;
			D += (U<ShadowMaskU-1) ? (((DWORD)*Src++) << (8+2)) : (D&0x200) ? 0xC00 : 0;

			FILTER_TAB& Tab1 = FilterTab[D & 0x7f];
			*Dests[0]++     += Tab1[0];
			*Dests[1]++     += Tab1[1];
			*Dests[2]++     += Tab1[2];

			FILTER_TAB& Tab2 = FilterTab[(D>>4) & 0x7f];
			*Dests[0]++     += Tab2[0];
			*Dests[1]++     += Tab2[1];
			*Dests[2]++     += Tab2[2];
		}
		SrcBits += ShadowMaskU;
		if( V == 0            ) Dests[0] -= Size4;
		if( V == Tex.VClamp-2 ) Dests[2] -= Size4;
	}
	unguardSlow;
}

/*------------------------------------------------------------------------------------
	Vertex lighting and fogging.
------------------------------------------------------------------------------------*/

void FLightManager::LightAndFog( FTransSample& Vert, DWORD PolyFlags )
{
	guard(FLightManager::LightAndFog);
	Clock(GStat.MeshLightTime);

	Vert.Light = AmbientVector;
	Vert.Fog = FPlane(0);

	if( PolyFlags & PF_Unlit )
		return;

	// Lit.
	STAT(GStat.MeshVertLightCount += LastLight-FirstLight);
#if !__PSX2_EE__
	FLOAT PointSquared(Vert.Point.SizeSquared());
#endif
	for( FLightInfo* Light=FirstLight; Light<LastLight; Light++ )
	{
		if( Light->Opt != ALO_NotLight )
		{
			// Diffuse lighting.
			FVector LightVector  = Light->Location - Vert.Point;
			FLOAT   LightSquared = LightVector.SizeSquared();
			FLOAT   LightSize    = SqrtApprox( LightSquared );

			// Compute incidence, based on light directionality.
			FLOAT	G;
			if( Light->Actor->LightSource == LD_Ambient )
			{
				// Constant lighting in all directions.
				G = 1.0f;
				LightVector = Vert.Normal * LightSize;
			}
			else if( Light->Actor->LightSource == LD_Plane )
			{
				// Planar lighting.
				G = Light->LightDir | Vert.Normal;
				LightVector = Light->LightDir * LightSize;
			}
			else
				// Radial lighting.
				G = (LightVector | Vert.Normal) / LightSize;

			if( Diffuse == 1.4f )		// Debug hack old lighting.
				G = Square(1.0f + G) - 1.5f;
			G *= Diffuse;
			if( G < 0.0f)
				G = 0.0f;

#if !__PSX2_EE__
			// Specular lighting.
			if( Specular > 0.f )
			{
				FLOAT S = LightVector.MirrorByVector(Vert.Normal) | Vert.Point;
				if( S > 0.0f )
				{
					S *= S/(LightSquared*PointSquared);
					S -= SpecMinCos;
					if( S > 0.f )
 						G += Specular * S;
				}
			}
#endif

			// Update result color.
			if( G > 0.0f )
			{
				// Radial falloff.
				G *= Min(1.f, (Light->Radius - LightSize) * Light->RRadiusMult);
				Vert.Light += Light->FloatColor * G;
				if( Light->Actor->bDarkLight )
				{
					// Account for any negative results.
					Vert.Light.X = Max( Vert.Light.X, 0.0f );
					Vert.Light.Y = Max( Vert.Light.Y, 0.0f );
					Vert.Light.Z = Max( Vert.Light.Z, 0.0f );
				}
			}
		}
	}

	Vert.Light.X = MinPositiveFloat( Vert.Light.X, 1.f );
	Vert.Light.Y = MinPositiveFloat( Vert.Light.Y, 1.f );
	Vert.Light.Z = MinPositiveFloat( Vert.Light.Z, 1.f );

	/*
	// Convert shade to highlight in case of overlighting.
	FLOAT S = MaxPositiveFloat( MaxPositiveFloat(Vert.Light.X, Vert.Light.Y), Vert.Light.Z );
	if( S > 1.f )
	{
		Vert.Light /= S;
		Vert.Fog = Vert.Light * MinPositiveFloat( S-1.f, MAX_OVERLIGHT );
	}
	*/

#if 0
	// Editor highlighting.
	if( (PolyFlags & PF_Selected) && GIsEditor )
		Vert.Light = Vert.Light*0.5f + FVector(0.5f,0.5f,0.5f);
#endif

	// Fog if needed.
	if( PolyFlags & PF_RenderFog )
	{
		// Volumetric fog.
		for( FLightInfo** Ptr=FirstVtric; Ptr<LastVtric; Ptr++ )
		{
			FLightInfo* LightInfo = *Ptr;
			FLOAT VolumeValue = 2.0f * Volumetric( LightInfo, Vert.Point);
			if( *(DWORD*)&VolumeValue )
			{
				FLOAT A = MinPositiveFloat( LightInfo->VolumetricColor.W * VolumeValue, 1.0f );
				Vert.Fog.X   = MinPositiveFloat( LightInfo->VolumetricColor.X * VolumeValue + Vert.Fog.X*(1-A), 1.0f );
				Vert.Fog.Y   = MinPositiveFloat( LightInfo->VolumetricColor.Y * VolumeValue + Vert.Fog.Y*(1-A), 1.0f );
				Vert.Fog.Z   = MinPositiveFloat( LightInfo->VolumetricColor.Z * VolumeValue + Vert.Fog.Z*(1-A), 1.0f );
				Vert.Fog.W   = MinPositiveFloat( A + Vert.Fog.W, 1.0f );
			}
		}		
	}
	unguard;
}

#pragma warning (default:4799)

/*------------------------------------------------------------------------------------
	Light merging.
------------------------------------------------------------------------------------*/

#if _MSC_VER && defined(_M_IX86)
void FLightManager::Merge( FTextureInfo& Tex, BYTE Effect, INT Key, FLightInfo* Info, DWORD* Stream, DWORD* Dest )
{
	guardSlow(FLightManager::Merge);

	static INT Count; 
	FColor* Palette;
    INT Skip;

	// Merge the two streams of light.
	BYTE* Src = Info->IlluminationMap;
	Palette   = Info->Palette;
	Skip      = Info->MinU;
	Count     = Info->MaxU - Info->MinU;

	if( Count<=0 ) return;

	UBOOL FXDetect = ( (Effect==LE_TorchWaver) || (Effect==LE_FireWaver) || (Effect==LE_WateryShimmer) );

	Src    += Info->MinV * Tex.UClamp;
	Dest   += Info->MinV * Tex.USize;
	Stream += Info->MinV * Tex.USize;

	for( INT i=Info->MinV; i<Info->MaxV; i++ )
	{
		BYTE* NewSrc = Src;

		// Execute merge-time effects.
		if( FXDetect )
		{
			BYTE Temp[1024];
			NewSrc = Temp;
			if( Effect==LE_TorchWaver )
			{
				for( INT i=Info->MinU; i<Info->MaxU; i++ )
					Temp[i] = appFloor((FLOAT)Src[i] * (0.95f + 0.05f * GRandoms->RandomBase(Key++)));
			}
			else if( Effect==LE_FireWaver )
			{
				for( INT i=Info->MinU; i<Info->MaxU; i++ )
					Temp[i] = appFloor((FLOAT)Src[i] * (0.80f + 0.20f * GRandoms->RandomBase(Key++)));
			}
			else if( Effect==LE_WateryShimmer )
			{
				for( INT i=Info->MinU; i<Info->MaxU; i++ )
					Temp[i] = appFloor((FLOAT)Src[i] * (0.60f + 0.40f * GRandoms->Random(Key++)));
			}
		}

		// Scale and merge the lighting.
		if( Info->Actor->bDarkLight )
		{
			// Subtract the color. Only non-ASM version for now.
			for( INT j=Info->MinU; j<Info->MaxU; j++ )
			{
				Dest[j] = Stream[j] - GET_COLOR_DWORD(Palette[NewSrc[j]]);
				DWORD SatMask = Dest[j] & 0x80808080;
				if( SatMask )
				{
					// Handle saturation by clamping low.
					SatMask += SatMask - (SatMask>>7);
					Dest[j] &= ~SatMask;
				}
			}
		}
		else
	#if ASM && defined(_M_IX86)
		__asm
		{
			// esi = Stream
			// edi = Dest
			// eax = Temp
			// ebx = Palette
			// ecx = Loop counter
			// edx = Temp
			// ebp = Light element
			// esp = NewSrc

			// Setup.
			mov		[SavedESP], esp
			mov		[SavedEBP], ebp
			mov		edx, [Skip]
			mov		esi, [Stream]
			mov		edi, [Dest]
			mov		ebx, [Palette]
			mov		esp, [NewSrc]
			xor		ecx, ecx
			mov     ebp, [Count]
			lea     esi, [esi+edx*4]
			lea     edi, [edi+edx*4]
			lea     esp, [esp+edx]

			// lead-in
			xor		edx, edx
			dec     ebp   // Compensate for leadin/out plus test for 1.
			mov     dl, [esp+ecx]
			jz      LeadOut 
			
			align 16
			// Get scaled light element - 6 cycles
			LightLoop:
			mov		eax, [esi+ecx*4] // Get stream
			mov		edx, [ebx+edx*4] // Get color from palette
			add		eax, edx         // Add stream and color		
			xor     edx, edx
			test	eax, 0x80808080  // Check for saturation
			jnz		LightSaturate    // Fix up after saturation
			mov		dl,[esp+ecx+1]
			mov		[edi+ecx*4], eax // Store result
			inc		ecx
			cmp		ecx, ebp
			jb		LightLoop

			// lead-out
			LeadOut:
			mov		eax, [esi+ecx*4] // Get stream
			mov		edx, [ebx+edx*4] // Get color from palette
			add		eax, edx         // Add stream and color		
			test	eax, 0x80808080  // Check for saturation
			jnz		LastLightSaturate    // Fix up after saturation
			mov		[edi+ecx*4], eax     // Store result
			jmp		LightOut

			align 16
			// Handle saturation - about 9 cycles
			LightSaturate:
			mov		edx,eax
			and		edx,0x80808080
			mov		ebp,edx
			shr		edx,7
			and		eax,0x7F7F7F7F	// mask out all overflowed bits
			sub		ebp,edx			// Creates 7f in each overflowing channel
			xor     edx,edx
			or		eax,ebp			// Set saturated channels
			mov     ebp,[Count]     // reload end indicator
			mov		[edi+ecx*4],eax // Store result
			mov		dl,[esp+ecx+1]
			dec     ebp             // compensate for leadin/out
			inc		ecx
			cmp		ecx, ebp
			jb		LightLoop
			jmp		LeadOut


			align 16
			LastLightSaturate:
			mov		edx,eax
			and		edx,0x80808080
			mov		ebp,edx
			shr     edx,7
			and     eax,0x7f7f7f7f
			sub     ebp,edx
			or      eax,ebp
			mov     [edi+ecx*4],eax

			///
			align 16
			LightOut:
			mov ebp, [SavedEBP]
			mov esp, [SavedESP]
		}		

	#else

		for( INT j=Info->MinU; j<Info->MaxU; j++ )
		{
			Dest[j] = Stream[j] + GET_COLOR_DWORD(Palette[NewSrc[j]]);
			if( Dest[j] & 0x80808080 )
			{
				// Handle saturation.
				DWORD SatMask = Dest[j] & 0x80808080;
				SatMask -= (SatMask >>7);
				Dest[j] = (Dest[j] & 0x7f7f7f7f) | SatMask;
			}
		}

	#endif

		Src    += Tex.UClamp;
		Stream += Tex.USize;
		Dest   += Tex.USize;
	}

	unguardSlow;
}

#else

void FLightManager::Merge( FTextureInfo& Tex, BYTE Effect, INT Key, FLightInfo* Info, DWORD* StreamParam, DWORD* DestParam )
{
	guardSlow(FLightManager::Merge);

#if ASMLINUX && defined(__i386__)
	static INT Inputs[6];
	FColor*& Palette = (FColor*&)Inputs[0];
	INT& Skip = Inputs[1];
	INT& Count = Inputs[2];
	DWORD*& Stream = (DWORD*&)Inputs[3];
	DWORD*& Dest = (DWORD*&)Inputs[4];
	BYTE*& NewSrc = (BYTE*&)Inputs[5];
#else
	FColor* Palette;
	INT Skip;
	INT Count;
	DWORD* Stream;
	DWORD* Dest;
	BYTE* NewSrc;
#endif

	Stream = StreamParam;
	Dest = DestParam;

	// Merge the two streams of light.
	BYTE* Src = Info->IlluminationMap;
	Palette   = Info->Palette;
	Skip      = Info->MinU;
	Count     = Info->MaxU - Info->MinU;

	if( Count<=0 ) return;

	UBOOL FXDetect = ( (Effect==LE_TorchWaver) || (Effect==LE_FireWaver) || (Effect==LE_WateryShimmer) );

	Src    += Info->MinV * Tex.UClamp;
	Dest   += Info->MinV * Tex.USize;
	Stream += Info->MinV * Tex.USize;

	for( INT i=Info->MinV; i<Info->MaxV; i++ )
	{
		NewSrc = Src;

		// Execute merge-time effects.
		if( FXDetect )
		{
			BYTE Temp[1024];
			NewSrc = Temp;
			if( Effect==LE_TorchWaver )
			{
				for( INT i=Info->MinU; i<Info->MaxU; i++ )
					Temp[i] = appFloor((FLOAT)Src[i] * (0.95f + 0.05f * GRandoms->RandomBase(Key++)));
			}
			else if( Effect==LE_FireWaver )
			{
				for( INT i=Info->MinU; i<Info->MaxU; i++ )
					Temp[i] = appFloor((FLOAT)Src[i] * (0.80f + 0.20f * GRandoms->RandomBase(Key++)));
			}
			else if( Effect==LE_WateryShimmer )
			{
				for( INT i=Info->MinU; i<Info->MaxU; i++ )
					Temp[i] = appFloor((FLOAT)Src[i] * (0.60f + 0.40f * GRandoms->Random(Key++)));
			}
		}

		// Scale and merge the lighting.
    #if ASMLINUX && defined(__i386__)
		// Dummy output variables.
		int D0, D1, D2, D3, D4;

		asm volatile ("
			# Registers:
			# %0    = Byte-accessible temp.
			# %1    = Temp.
			# %2    = Loop counter.
			# %3    = Stream.
			# %4    = Dest.
			# %%esp = NewSrc.
			# %%ebp = Count / Light element.
			#
			# Memory:
			# %5    = SavedESP.
			# %6    = SavedEBP.
			# %7    = Inputs[0]: Palette.
			# 4+%7  = Inputs[1]: Skip.
			# 8+%7  = Inputs[2]: Count.
			# 12+%7 = Inputs[3]: Stream.
			# 16+%7 = Inputs[4]: Dest.
			# 20+%7 = Inputs[5]: NewSrc.

			# Setup.
			movl	%%esp, %5;         # Save esp.
			movl	%%ebp, %6;         # Save ebp.
			movl	4+%7, %0;          # Load Skip.
			movl	8+%7, %%ebp;       # Load Count.
			movl	12+%7, %3;         # Load Stream.
			movl	16+%7, %4;         # Load Dest.
			movl	20+%7, %%esp;      # Load NewSrc.
			xorl	%2, %2;            # Clear loop counter.
			leal	(%3,%0,4), %3;     # Stream += Skip.
			leal	(%4,%0,4), %4;     # Dest += Skip.
			leal	(%%esp,%0), %%esp; # NewSrc += Skip.

			# Lead-in.
			xorl	%0, %0;
			decl	%%ebp; # Compensate for leadin/out plus test for 1.
			movb	(%%esp,%2), %b0;
			jz		LeadOut;

			.align 16;
			# Get scaled light element.
			LightLoop:
			movl	%7, %1;          # Get palette.
			movl	(%1,%0,4), %0;   # Get color from palette.
			movl	(%3,%2,4), %1;   # Get stream.
			addl	%0, %1;          # Add stream and color.
			xorl	%0, %0;
			testl	$0x80808080, %1; # Check for saturation.
			jnz		LightSaturate;   # Fix up after saturation.
			movb	(%%esp,%2), %b0;
			movl	%1, (%4,%2,4);   # Store result.
			incl	%2;
			cmpl	%%ebp, %2;
			jb		LightLoop;

			# Lead-out.
			LeadOut:
			movl	%7, %1;            # Get palette.
			movl	(%1,%0,4), %0;     # Get color from palette.
			movl	(%3,%2,4), %1;     # Get stream.
			addl	%0, %1;            # Add stream and color.
			testl	$0x80808080, %1;   # Check for saturation.
			jnz		LastLightSaturate; # Fix up after saturation.
			movl	%1, (%4,%2,4);     # Store result.
			jmp		LightOut;

			.align 16;
			# Handle saturation.
			LightSaturate:
			movl	%1, %0;
			andl	$0x80808080, %0;
			movl	%0, %%ebp;
			shrl	$7, %0;
			andl	$0x7F7F7F7F, %1;  # Mask out all overflowed bits.
			subl	%0, %%ebp;        # Creates 7F in each overflowing channel.
			xorl	%0, %0;
			orl		%%ebp, %1;        # Set saturated channels.
			movl	8+%7, %%ebp;      # Reload end indicator.
			movl	%1, (%4,%2,4);    # Store result.
			movb	1(%%esp,%2), %b0;
			decl	%%ebp;            # Compensate for leadin/out.
			incl	%2;
			cmpl	%%ebp, %2;
			jb		LightLoop;
			jmp		LeadOut;

			.align 16;
			LastLightSaturate:
			movl	%1, %0;
			andl	$0x80808080, %0;
			movl	%0, %%ebp;
			shrl	$7, %0;
			andl	$0x7F7F7F7F, %1;
			subl	%0, %%ebp;
			orl		%%ebp, %1;
			movl	%1, (%4,%2,4);

			.align 16;
			LightOut:
			movl	%6, %%ebp;
			movl	%5, %%esp;
        "
        : "=c" (D0),
          "=S" (D1),
          "=D" (D2),
          "=d" (D3),
          "=a" (D4),
          "=m" (SavedESP),
          "=m" (SavedEBP)
        : "m" (Inputs[0])
		: "memory"
		);
	#else

		for( INT j=Info->MinU; j<Info->MaxU; j++ )
		{
			Dest[j] = Stream[j] + GET_COLOR_DWORD(Palette[NewSrc[j]]);
			if( Dest[j] & 0x80808080 )
			{
				// Handle saturation.
				DWORD SatMask = Dest[j] & 0x80808080;
				SatMask -= (SatMask >>7);
				Dest[j] = (Dest[j] & 0x7f7f7f7f) | SatMask;
			}
		}

	#endif

		Src    += Tex.UClamp;
		Stream += Tex.USize;
		Dest   += Tex.USize;
	}

	unguardSlow;
}
#endif

/*------------------------------------------------------------------------------------
	Spatial effect functions.
------------------------------------------------------------------------------------*/

inline FLOAT FLightManager::Volumetric( FLightInfo* Info, FVector &Vertex )
{
	if( Info->Actor->LightSource == LD_Ambient )
	{
		return MinPositiveFloat( Vertex.SizeApprox() * Info->RVolRadius * Info->VolBrightness, 1.f );
	}

	// Optimize: there's 2 sqrtapproxes and 2 divides -   too many?
	//
    // d  = (square of) distance of shortest line from viewer-to-surface-line to light location.
	// c1 = distance along viewer-to-surface line to surface, relative to nearest point to light location.
	// c2 = distance along viewer-to-surface line to viewer, relative to nearest point to light location.
	// F  = fog line-integral.
	//
	// FLOAT VertexSize = SqrtApprox(Vertex.SizeSquared());	// Distance eye-to-vertex.
	//
	static int FogRejectionMethod = 0;

	FLOAT c1, c2, d, F, h, c0, S, S2; 

	S  = ( Info->Location | Vertex ); // 3 fmuls 2 fadds 
	
	if (! Info->VolInside )
	{
		// No fog if negative dotproduct Light*Vertex AND we're not inside the sphere.
		if (IsNegativeFloat(S)) return 0.0f;
	}

	// Determine distance to the vertex.
	FLOAT VertexSizeSquared = Vertex.SizeSquared(); // 3 fmuls
	S2  = S*S; 

	// Whenever the stricter 'd'-check rejected a vertex, we'll try to revert to this one next time.
	// 2 fmuls.
	if ( FogRejectionMethod )
		// equivalent to    if( Info->VolRadiusSquared < d )
		if ( (VertexSizeSquared * Info->VolRadiusSquared) < (VertexSizeSquared * Info->LocationSizeSquared - S2) )	
		{
			return 0.0f;
		}

	// d>=0, sqrt(fabs(d))= Distance of shortest line from viewer-to-surface-line to light location.
	d  = Info->LocationSizeSquared - S2/VertexSizeSquared;   

	// Viewer-to-surface line does not intersect light's sphere.
	if( Info->VolRadiusSquared < d )	// Out distance squared < d 
	{
		FogRejectionMethod = 1; // primed to do cheaper radius checks.
		return 0.0f;
	}

	FogRejectionMethod = 0; //radius checks failed, probably next time also.

	// ERIK+: Maybe it's worth having a lookup table which simultaneously
	// returns sqrt(x) and 1.0/sqrt(x), to avoid the S/VertexSize divide. -Tim

	FLOAT VertexSize = SqrtApprox(VertexSizeSquared);	
	c0 = S/VertexSize;		// c2 <= Info.Location.Size().

	// Compute c1 and c2 from line clipped to interior of sphere.
	h  = SqrtApprox( Info->VolRadiusSquared - d );

	int FullSphere = 0;

	c1 = c0 - VertexSize;   

	if (c0 > h)
	{ 
		c2 = h;
		if (c1 < -h)
		{
			c1 = -h;        // special-case: c1==-h, c2==h, only one integral needed.
			FullSphere = 1;
		}
	}
	else
	{
		c2 = c0;
		if (c1 < -h)
		{
			c1 = -h;
		}	
	}
	
	if (c1 >= c2) return 0.0f; // point totally outside sphere

	FLOAT D2 = d  * Info->RVolRadiusSquared; // real distance from center -> normalized.
	c2 = c2 * Info->RVolRadius;	// scaled relative to volume radius.

	if (FullSphere) // c1== -c2
	{
		FLOAT I = (3-3*D2);
		F =  2.0f * Info->VolBrightness * ( c2*(I - c2*c2) );

		// F = 2.0f * Bri * ( c2 * Rad  ( (3-3*d*rad*rad)  - c2 c2 rad rad ) );
	}
	else
	{
		c1 = c1  * Info->RVolRadius;  
		FLOAT I = (3-3*D2);
		F =  Info->VolBrightness *( (c2*(I - c2*c2 ) ) - ( c1*(I - c1*c1 ) ) );
	}

	if (F < 0.0f) return 0.0f; //#debug superfluous check, with the new lighting code ?!!
	return MinPositiveFloat( F, 1.0f );       	
}

//
// Template functions for implementing all light effects.
//

template<class E>
void SpatialGeneral( E*, FTextureInfo& Map, FLightInfo* Info, BYTE* Src, BYTE* Dest )
// Can implement any effect, controlled by class E.
{
	E Effect;
	Effect.Init( Info );

	// Compute values for stepping through mesh points.
	FVector VertexV = FLightManager::VertexBase 
		+ FLightManager::VertexDV*Info->MinV + FLightManager::VertexDU*Info->MinU - 
		Info->Actor->Location;
	Src  += (FLightManager::ShadowMaskU*8)*Info->MinV + Info->MinU;
	Dest += Map.UClamp*Info->MinV + Info->MinU;
	INT USkip = Map.UClamp - (Info->MaxU - Info->MinU);

	FLOAT RadiusMult = 4093.0f / Square(Info->Radius);

	// Step through the pixels, incrementing the vertex.
	for( INT VCounter=Info->MinV; VCounter<Info->MaxV; VCounter++, Src+=USkip+FLightManager::ShadowSkip, Dest+=USkip, VertexV += FLightManager::VertexDV ) 
	{
		FVector Vertex = VertexV;
		for( INT UCounter=Info->MinU; UCounter<Info->MaxU; UCounter++, Vertex+=FLightManager::VertexDU, Src++, Dest++ )
		{
			// Perform per-effect visibility.
			if( Effect.Visible( Vertex) )
			{
				// Get the light-radius table index.
				INT DistSq = appRound( Vertex.SizeSquared() * RadiusMult );
				if( DistSq < 4096 && *Src )
					*Dest = ToByte( *Src * Effect.Spatial(DistSq, Vertex) * Effect.Scale(Vertex) );
				else
					*Dest = 0;
			}
			else 
				*Dest = 0;
		 }
	}
}

template<class E>
void SpatialRadial( E*, FTextureInfo& Map, FLightInfo* Info, BYTE* Src, BYTE* Dest )
// Optimized version for radial lights.
{
	guardSlow(FLightManager::SpatialRadial);

	// Variables.
	static FVector Vertex;
	static FLOAT   Scale, Diffuse;
	static INT     Dist, DistU, DistV, DistUU, DistVV, DistUV;
	static INT     Interp00, Interp10, Interp20, Interp01, Interp11, Interp02;
	static DWORD   Inner0, Inner1;

	E Effect;
	Effect.Init( Info );

	// Compute values for stepping through mesh points.
	Vertex   = FLightManager::VertexBase - Info->Actor->Location 
			 + FLightManager::VertexDU*Info->MinU + FLightManager::VertexDV*Info->MinV;
	Scale    = Square(Info->RRadius * 4095);
	Dist     = appRound((Vertex                  | Vertex  ) * Scale);
	DistU    = appRound((Vertex                  | FLightManager::VertexDU) * Scale);
	DistV    = appRound((Vertex                  | FLightManager::VertexDV) * Scale);
	DistUU   = appRound((FLightManager::VertexDU | FLightManager::VertexDU) * Scale);
	DistVV   = appRound((FLightManager::VertexDV | FLightManager::VertexDV) * Scale);
	DistUV   = appRound((FLightManager::VertexDU | FLightManager::VertexDV) * Scale);

	Interp00 = Dist;
	Interp10 = 2 * DistV + DistVV;
	Interp20 = 2 * DistVV;
	Interp01 = 2 * DistU + DistUU;
	Interp11 = 2 * DistUV;
	Interp02 = 2 * DistUU;

	Src  += (FLightManager::ShadowMaskU*8) * Info->MinV + Info->MinU;
	Dest += Map.UClamp * Info->MinV + Info->MinU;
	INT USkip = Map.UClamp - (Info->MaxU-Info->MinU);

	for( INT VCounter=Info->MinV; VCounter<Info->MaxV; VCounter++ )
	{
		// Forward difference the square of the distance between the points.
		Inner0 = Interp00;
		Inner1 = Interp01;
		for( INT U=Info->MinU; U<Info->MaxU; U++ )
		{
			if( Inner0<4096*4096 && *Src!=0 ) 
				*Dest = ToByte( *Src * Effect.Spatial(Inner0>>12, Vertex) );
			else 
				*Dest = 0;
			Src++;
			Dest++;
			Inner0 += Inner1;
			Inner1 += Interp02;
		}
		Interp00 += Interp10;
		Interp10 += Interp20;
		Interp01 += Interp11;
		Src  += USkip+FLightManager::ShadowSkip;
		Dest += USkip;
	}
	unguardSlow;
}


#define MAKE_SPATIAL_FUNC(E) \
	void FLightManager::spatial_##E( FTextureInfo& Map, FLightInfo* Info, BYTE* Src, BYTE* Dest ) \
		{ SpatialGeneral( (Effect_##E*)0, Map, Info, Src, Dest ); }

//
// Effect classes and functions.
//

// Standard radial distribution and incidence direction,
// taking account of inner radius.
class Effect_None
{
protected:
	FLOAT	Diffuse;					// Cached local values.
	FLOAT*	LightDistTab;

public:

	inline void Init( FLightInfo* Info )  
	{
		LightDistTab = Info->LightDistTab;
		Diffuse = Info->Diffuse * Info->Radius / (Info->Radius - Info->RadiusInner);
	}

	inline static bool Visible( const FVector& )
	{
		return true;
	}

	inline FLOAT Spatial( DWORD DistSq, const FVector& )
	{
		return MinPositiveFloat( Diffuse * LightDistTab[DistSq], 1.0f );
	}
	inline static FLOAT Scale( const FVector& )
	{
		return 1.0f;
	}
};

// Optimised for no inner radius.
class Effect_Simple: public Effect_None
{
public:
	inline FLOAT Spatial( DWORD DistSq, const FVector& )
	{
		return Diffuse * LightDistTab[DistSq];
	}
};

void FLightManager::spatial_None( FTextureInfo& Map, FLightInfo* Info, BYTE* Src, BYTE* Dest )
{
	if( Info->RadiusInner == 0.0f )
		// Optimised for standard case.
		SpatialRadial( (Effect_Simple*)0, Map, Info, Src, Dest );
	else
		SpatialRadial( (Effect_None*)0, Map, Info, Src, Dest );
}

class Effect_Cycle: public Effect_None
// Base class for all cycling effects.
{
protected:
	FLOAT TimeSeconds;
	FLOAT CycleAngle;
	INT   CycleAngTab;

public:

	void Init( FLightInfo* Info )  
	{
		Effect_None::Init( Info );
		TimeSeconds = FLightManager::LevelInfo->TimeSeconds;
		CycleAngTab = INT(Info->Actor->LightPeriod ? TimeSeconds * (1<<16<<4) / Info->Actor->LightPeriod : 0);
		CycleAngTab += Info->Actor->LightPhase << 8;
		CycleAngle = CycleAngTab * TO_RADIANS;
	}
};

// Yawing searchlight effect.
class Effect_SearchLight: public Effect_Cycle
{
	FLOAT Angle;

public:

	void Init( FLightInfo* Info )
	{
		Effect_Cycle::Init( Info );
		CycleAngle += 0.5f*PI;
	}

	inline bool Visible( const FVector& Vertex )
	{
		Angle = appFmod( 4.0f * (CycleAngle + appAtan2( Vertex.X, Vertex .Y)), 8.f*PI );
		return Angle>=PI && Angle<=PI*3.0f;
	}

	inline FLOAT Scale( const FVector& Vertex )
	{
		FLOAT Scale = 0.5f + 0.5f * GMath.CosFloat(Angle);
		FLOAT D     = 0.00006f * (Square(Vertex.X) + Square(Vertex.Y));
		if( D < 1.0f )
			Scale *= D;
		return Scale;
	}
};
MAKE_SPATIAL_FUNC(SearchLight)

// Yawing rotor effect.
class Effect_Rotor: public Effect_Cycle
{
public:
	inline FLOAT Scale( const FVector& Vertex )
	{
		FLOAT Angle = 6.0f * appAtan2(Vertex.X,Vertex.Y) + CycleAngle;
		FLOAT Scale = 0.5f + 0.5f * GMath.CosFloat(Angle);
		FLOAT D     = 0.0001f * (Square(Vertex.X) + Square(Vertex.Y));
		if (D<1.0f) Scale = 1.0f - D + Scale * D;
		return Scale;
	}
};
MAKE_SPATIAL_FUNC(Rotor)

// Radial waves.
class Effect_SlowWave: public Effect_Cycle
{
public:
	inline FLOAT Scale( const FVector& Vertex )
	{
		return 0.7f + 0.3f * GMath.SinTab(appRound(Vertex.SizeApprox() * 1024.0f) - CycleAngTab);
	}
};
MAKE_SPATIAL_FUNC(SlowWave)

class Effect_FastWave: public Effect_Cycle
{
public:
	inline FLOAT Scale( const FVector& Vertex )
	{
		return 0.7f + 0.3f * GMath.SinTab( appRound(Vertex.SizeApprox() * 512.0f) - CycleAngTab );
	}
};
MAKE_SPATIAL_FUNC(FastWave)

// Shock wave.
class Effect_Shock: public Effect_Cycle
{
public:
	inline FLOAT Scale( const FVector& Vertex )
	{
		INT Dist = INT (8.0f * Vertex.SizeApprox());
		FLOAT Brightness  = 0.9f + 0.1f * GMath.SinTab((Dist<<5) - CycleAngTab);
		Brightness       *= 0.9f + 0.1f * GMath.CosTab((Dist<<4) + CycleAngTab);
		Brightness       *= 0.9f + 0.1f * GMath.SinTab((Dist>>3) - CycleAngTab);
		return Brightness;
	}
};
MAKE_SPATIAL_FUNC(Shock)

// Disco ball.
class Effect_Disco: public Effect_Cycle
{
public:
	inline FLOAT Scale( const FVector& Vertex )
	{
		FLOAT Yaw	= 11.0f * appAtan2(Vertex.X,Vertex.Y);
		FLOAT Pitch = 11.0f * appAtan2(GMath.SqrtApprox(Square(Vertex.X)+Square(Vertex.Y)),Vertex.Z);

		FLOAT Scale1 = 0.50f + 0.50f * GMath.CosFloat(Yaw   + CycleAngle);
		FLOAT Scale2 = 0.50f + 0.50f * GMath.CosFloat(Pitch + CycleAngle);

		FLOAT Scale  = Scale1 + Scale2 - Scale1 * Scale2;

		FLOAT D = 0.00005f * (Square(Vertex.X) + Square(Vertex.Y));
		if (D<1.0f) Scale *= D;

		return (1.0f-Scale);
	}
};
MAKE_SPATIAL_FUNC(Disco)

// Interference pattern.
class Effect_Interference: public Effect_Cycle
{
public:
	inline FLOAT Scale( const FVector& Vertex )
	{
		FLOAT Pitch = 11.0f * appAtan2(GMath.SqrtApprox(Square(Vertex.X)+Square(Vertex.Y)),Vertex.Z);
		return 0.50f + 0.50f * GMath.CosFloat(Pitch + CycleAngle);
	}
};
MAKE_SPATIAL_FUNC(Interference)

// Spotlight lighting.
class Effect_Spotlight: public Effect_None
{
	FVector View;
	FLOAT   RSine;
	FLOAT   SineRSine;
	FLOAT   SineSq;
	FLOAT	SizeSq;
	FLOAT	VDotV;

public:
	void Init( FLightInfo* Info )
	{
		Effect_None::Init( Info );

		View      = Info->Actor->GetViewRotation().Vector();
		FLOAT Sine= 1.0f - Info->Actor->LightCone / 256.0f;
		RSine     = 1.0f / (1.0f - Sine);
		SineRSine = Sine * RSine;
		SineSq    = Sine * Sine;
	}

	inline bool Visible( const FVector& Vertex )
	{
		SizeSq = Vertex | Vertex;
		VDotV  = Vertex | View;
		return VDotV > 0.0f && Square(VDotV) > SineSq * SizeSq;
	}
	inline FLOAT Scale( const FVector& Vertex )
	{
		return Square( VDotV * RSine * GMath.DivSqrtApprox(SizeSq) - SineRSine );
	}
};
MAKE_SPATIAL_FUNC(Spotlight)

//
// Alternate spatial distributions.
//

// Cylinder pattern.
class Effect_Cylinder: public Effect_None
{
	FLOAT RRadiusSq;
public:
	inline void Init( FLightInfo* Info )
	{
		RRadiusSq = Square(Info->RRadius);
	}
	inline FLOAT Spatial( DWORD, const FVector& Vertex )
	{
		return Max(0.0f, (1.0f - ( Square(Vertex.X) + Square(Vertex.Y) ) * RRadiusSq) );
	}
};
MAKE_SPATIAL_FUNC(Cylinder)

// Non-incident, i.e. ambient lighting. Equivalent to LS_Ambient source.
class Effect_NonIncidence: public Effect_None
{
public:
	inline void Init( FLightInfo* Info )  
	{
		Effect_None::Init( Info );

		// Override Info to create ambient incidence.
		LightDistTab = FLightManager::LightDist;
		Diffuse = Info->Radius / (Info->Radius - Info->RadiusInner);
	}
};
MAKE_SPATIAL_FUNC(NonIncidence)

// Shell pattern.
class Effect_Shell: public Effect_None
{
public:
	inline FLOAT Spatial( DWORD DistSq, const FVector& Vertex )
	{
		// Get straight dist value, munge so that it lies in a shell.
		FLOAT Amp = FLightManager::LightDist[DistSq];
		return Max(0.0f, 1.0f - 10.0f*Abs(Amp-0.1f));
	}
};
MAKE_SPATIAL_FUNC(Shell)

/*------------------------------------------------------------------------------------
	Global light effects.
------------------------------------------------------------------------------------*/

// No global lighting
static void global_None( AActor* Owner, FLOAT& Brightness, FVector& Color )
{
	guardSlow(global_None);
	Brightness=0.0f;
	unguardSlow;
}

// Steady global lighting
static void global_Steady( AActor* Owner, FLOAT& Brightness, FVector& Color )
{
	guardSlow(global_Steady);
	unguardSlow;
}

// Global light pulsing effect
static void global_Pulse( AActor* Owner, FLOAT& Brightness, FVector& Color )
{
	guardSlow(global_Pulse);
	Brightness *= 0.6f + 0.39f * GMath.SinTab
	(
		appRound((Owner->Level->TimeSeconds * 35.0f * 65536.0f) / Max((INT)Owner->LightPeriod,1) + (Owner->LightPhase << 8))
	);
	unguardSlow;
}

// Global blinking effect
static void global_Blink( AActor* Owner, FLOAT& Brightness, FVector& Color )
{
	guardSlow(global_Blink);
	if( appRound((Owner->Level->TimeSeconds * 35.0f * 65536.0f)/(Owner->LightPeriod+1) + (Owner->LightPhase << 8)) & 1 )
		Brightness = 0.0f;
	unguardSlow;
}

// Global flicker effect
static void global_Flicker( AActor* Owner, FLOAT& Brightness, FVector& Color )
{
	guardSlow(global_Flicker);

	FLOAT Random = GRandoms->RandomBase( reinterpret_cast<UPTRINT>(Owner) );
	if( Random < 0.5f )	Brightness = 0.0f;
	else				Brightness *= Random;
	unguardSlow;
}

// Strobe light.
static void global_Strobe( AActor* Owner, FLOAT& Brightness, FVector& Color )
{
	guardSlow(global_Strobe);
	static float LastUpdateTime=0; static int Toggle=0;
	if( LastUpdateTime != Owner->Level->TimeSeconds )
	{
		LastUpdateTime = Owner->Level->TimeSeconds;
		Toggle ^= 1;
	}
	if( Toggle ) Brightness = 0.0f;
	unguardSlow;
}

// Simulated light emmanating from the backdrop.
static void global_BackdropLight( AActor* Owner, FLOAT& Brightness, FVector& Color )
{
	guardSlow(global_BackdropLight);
	unguardSlow;
}

// Global subtle light pulsing effect
static void global_SubtlePulse( AActor* Owner, FLOAT& Brightness, FVector& Color )
{
	guardSlow(global_Pulse);
	Brightness *= 0.9f + 0.09f * GMath.SinTab
	(
		(Owner->Level->TimeSeconds * 35.0f * 65536.0f) / Max((INT)Owner->LightPeriod,1) + (Owner->LightPhase << 8)
	);
	unguardSlow;
}

// Use texture palette with LifeSpan to indicate index.
static void global_TexturePaletteOnce( AActor* Owner, FLOAT& Brightness, FVector& Color )
{
	guardSlow(global_TexturePaletteOnce);
	if( Owner->Skin && Owner->Skin->Palette )
	{
		FColor C = Owner->Skin->Palette->Colors(appFloor(255.0f * Owner->LifeFraction()));
		Color = FVector( C.R, C.G, C.B ).SafeNormal();
		Brightness *= C.FBrightness() * 2.8f;
	}
	unguardSlow;
}

// Use texture palette, looping over and over.
static void global_TexturePaletteLoop( AActor* Owner, FLOAT& Brightness, FVector& Color )
{
	guardSlow(global_TexturePaletteLoop);
	if( Owner->Skin && Owner->Skin->Palette )
	{
		FLOAT Time        = Owner->Level->TimeSeconds * 35 / Max((int)Owner->LightPeriod,1) + Owner->LightPhase;
		FColor C          = Owner->Skin->Palette->Colors((appRound(Time*256) & 255) % 255);
		Color             = FVector( C.R, C.G, C.B ).UnsafeNormal();
		Brightness       *= C.FBrightness() * 2.8f;
	}
	unguardSlow;
}

// Table of global lighting functions.
typedef void (*LIGHT_TYPE_FUNC)( AActor* Owner, FLOAT& Brightness, FVector& Color );
static const LIGHT_TYPE_FUNC GLightTypeFuncs[LT_MAX] =
{
	global_None,
	global_Steady,
	global_Pulse,
	global_Blink,
	global_Flicker,
	global_Strobe,
	global_BackdropLight,
	global_SubtlePulse,
	global_TexturePaletteOnce,
	global_TexturePaletteLoop
};

// Compute global lighting for an actor.
void URender::GlobalLighting( UBOOL Realtime, AActor* Owner, FLOAT& Brightness, FPlane& Color )
{
	guard(URender::GlobalLighting);

	// Figure out global dynamic lighting effect.
	ELightType Type = (ELightType)Owner->LightType;
	if( !Realtime )
		Type = LT_Steady;

	// Coloring.
	Color = FGetHSV( Owner->LightHue, Owner->LightSaturation, 255 );

	// Only compute global lighting effect once per actor per frame, so that
	// lights with random functions produce consistent lighting on all surfaces they hit!!
	if( Type<LT_MAX )
		GLightTypeFuncs[Type]( Owner, Brightness, Color );
	Brightness = Clamp( Brightness, 0.0f, 1.0f );

	unguard;
}

/*------------------------------------------------------------------------------------
	Implementation of FLightInfo class
------------------------------------------------------------------------------------*/

//
// Compute lighting information based on an actor lightsource.
//
void FLightInfo::ComputeFromActor( FTextureInfo* Map, FSceneNode* Frame )
{
	guard(FLightInfo::ComputeFromActor);

	// General setup.
	Radius			= Actor->WorldLightRadius();
	RadiusInner		= Radius * Actor->LightRadiusInner / 256.0f;
	RRadius			= 1.0f/Max<FLOAT>(1.0f,Radius);
	RRadiusMult		= 1.0f/Max<FLOAT>(1.0f, Radius - RadiusInner);
	Location		= Actor->Location.TransformPointBy( Frame->Coords );
	Brightness      = Actor->LightBrightness/255.0f;
	MaxSpecDir		= FVector(0);
	Effect          = FLightManager::Effects[(Actor->LightEffect<LE_MAX) ? Actor->LightEffect : 0];
	GRender->GlobalLighting( (Frame->Viewport->Actor->ShowFlags&SHOW_PlayerCtrl)!=0, Actor, Brightness, FloatColor );
	FloatColor     *= Brightness * Actor->Level->Brightness;

	if( Actor->LightSource == LD_Plane )
		LightDir = Actor->GetViewRotation().Vector();
	else
		LightDir = FVector(0);

	// Surface setup.
	if( Map )
	{
		// Compute coords.
		if( !FLightManager::TemporaryTablesBuilt )
		{
			FLightManager::TemporaryTablesBuilt = 1;
			FLightManager::MapUncoords = FLightManager::MapCoords->Inverse().Transpose();
			FLightManager::VertexBase  = FLightManager::MapCoords->Origin + FLightManager::MapUncoords.XAxis*Map->Pan.X + FLightManager::MapUncoords.YAxis*Map->Pan.Y;
			FLightManager::VertexDU    = FLightManager::MapUncoords.XAxis * Map->UScale;
			FLightManager::VertexDV    = FLightManager::MapUncoords.YAxis * Map->VScale;
		}

		// Surface lighting.
		switch( Actor->LightSource )
		{
		default:
			// Radial illumination.
			Diffuse = Abs((Actor->Location - FLightManager::MapCoords->Origin) | FLightManager::MapCoords->ZAxis) * RRadius;
			LightDistTab = FLightManager::LightDistInc;
			break;
		case LD_Plane:
			Diffuse = Max( -(Actor->GetViewRotation().Vector() | FLightManager::MapCoords->ZAxis), 0.f );
			LightDistTab = FLightManager::LightDist;
			break;
		case LD_Ambient:
			Diffuse = 1.0f;
			LightDistTab = FLightManager::LightDist;
			break;
		}

		// Cache the scaler palette.
		STAT(clock(GStat.ExtraTime));
		QWORD CacheID = MakeCacheID( CID_LightPalette, Actor );
		FVector* Color = (FVector*)GCache.Get(CacheID,*FLightManager::TopItemToUnlock++);
		if( !Color || *Color!=FloatColor || Actor->bLightChanged )
		{
			// Create or replace the palette.
			if( !Color )
				Color = (FVector*)GCache.Create(CacheID,FLightManager::TopItemToUnlock[-1],appCheckedIntSize(sizeof(FVector)+256*sizeof(FColor)));
			*Color = FloatColor;
			Palette = (FColor*)(Color+1);
			INT FixR = 0; INT FixDR=appFloor(FloatColor.X*65536.0f);
			INT FixG = 0; INT FixDG=appFloor(FloatColor.Y*65536.0f);
			INT FixB = 0; INT FixDB=appFloor(FloatColor.Z*65536.0f);
			for( INT i=0; i<256; i++ )
			{
				Palette[i].B = Min(Unfix(FixR),127); FixR+=FixDR;
				Palette[i].G = Min(Unfix(FixG),127); FixG+=FixDG;
				Palette[i].R = Min(Unfix(FixB),127); FixB+=FixDB;
				Palette[i].A = 255;
			}
		}
		Palette = (FColor*)(Color+1);
		STAT(unclock(GStat.ExtraTime));

		// Compute clipping region.
		FLOAT   PlaneDot    = (Actor->Location - FLightManager::MapCoords->Origin) | FLightManager::MapCoords->ZAxis;
		FLOAT   Radius      = Actor->WorldLightRadius();
		FLOAT   PlaneRadius = SqrtApprox( Max<FLOAT>( Radius*Radius*1.05f - PlaneDot*PlaneDot, 0.0f ) );
		FVector Center      = Actor->Location - FLightManager::MapCoords->ZAxis*PlaneDot;
		FLOAT   CenterU     = ((Center - FLightManager::MapCoords->Origin) | FLightManager::MapCoords->XAxis) - FLightManager::LightMap.Pan.X;
		FLOAT   CenterV     = ((Center - FLightManager::MapCoords->Origin) | FLightManager::MapCoords->YAxis) - FLightManager::LightMap.Pan.Y;
		FLOAT   RadiusU     = PlaneRadius * SqrtApprox(FLightManager::MapCoords->XAxis.SizeSquared());
		FLOAT   RadiusV     = PlaneRadius * SqrtApprox(FLightManager::MapCoords->YAxis.SizeSquared());

		// Save clipping region.
		MinU = Max( appRound( (CenterU - RadiusU)/FLightManager::LightMap.UScale), 0 );
		MinV = Max( appRound( (CenterV - RadiusV)/FLightManager::LightMap.VScale), 0 );
		MaxU = Min( appRound( (CenterU + RadiusU)/FLightManager::LightMap.UScale), FLightManager::LightMap.UClamp );
		MaxV = Min( appRound( (CenterV + RadiusV)/FLightManager::LightMap.VScale), FLightManager::LightMap.VClamp );
	}

	// Init volumetric lighting.
	if( IsVolumetric )
	{		
		VolumetricColor   = FloatColor;   
		VolumetricColor.W = (FLOAT)Actor->VolumeFog * (1.0f/255.0f);

		// Cache the volumetric color scaler palette
		STAT(clock(GStat.ExtraTime));
		QWORD CacheID = MakeCacheID( CID_VolumetricScaler, Actor );
		FPlane* Color = (FPlane*)GCache.Get(CacheID,*FLightManager::TopItemToUnlock++);
		if( !Color || *Color!=VolumetricColor || Actor->bLightChanged )
		{
			// Create or replace the palette.
			if( !Color )
				Color = (FPlane*)GCache.Create(CacheID,FLightManager::TopItemToUnlock[-1],appCheckedIntSize(sizeof(FPlane)+256*sizeof(FColor)));
			*Color = VolumetricColor;
			VolPalette = (FColor*)(Color+1);
			INT FixR = 0; INT FixDR=appFloor(VolumetricColor.X*65536.0f);
			INT FixG = 0; INT FixDG=appFloor(VolumetricColor.Y*65536.0f);
			INT FixB = 0; INT FixDB=appFloor(VolumetricColor.Z*65536.0f);
			INT FixA = 0; INT FixDA=appFloor(VolumetricColor.W*65536.0f);
			for( INT i=0; i<256; i++ )
			{
				VolPalette[i].B = Min(Unfix(FixR),127); FixR+=FixDR;
				VolPalette[i].G = Min(Unfix(FixG),127); FixG+=FixDG;
				VolPalette[i].R = Min(Unfix(FixB),127); FixB+=FixDB;
				VolPalette[i].A = Min(Unfix(FixA),127); FixA+=FixDA;
			}
		}
		VolPalette = (FColor*)(Color+1);
		STAT(unclock(GStat.ExtraTime));

		VolRadius			= Actor->WorldVolumetricRadius();
		VolRadiusSquared	= VolRadius * VolRadius;
		RVolRadius          = 1.0f/VolRadius;
		RVolRadiusSquared   = RVolRadius * RVolRadius;
		LocationSizeSquared = Location.SizeSquared();
		VolBrightness		= Brightness * Actor->VolumeBrightness / 64.0f;
		VolInside           = (LocationSizeSquared < VolRadiusSquared);
	}
	unguard;
}

/*------------------------------------------------------------------------------------
	Implementation of FLightList class
------------------------------------------------------------------------------------*/

//
// For sorting volumetric lights by distance.
//
INT CDECL LightInfoDistCompare( const void* A, const void* B )
{
	return 2*((*(FLightInfo**)A)->Location.Z<(*(FLightInfo**)B)->Location.Z)-1;
}

//
// Compute fast light list for a surface.
//
#pragma warning (disable : 4799)
void FLightManager::SetupForSurf
(
	FSceneNode*		InFrame,
	FCoords&		InMapCoords,
	FBspDrawList*	Draw,
	FTextureInfo*&	OutLightMap,
	FTextureInfo*&	OutFogMap,
	UBOOL			Merged
)
{
	guard(FLightManager::SetupForSurf);
	STAT(clock(GStat.IllumTime));
	INT Key=0;

#if 0
	// To regenerate all lighting every frame, uncomment this.
	guard(FlushAll);
	GCache.Flush(MakeCacheID(CID_StaticMap      ,0,0),MakeCacheID(CID_MAX,0,0,NULL));
	GCache.Flush(MakeCacheID(CID_DynamicMap     ,0,0),MakeCacheID(CID_MAX,0,0,NULL));
	GCache.Flush(MakeCacheID(CID_ShadowMap      ,0,0),MakeCacheID(CID_MAX,0,0,NULL));
	GCache.Flush(MakeCacheID(CID_IlluminationMap,0,0),MakeCacheID(CID_MAX,0,0,NULL));
	unguard;
#endif

	// Init mip pointer.
	LightMap.bRealtimeChanged	= 0;
	LightMap.NumMips			= 1;
	LightMap.Mips[0]			= &LightMip;
	LightMap.Format				= TEXF_RGBA7;
	LightMap.Palette			= NULL;
	LightMap.Mips[0]->DataPtr	= NULL;

	// Fog.
	FogMap.bRealtimeChanged	    = 0;
	FogMap.NumMips				= 1;
	FogMap.Mips[0]				= &FogMip;
	FogMap.Format				= TEXF_RGBA7;
	FogMap.Palette				= NULL;
	FogMap.Mips[0]->DataPtr		= NULL;

	// Set up variables.
	Mark						= FMemMark(GMem);
	Frame						= InFrame;
	Level						= Frame->Level;
	INT iLightMap				= Level->Model->Surfs(Draw->iSurf).iLightMap;
	LevelInfo					= Level->GetLevelInfo();
	Zone						= NULL;
	TemporaryTablesBuilt		= 0;	
	FBspSurf& Surf				= Level->Model->Surfs(Draw->iSurf);
	AMover* Mover				= (Frame->Level->BrushTracker && Frame->Level->BrushTracker->SurfIsDynamic(Draw->iSurf)) ?  (AMover*)Surf.Actor : NULL;
	UModel* Model				= Mover ? Mover->Brush : Level->Model;
	FLightMapIndex* Index		= &Model->LightMap(iLightMap);
	Zone						= Draw->Zone;
	BYTE ZoneID					= Zone ? Zone->Region.ZoneNumber : 255;
	LastLight					= FirstLight;
	StaticLights				= 0;
	DynamicLights				= 0;
	MovingLights				= 0;
	StaticLightingChanged		= 0;
	MapCoords					= &InMapCoords;
	ShadowMaskU					= (Index->UClamp+7) >> 3;
	ShadowMaskSpace				= ShadowMaskU * Index->VClamp;
	ShadowSkip					= ShadowMaskU*8 - Index->UClamp;
	OutLightMap					= &LightMap;

	// Handle lighting.
	if( !Mover || !Mover->bDynamicLightMover )
	{
		guard(SetupNormalSurface);
		Mover = NULL;

		// Static lights.
		BYTE* ShadowBase = &Model->LightBits(Index->DataOffset);
		if( Index->iLightActors != INDEX_NONE )
			for( INT i=0; Model->Lights(i+Index->iLightActors); i++,ShadowBase+=ShadowMaskSpace )
				if( AddLight( Mover, Model->Lights(i+Index->iLightActors) ) )
					LastLight[-1].ShadowBits = ShadowBase;

		// Dynamic lights.
		for( FActorLink* Link=Draw->SurfLights; Link; Link=Link->Next )
			AddLight( Mover, Link->Actor );

		unguard;
	}
	else if( Mover->Region.iLeaf!=INDEX_NONE && Level->Model->Leaves.Num() )
	{
		guard(SetupMoverSurface);

		// Static mover lights.
		FLeaf& Leaf = Level->Model->Leaves(Mover->Region.iLeaf);
		if( Leaf.iPermeating!=INDEX_NONE )
			for( INT i=Leaf.iPermeating; Level->Model->Lights(i); i++ )
				if( ((Level->Model->Lights(i)->Location - MapCoords->Origin)|MapCoords->ZAxis) > 0.0f && Level->Model->Lights(i)->bSpecialLit==Mover->bSpecialLit )
					AddLight( NULL, Level->Model->Lights(i) );

		// Dynamic mover lights.
		for( FActorLink* Link=Draw->SurfLights; Link; Link=Link->Next )
			if( Link->Actor->bSpecialLit==Mover->bSpecialLit )
				AddLight( Mover, Link->Actor );

		// Volumetric mover lights.
		for( FActorLink* Volumetrics=Draw->Volumetrics; Volumetrics && LastLight<FinalLight; Volumetrics=Volumetrics->Next )
		{
			FLightInfo* Find = FirstLight;
			for( ; Find<LastLight && Find->Actor!=Volumetrics->Actor; Find++ );
			Find->IsVolumetric = 1;
			if( Find==LastLight )
			{
				// New volumetric light.
				LastLight->Actor = Volumetrics->Actor;
				LastLight->Opt   = ALO_NotLight;
				LastLight++;
			}
		}
		unguard;
	}

	// Volumetric lights.
	if( Zone && Zone->bFogZone && Draw->Volumetrics && !(Draw->PolyFlags&PF_Translucent) )
	{
		guard(SetupVolumetrics);
		OutFogMap = &FogMap;
		for( FActorLink* Link=Draw->Volumetrics; Link && LastLight<FinalLight; Link=Link->Next )
		{
			// See if volumetric light is already on the list as a regular light.
			FLightInfo* Find = FirstLight;
			for( ; Find<LastLight && Find->Actor!=Link->Actor; Find++ );
			Find->IsVolumetric = 1;
			if( Find==LastLight )
			{
				// New volumetric light.
				LastLight->Actor     = Link->Actor;
				LastLight->Opt       = ALO_NotLight;
				LastLight++;
			}
		}

		// Setup FogMip and FogMap.
		FogMip.UBits			= appCeilLogTwo(Index->UClamp);
		FogMip.VBits			= appCeilLogTwo(Index->VClamp);
		FogMip.USize			= 1 << FogMip.UBits;
		FogMip.VSize			= 1 << FogMip.VBits;
		FogMap.Pan				= Index->Pan;
		FogMap.UScale			= Index->UScale;
		FogMap.VScale			= Index->VScale;
		FogMap.UClamp			= Index->UClamp;
		FogMap.VClamp			= Index->VClamp;
		FogMap.USize			= FogMip.USize;
		FogMap.VSize			= FogMip.VSize;
		FogMap.bRealtimeChanged = 1;
		FogMap.CacheID			= MakeCacheID( CID_RenderFogMap, iLightMap, ZoneID, Model );

		// Setup the volumetrics.
		FogMip.DataPtr = New<BYTE>(GMem,appCheckedIntSize(static_cast<SIZE_T>(FogMap.USize)*static_cast<SIZE_T>(FogMap.VSize)*sizeof(DWORD)+sizeof(FColor)));
		FogMap.MaxColor = (FColor*)FogMip.DataPtr; FogMip.DataPtr += sizeof(FColor);
		*FogMap.MaxColor = FColor(255,255,255,255);
		unguard;

		// Merge the volumetrics.
		guard(MergeVolumetrics);
		UBOOL VirginFog = 1;
		LastVtric = FirstVtric;
		for( FLightInfo* Info=FirstLight; Info<LastLight; Info++ )
		{
			if( Info->IsVolumetric )
			{
				Info->ComputeFromActor( &FogMap, Frame );
				*LastVtric++ = Info;
			}
		}
		if( LastVtric != FirstVtric )
			appQsort( FirstVtric, appCheckedIntSize(static_cast<SIZE_T>(LastVtric-FirstVtric)), sizeof(FLightInfo*), LightInfoDistCompare );
		for( FLightInfo** Ptr=FirstVtric; Ptr<LastVtric; Ptr++ )
		{
			// Compute the volumetric.
			FLightInfo* Info = *Ptr;
			FVector	Vertex1  = VertexBase.TransformPointBy ( Frame->Coords );
			FVector VertDU   = VertexDU  .TransformVectorBy( Frame->Coords );
			FVector VertDV   = VertexDV  .TransformVectorBy( Frame->Coords );
			if( VirginFog )
			{					
				// First-time fog calculation, no merging required.
				FColor* Dest = (FColor*)FogMip.DataPtr;
				for( INT i=0; i<FogMap.VClamp; i++ )
				{
					FVector Vertex = Vertex1;
					for( INT j=0; j<FogMap.UClamp; j++,Vertex += VertDU )
						Dest[j] = Info->VolPalette[appRound( Volumetric( Info, Vertex ) * 255.0f )];
					Vertex1 += VertDV;
					Dest    += FogMap.USize; 
				}
				VirginFog = 0;
			}
			else
			{
				// Merge in more fog. 
				FColor* Dest = (FColor*)FogMip.DataPtr;
				for( INT i=0; i<FogMap.VClamp; i++ )
				{
					FVector Vertex = Vertex1;
					for( INT j=0; j<FogMap.UClamp; j++,Vertex+=VertDU )
					{
						FLOAT FogAdd = Volumetric( Info, Vertex );
						if( *(DWORD*)&FogAdd )
						{
							DWORD Light = appRound( FogAdd  * 255.0f );
							BYTE* Table = &ByteFog[128*(INT)Info->VolPalette[Light].A];
							Dest[j].R = Min(Table[Dest[j].R]+Info->VolPalette[Light].R,127); 
							Dest[j].G = Min(Table[Dest[j].G]+Info->VolPalette[Light].G,127); 
							Dest[j].B = Min(Table[Dest[j].B]+Info->VolPalette[Light].B,127); 
							Dest[j].A = Min(Dest[j].A+Info->VolPalette[Light].A,127); 
						}
					}
					Vertex1 += VertDV;
					Dest += FogMap.USize;
				}

			}
		}
		unguard;
	}

	// Set up LightMip and LightMap.
	guard(SetupLightMap);
	LightMip.UBits			= appCeilLogTwo(Index->UClamp);
	LightMip.VBits			= appCeilLogTwo(Index->VClamp);
	LightMip.USize			= 1 << LightMip.UBits;
	LightMip.VSize			= 1 << LightMip.VBits;
	LightMap.Pan			= Index->Pan;
	LightMap.UScale			= Index->UScale;
	LightMap.VScale			= Index->VScale;
	LightMap.UClamp			= Index->UClamp;
	LightMap.VClamp			= Index->VClamp;
	LightMap.USize			= LightMip.USize;
	LightMap.VSize			= LightMip.VSize;
	LightMap.CacheID		= MakeCacheID( CID_StaticMap, iLightMap, ZoneID, Model );
	unguard;

	// Handle static lighting.
	DWORD* Stream = (DWORD*)GCache.Get(LightMap.CacheID,*TopItemToUnlock++);
	struct FMoverStamp{ INT iLeaf; FVector Location; FRotator Rotation; };
	if( Mover && Stream )
	{
		StaticLightingChanged |= ((FMoverStamp*)Stream)->iLeaf    != Mover->Region.iLeaf;
		StaticLightingChanged |= ((FMoverStamp*)Stream)->Location != Mover->Location;
		StaticLightingChanged |= ((FMoverStamp*)Stream)->Rotation != Mover->Rotation;
	}
	if( !Stream || StaticLightingChanged )
	{
		// Setup caching.
		guard(StaticLighting);
		StaticLightingChanged=1;
		if( !Stream )
			Stream = (DWORD*)GCache.Create( LightMap.CacheID, TopItemToUnlock[-1], appCheckedIntSize(static_cast<SIZE_T>(LightMap.USize)*static_cast<SIZE_T>(LightMap.VClamp)*sizeof(DWORD) + sizeof(FColor) + sizeof(FMoverStamp)), DEFAULT_ALIGNMENT, appCheckedIntSize(static_cast<SIZE_T>(LightMap.USize)*static_cast<SIZE_T>(LightMap.VSize-LightMap.VClamp)) );
		if( Mover )
		{
			((FMoverStamp*)Stream)->iLeaf    = Mover->Region.iLeaf;
			((FMoverStamp*)Stream)->Location = Mover->Location;
			((FMoverStamp*)Stream)->Rotation = Mover->Rotation;
		}
		Stream += sizeof(FMoverStamp)/sizeof(DWORD);
		LightMap.MaxColor = (FColor*)Stream++;
		*LightMap.MaxColor = FColor(255,255,255,255);

		// Generate ambient color.
		AmbientVector = FGetHSV( Zone->AmbientHue, Zone->AmbientSaturation, Zone->AmbientBrightness );
		FColor AmbientColor( AmbientVector*0.25f );
		Exchange( AmbientColor.R, AmbientColor.B );
		AmbientColor.A = 127;
		DWORD* Temp = Stream;
		for( INT i=0; i<LightMap.VClamp; i++ )
		{
			for( INT j=0; j<LightMap.UClamp; j++ )
				Temp[j] = GET_COLOR_DWORD(AmbientColor);
			Temp += LightMap.USize;
		}

		// Add in all static lights.
		FMemMark Mark(GMem);
		for( FLightInfo* Info = FirstLight; Info < LastLight; Info++ )
		{
			if( Info->Opt == ALO_StaticLight )
			{
				// Static lighting.
				Info->ComputeFromActor( &LightMap, Frame );
				BYTE* ShadowMap = New<BYTE>(GMem,ShadowMaskSpace*8);
				ShadowMapGen( LightMap, Info->ShadowBits, ShadowMap );
				Info->IlluminationMap = New<BYTE>(GMem,LightMap.UClamp*LightMap.VClamp);
				Info->Effect.SpatialFxFunc( LightMap, Info, ShadowMap, Info->IlluminationMap );
				Merge( LightMap, Info->Actor->LightEffect, 0, Info, Stream, Stream );
				Mark.Pop();
			}
		}
		unguard;
	}
	else
	{
		Stream += sizeof(FMoverStamp)/sizeof(DWORD);
		LightMap.MaxColor = (FColor*)Stream++;
	}

	// Merge in the dynamic lights.
	if( DynamicLights || MovingLights )
	{
		guard(DynamicLight);
		DWORD* Static = Stream;
		LightMap.CacheID = MakeCacheID( CID_DynamicMap, iLightMap, ZoneID, Model );
		if( Merged )
		{
			// Allocate in temporary memory.
			Stream = New<DWORD>(GMem,LightMap.USize*LightMap.VSize+1);
			LightMap.MaxColor = (FColor*)Stream++;
		}
		else
		{
			// Cache it.
			Stream = (DWORD*)GCache.Get( LightMap.CacheID, *TopItemToUnlock++ );
			if( !Stream || *(FTime*)Stream!=Frame->Viewport->CurrentTime )
			{
				if( !Stream )
					Stream = (DWORD*)GCache.Create( LightMap.CacheID, TopItemToUnlock[-1], appCheckedIntSize((static_cast<SIZE_T>(LightMap.USize)*static_cast<SIZE_T>(LightMap.VClamp) + 3)*sizeof(DWORD)), DEFAULT_ALIGNMENT, appCheckedIntSize(static_cast<SIZE_T>(LightMap.USize)*static_cast<SIZE_T>(LightMap.VSize-LightMap.VClamp)) );
				*(FTime*)Stream = Frame->Viewport->CurrentTime;
				Stream += 2;
				LightMap.MaxColor = (FColor*)Stream++;
			}
			else
			{
				*(FTime*)Stream = Frame->Viewport->CurrentTime;
				Stream += 2;
				LightMap.MaxColor = (FColor*)Stream++;
				goto SkipDynamicLight;
			}
		}
		*LightMap.MaxColor = FColor(255,255,255,255);

		// Copy the static lighting.
		DWORD *TmpStatic=Static, *TmpStream=Stream;
		for( INT i=0; i<LightMap.VClamp; i++ )
		{
			appMemcpy( TmpStream, TmpStatic, LightMap.UClamp*sizeof(DWORD) );
			TmpStatic += LightMap.USize;
			TmpStream += LightMap.USize;
		}

		// Merge in the dynamic lights.
		FMemMark Mark(GMem);
		LightMap.bRealtimeChanged = 1;
		for( FLightInfo* Info=FirstLight; Info<LastLight; Info++ )
		{
			if( Info->Opt==ALO_DynamicLight || Info->Opt==ALO_MovingLight )
			{	
				// Set up.
				BYTE* ShadowMap;
				Info->ComputeFromActor( &LightMap, Frame );
				if( Info->Opt==ALO_MovingLight )
				{
					// Build a temporary shadow map and fill it with max.
					ShadowMap = New<BYTE>(GMem,ShadowMaskSpace*8);
					ShadowMapGen( LightMap, NULL, ShadowMap );

					// Build a temporary illumination map.
					Info->IlluminationMap = New<BYTE>(GMem,LightMap.UClamp*LightMap.VClamp);
					Info->Effect.SpatialFxFunc( LightMap, Info, ShadowMap, Info->IlluminationMap );
				}
				else if( Info->Effect.IsSpatialDynamic )
				{
					// This light has spatial effects, so we must cache its shadow map since
					// we will be generating its illumination map per frame.
					//note: we use iSurf because only (iLightMap,Actor,Mover) is unique.
					QWORD CacheID = MakeCacheID( CID_ShadowMap, Draw->iSurf/*iLightMap*/, 0, Info->Actor );
					ShadowMap = (BYTE *)GCache.Get( CacheID, *TopItemToUnlock++ );
					if( !ShadowMap  )
					{
						// Create and generate its shadow map.
						ShadowMap = (BYTE *)GCache.Create( CacheID, TopItemToUnlock[-1], ShadowMaskSpace*8 );
						ShadowMapGen( LightMap, Info->ShadowBits, ShadowMap );
					}

					// Build a temporary illumination map:
					Info->IlluminationMap = New<BYTE>(GMem,LightMap.UClamp*LightMap.VClamp);
					Info->Effect.SpatialFxFunc( LightMap, Info, ShadowMap, Info->IlluminationMap );
				}
				else
				{
					// No spatial lighting. We use a cached illumination map generated from a temporary
					// shadow map. See if the illumination map is already cached:
					//note: we use iSurf because only (iLightMap,Actor,Mover) is unique.
					QWORD CacheID = MakeCacheID( CID_IlluminationMap, Draw->iSurf/*iLightMap*/, 0, Info->Actor );
					Info->IlluminationMap = (BYTE *)GCache.Get( CacheID, *TopItemToUnlock++ );
					if( !Info->IlluminationMap || Info->Actor->bLightChanged )
					{
						// Build a temporary shadow map.
						ShadowMap = New<BYTE>(GMem,ShadowMaskSpace*8);
						ShadowMapGen( LightMap, Info->ShadowBits, ShadowMap );

						// Build and cache an illumination map
						if( !Info->IlluminationMap )
							Info->IlluminationMap = (BYTE *)GCache.Create( CacheID, TopItemToUnlock[-1], appCheckedIntSize((static_cast<SIZE_T>(LightMap.UClamp)*static_cast<SIZE_T>(LightMap.VClamp+1)+1)*sizeof(BYTE)) );
						Info->Effect.SpatialFxFunc( LightMap, Info, ShadowMap, Info->IlluminationMap );
					}
				}

				// Merge the illumination map in.
				Merge( LightMap, Info->Actor->LightEffect, Key, Info, Stream, Stream );
				Mark.Pop();
			}
		}
		unguard;
	}
	SkipDynamicLight:;

	// Set pointers.
	LightMip.DataPtr = (BYTE*)Stream;

	STAT(unclock(GStat.IllumTime));
	unguard;
}

//
// Finish surface lighting.
//
void FLightManager::FinishSurf()
{
	guard(FLightManager::FinishSurf);

	// Release working memory.
	Mark.Pop();

	// Unlock any locked cache items.
	while( TopItemToUnlock > &ItemsToUnlock[0] )
		(*--TopItemToUnlock)->Unlock();

	// Update stats.
	STAT(GStat.Lightage += LightMap.UClamp * LightMap.VClamp);
	STAT(GStat.LightMem += LightMap.UClamp * LightMap.VClamp * sizeof(FLOAT));

	unguard;
}

/*------------------------------------------------------------------------------------
	Light setup.
------------------------------------------------------------------------------------*/

//
// Add a light to the list.
//
UBOOL FLightManager::AddLight( AActor* Actor, AActor* Light )
{
	guardSlow(FLightManager::AddLight);

	// Reject.
	if
	(	LastLight>=FinalLight
	||	Light->LightType==LT_None
	||	Light->LightBrightness==0
	||	Light==Actor )
		return 0;

	// Set actor and optimization flags.
	if( Actor )
	{
		// Distance-reject will reject lights that partially hit the mesh
		// because it doesn't take the collision size into account.
		LastLight->Opt = ALO_MovingLight;
		MovingLights++;
	}
	else if( Light->LightEffect == LE_OmniBumpMap )
	{
		LastLight->Opt = ALO_NotLight;
	}
	else if( Actor || Light->bDynamicLight || !(Light->bStatic || Light->bNoDelete) )
	{
		if( Frame->Viewport->GetOuterUClient()->NoDynamicLights )
			return 0;
		LastLight->Opt = ALO_MovingLight;
		MovingLights++;
	}
	else if
	((	Light->bStatic 
	&&	Light->LightType==LT_Steady
	&&	!Effects[Light->LightEffect].IsSpatialDynamic
	&&	!Effects[Light->LightEffect].IsMergeDynamic) || Frame->Viewport->GetOuterUClient()->NoDynamicLights)
	{
		LastLight->Opt = ALO_StaticLight;
		StaticLights++;
	}
	else
	{
		LastLight->Opt = ALO_DynamicLight;
		DynamicLights++;
	}

	// Info.
	LastLight->Actor        = Light;
	LastLight->IsVolumetric = 0;
	LastLight->ShadowBits   = NULL;
	if( Light->bLightChanged )
		StaticLightingChanged = 1;
	LastLight++;
	return 1;
	unguardSlow;
}

/*------------------------------------------------------------------------------------
	Actor lighting.
------------------------------------------------------------------------------------*/

//
// Compute fast light list for an actor.
// Returns 1 if the actor should be lit, 0 if it should be fake-lit.
//
enum {MaxActorLights=16};
#if defined(LEGEND) // added by Legend 1/31/1999
	extern int GLODActorLights;
#else
	enum {DesiredActorLights=3};
#endif
AActor* Consider[120];
INT NumConsider=0, ConsiderTag=0;
inline void AddConsider( AActor* Actor, AActor* Light, BYTE Factor )
{
	if( Light->ExtraTag!=ConsiderTag )
	{
		// Note that distance reject neglects actor bounding sphere.
		FLOAT DistSquared = FDistSquared(Actor->Location,Light->Location);
		FLOAT Radius      = Light->WorldLightRadius();
		if( Light->bSpecialLit==Actor->bSpecialLit && DistSquared<Square(Radius) )
		{
			Light->LightingTag = appRound((1.0f-SqrtApprox(DistSquared)/Radius) * Light->LightBrightness * 1024);
			Light->ExtraTag    = ConsiderTag;
			Light->SpecialTag  = Factor;
			Consider[NumConsider++] = Light;
		}
	}
}
static INT Compare( const AActor* A1, const AActor* A2 )
{
	return 1 - 2*(A1->LightingTag>A2->LightingTag);
}
DWORD FLightManager::SetupForActor( FSceneNode* InFrame, AActor* InActor, FVolActorLink* LeafLights, FActorLink* Volumetrics )
{
	guard(FLightManager::SetupForActor);
	DWORD Result=0;

	// Init per actor variables.
	Mark      = FMemMark(GMem);
	Frame     = InFrame;
	Level	  = Frame->Level;
	LevelInfo = Level->GetLevelInfo();
	Actor     = InActor;
	Zone	  = Actor->Region.Zone ? Actor->Region.Zone : LevelInfo;
	MapCoords = NULL;
	LastLight = FirstLight;
	Diffuse   = Actor->ScaleGlow * 2.f;
	Specular  = Actor->SpecularGlow * 2.f;

	// The specular component of reflection is squared, which corresponds
	// to a "width" of 2/3, or 170/255. This value is used as the base for
	// width. Lower widths imply a higher min cos value.
	SpecMinCos   = Square(1.f - Actor->SpecularWidth/170.f);
	Specular /= Square(1.f - SpecMinCos);

	if( Diffuse == 1.4f )		// Debug hack old lighting.
		Specular = 6.f;

	// If not zoned, try now.
	if( Actor->Region.ZoneNumber==0 )
		Level->SetActorZone( Actor, 1, 0 );

	// Ambient lighting.
	FLOAT C       = Actor->AmbientGlow!=255 ? Actor->AmbientGlow/255.0f : 0.25f+0.2f*appSin(Frame->Viewport->CurrentTime.GetFloat()*8.f);
	AmbientVector = FVector(C,C,C) + FGetHSV( Zone->AmbientHue, Zone->AmbientSaturation, Zone->AmbientBrightness ) * Diffuse;

	// Reject if the proper data structures aren't in place.
	if
	(	!Actor->bUnlit
	&&	Actor->Region.iLeaf!=INDEX_NONE
	&&	Level->Model->Leaves.Num()
	&&	Frame->Viewport->Actor->RendMap==REN_DynLight 
	&&	!Frame->Viewport->GetOuterUClient()->NoLighting )
	{
		// Get actor light cache.
		INT Delta      = Clamp(appRound((Frame->Viewport->CurrentTime-Frame->Viewport->LastUpdateTime)*768),0,255);
		INT FrameCount = Frame->Viewport->FrameCount;
		INT* Num       = NULL;
		struct FInfo
		{
			AActor* Actor;
			BYTE Factor;
			INT Index;
		} *Senders;
		UBOOL FirstSeeActor = 0;
		QWORD CacheID  = MakeCacheID( CID_ActorLightCache, Actor );
		Senders        = (FInfo*)GCache.Get( CacheID, *TopItemToUnlock++ );
		if( Senders )
		{
			// Look up actors from cache.
			guardSlow(CacheLook);
			Num = (INT*)Senders++;
			INT NewNum = 0;
			for( INT i=0; i<*Num; i++ )
			{
				if( NewNum!=i )
					Senders[NewNum] = Senders[i];
				if( UObject::GetIndexedObject(Senders[i].Index)==Senders[i].Actor && !Senders[i].Actor->bDeleteMe )
					NewNum++;
			}
			*Num = NewNum;
			unguardSlow;
		}
		else
		{
			// Create cache item.
			guardSlow(CacheCreate);
			FirstSeeActor = 1;
			Senders  = (FInfo*)GCache.Create( CacheID, TopItemToUnlock[-1], appCheckedIntSize(static_cast<SIZE_T>(MaxActorLights+1)*sizeof(FInfo)) );
			Num      = (INT*)Senders++;
			*Num     = 0;
			unguardSlow;
		}

		// Make list of all lights to consider.
		NumConsider=0;
		ConsiderTag++;

		// Cached lights.
		guardSlow(ConsiderLights);
		for( INT i=0; i<*Num; i++ )
			if( Senders[i].Actor )
				AddConsider( Actor, Senders[i].Actor, Senders[i].Factor );
		unguardSlow;

		// Static leaf lights.
		guardSlow(StaticLeafLights);
		FLeaf& Leaf = Level->Model->Leaves(Actor->Region.iLeaf);
		if( Leaf.iPermeating!=INDEX_NONE )
			for( INT i=Leaf.iPermeating; Level->Model->Lights(i) && NumConsider<ARRAY_COUNT(Consider); i++ )
				AddConsider( Actor, Level->Model->Lights(i), 0 );
		unguardSlow;

		// Dynamic leaf lights.
		guardSlow(DynamicLeafLights);
		for( FVolActorLink* Link=LeafLights; Link && NumConsider<ARRAY_COUNT(Consider); Link=Link->Next )
			AddConsider( Actor, Link->Actor, 255 );
		unguardSlow;

		// Sort considered lights by priority.
		Sort( Consider, NumConsider );

		// Amortized trace visibility.
		INT NumStaticVisible=0, NumRealVisible=0, Threshold=-1;
		guardSlow(TraceVis);
		for( INT i=0; i<NumConsider; i++ )
		{
			// Determine effective visibility.
			UBOOL IsVisible;
#if defined(LEGEND) // added by Legend 1/31/1999
			if( Consider[i]->LightingTag<Threshold || NumRealVisible>=GLODActorLights )
#else
			if( Consider[i]->LightingTag<Threshold || (Consider[i]->bStatic ? NumStaticVisible : NumRealVisible)>=DesiredActorLights )
#endif
			{
				IsVisible = 0;
			}
			else if( !Consider[i]->bStatic && Consider[i]->bMovable )
			{
				IsVisible = 1;
			}
			else if( FirstSeeActor || ((FrameCount ^ Consider[i]->GetIndex())&15)==0 )
			{
				FCheckResult Hit(0);
				IsVisible = Level->Model->LineCheck( Hit, NULL, Consider[i]->Location, Actor->Location, FVector(0,0,0), 0 );
				if( IsVisible && FirstSeeActor )
					Consider[i]->SpecialTag = 255;
			}
			else
			{
				IsVisible = (Consider[i]->SpecialTag&1);
			}

			// Handle light.
			if( IsVisible )
			{
				// This light is considered visible.
				NumRealVisible++;
				if( Consider[i]->bStatic )
					NumStaticVisible++;
				if( Threshold==-1 )
					Threshold = Consider[i]->LightingTag/8;
				AddLight( Actor, Consider[i] );
				Consider[i]->SpecialTag = Min( (INT)Consider[i]->SpecialTag+Delta, 255 ) | 1;
			}
			else
			{
				// This light is not considered visible.
				Consider[i]->SpecialTag = Max( (INT)Consider[i]->SpecialTag-Delta, 0 ) & ~1;
				if( Consider[i]->SpecialTag>0 )
					AddLight( Actor, Consider[i] );
			}
		}
		unguardSlow;

		// Recache all accepted lights.
		guardSlow(Recache);
		*Num=0;
		for( INT i=0; i<NumConsider && *Num<MaxActorLights; i++ )
		{
			if( Consider[i]->SpecialTag > 1 )
			{
				Senders[*Num].Actor  = Consider[i];
				Senders[*Num].Index  = Consider[i]->GetIndex();
				Senders[*Num].Factor = Consider[i]->SpecialTag;
				++*Num;
			}
		}
		unguardSlow;

		// Volumetric occluding lights.
		guardSlow(Vtrics);
		for( Volumetrics; Volumetrics && LastLight<FinalLight; Volumetrics=Volumetrics->Next )
		{
			Result |= PF_RenderFog;
			FLightInfo* Find = FirstLight;
			for( ; Find<LastLight && Find->Actor!=Volumetrics->Actor; Find++ );
			Find->IsVolumetric = 1;
			if( Find==LastLight )
			{
				// New volumetric light.
				LastLight->Actor = Volumetrics->Actor;
				LastLight->Opt   = ALO_NotLight;
				STAT(GStat.MeshVtricCount++);
				LastLight++;
			}
		}
		unguardSlow;

		// Set up the lights.
		guardSlow(SetupLights);
		LastVtric = FirstVtric;
		for( FLightInfo* Light=FirstLight; Light<LastLight; Light++ )
		{
			Light->ComputeFromActor( NULL, Frame );
			Light->FloatColor *= Light->Actor->SpecialTag/255.0f;
			if( Light->Actor->bDarkLight )
				Light->FloatColor *= -1.f;
			if( Light->IsVolumetric )
				*LastVtric++ = Light;
		}
		if( LastVtric != FirstVtric )
			appQsort( FirstVtric, appCheckedIntSize(static_cast<SIZE_T>(LastVtric-FirstVtric)), sizeof(FLightInfo*), LightInfoDistCompare );
		unguardSlow;
	}
	STAT(GStat.MeshLightCount+=(LastLight-FirstLight));
	return Result;
	unguard;
}

//
// Finish actor lighting.
//
void FLightManager::FinishActor()
{
	guard(FLightManager::FinishActor);

	// Release working memory.
	Mark.Pop();

	// Unlock any locked cache items.
	while( TopItemToUnlock > &ItemsToUnlock[0])
		(*--TopItemToUnlock)->Unlock();

	unguard;
}

void FLightManager::LightParticleSystem( FSceneNode* Frame, AParticleFX* ParticleFX )
{
	ParticleFX->LightColor = FPlane(0, 0, 0, 1);

	for( FLightInfo* Light=FirstLight; Light<LastLight; Light++ )
	{
		FLOAT Intensity = Light->MaxSpecDir | Light->LightDir;
		ParticleFX->LightColor += (Light->ScaledColor * Intensity * Intensity);
	}	
}


/*------------------------------------------------------------------------------------
	Light subsystem instantiation
------------------------------------------------------------------------------------*/

static FLightManager GLightManagerInstance;
FLightManagerBase* GLightManager = &GLightManagerInstance;

/*------------------------------------------------------------------------------------
	The End
------------------------------------------------------------------------------------*/

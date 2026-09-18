/*=============================================================================
	UnStat.h: Profiling stats definition.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

/*------------------------------------------------------------------------------------
	Debugging stats.
------------------------------------------------------------------------------------*/

#ifndef _CORE_STAT_H
#define _CORE_STAT_H

//
// General-purpose statistics:
//

#if STATS

class FClock
{
	INT& Stat;

public:
	FClock( INT& InStat )
	:	Stat(InStat)
	{
		clock(Stat);
	}
	~FClock()
	{
		unclock(Stat);
	}
};

#define Clock(stat)	FClock _Clock(stat)

struct CORE_API FStats
{
	// Misc.
	INT ExtraTime;

	// Totals.
	INT FrameTime, ExternalTime, RenderTime;

	// MeshStats.
	INT MeshTime;
	INT MeshGetFrameTime, MeshProcessTime, MeshLightSetupTime, MeshLightTime, MeshSubTime, MeshClipTime, MeshTmapTime;
	INT MeshCount, MeshPolyCount, MeshVertCount, MeshSubCount, MeshVertLightCount, MeshLightCount, MeshVtricCount;

	// ParticleStats.
	INT ParticleTime;
	INT ParticleEmitTime, ParticleUpdateTime, ParticleRenderTime, ParticleRasterTime;
	INT ParticlesEmitted, ParticlesUpdated, ParticlesSeen, ParticlesRendered;
	INT ParticleArea, ParticleAreaRendered;

	// ActorStats.

	// FilterStats.
	INT DynFilterTime, FilterTime, DynCount;

	// RejectStats.

	// SpanStats.
	INT SpanPix;

	// ZoneStats.

	// OcclusionStats.
	INT OcclusionTime, ClipTime, RasterTime, SpanTime;
	INT NodesDone, NodesTotal;
	INT NumRasterPolys, NumRasterBoxReject;
	INT NumTransform, NumClip;
	INT BoxTime, BoxChecks, BoxBacks, BoxIn, BoxOutOfPyramid, BoxSpanOccluded;
	INT NumPoints;

	// IllumStats.
	INT IllumTime;

	// PolyVStats.
	INT PolyVTime;

	// Actor drawing stats:
	INT NumSprites;			// Number of sprites filtered.
	INT NumChunks;			// Number of final chunks filtered.
	INT NumFinalChunks;		// Number of final chunks.
	INT NumMovingLights;    // Number of moving lights.
	INT ChunksDrawn;		// Chunks drawn.

	// Texture subdivision stats
	INT DynLightActors;		// Number of actors shining dynamic light.

	// Span buffer:
	INT SpanTotalChurn;		// Total spans added.
	INT SpanRejig;			// Number of span index that had to be reallocated during merging.

	// Clipping:
	INT ClipAccept;			// Polygons accepted by clipper.
	INT ClipOutcodeReject;	// Polygons outcode-rejected by clipped.
	INT ClipNil;			// Polygons clipped into oblivion.

	// Memory:
	INT MemTime, StatMemTime;
	INT GMem;				// Bytes used in global memory pool.
	INT GDynMem;			// Bytes used in dynamics memory pool.

	// Zone rendering:
	INT CurZone;			// Current zone the player is in.
	INT NumZones;			// Total zones in world.
	INT VisibleZones;		// Zones actually processed.
	INT MaskRejectZones;	// Zones that were mask rejected.

	// Illumination cache:
	INT PalCycles;			// Time spent in palette regeneration.

	// Lighting:
	INT Lightage,LightMem,MeshPtsGen,MeshesGen,VolLightActors;

	// Textures:
	INT UniqueTextures,UniqueTextureMem,CodePatches;

	// Extra:
	INT Extra1,Extra2,Extra3,Extra4;

	// Decal stats
	INT DecalTime, DecalClipTime, DecalUpdateTime, DecalCount;

	// Routine timings:
	INT GetValidRangeCycles;
	INT BoxIsVisibleCycles;
	INT CopyFromRasterUpdateCycles;
	INT CopyFromRasterCycles;
	INT CopyIndexFromCycles;
	INT MergeWithCycles;
	INT CalcRectFromCycles;
	INT CalcLatticeFromCycles;
	INT GenerateCycles;
	INT CalcLatticeCycles;
	INT RasterSetupCycles;
	INT RasterGenerateCycles;
	INT TransformCycles;
	INT ClipCycles;
	INT AsmCycles;
	// EARI Stats.
	INT TotalEARITime;
	INT EARIActorDrawTime;
	INT TotalEARIActors;
	INT TotalEARISubActors;
	FName EARIActorNames[64];
	FName EARIActorTags[64];
	INT EARISubActors[64];
	INT EARITime[64];
	INT EARIDrawTime[64];
};
extern CORE_API FStats GStat;
#else

#define Clock(stat)

#endif

#endif

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/


/*=============================================================================
	UnStats.h: Performance statistics utilities.
	Copyright 1997-2002 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Andrew Scheidecker
		* Rewritten multiple times by Andrew Scheidecker and Daniel Vogel
		* Last rewrite by Daniel Vogel for .csv support
=============================================================================*/

enum EStatsType
{
	STATSTYPE_Render		= 0,
	STATSTYPE_Game,
	STATSTYPE_Hardware,
	STATSTYPE_Anim,
	STATSTYPE_Matinee,
	STATSTYPE_Audio,
	STATSTYPE_Net,
	STATSTYPE_MAX
};

enum EStatsDataType
{
	STATSDATATYPE_DWORD		= 0,
	STATSDATATYPE_FLOAT,
	STATSDATATYPE_STRING,
	STATSDATATYPE_MAX
};

enum EStatsUnit
{
	STATSUNIT_Default		= 0,
	STATSUNIT_Combined_Default_MSec,
	STATSUNIT_Combined_Default_Default,
	STATSUNIT_Byte,
	STATSUNIT_KByte,
	STATSUNIT_MByte,
	STATSUNIT_MSec
};

class ENGINE_API FStats
{
public:
	// Init Data.
	UBOOL			Registered[STATSTYPE_MAX];

	// Stats Data.
	TArray<DWORD>	DWORDStats;
	TArray<DWORD>	DWORDOldStats;
	TArray<FString>	DWORDStatsDescriptions;
	TArray<FString>	DWORDStatsSectionDescriptions;

	TArray<FLOAT>	FLOATStats;
	TArray<FLOAT>	FLOATOldStats;
	TArray<FString>	FLOATStatsDescriptions;
	TArray<FString>	FLOATStatsSectionDescriptions;

	TArray<FString>	FSTRINGStats;
	TArray<FString> FSTRINGOldStats;
	TArray<FString>	FSTRINGStatsDescriptions;
	TArray<FString>	FSTRINGStatsSectionDescriptions;

	struct FSingleStatsInfo
	{
		INT				Index;
		EStatsDataType	DataType;
		EStatsUnit		StatsUnit;
	};

	TArray<FSingleStatsInfo> Stats[STATSTYPE_MAX];

	struct FMovingAverage
	{		
		INT CycIndex;
		INT CycBufferSize;
		INT Count;
		EStatsDataType	DataType;
		TArray<DWORD>   Samples;
	};

	TArray<FMovingAverage> Averages;

	// Interface.
	FStats();
	INT RegisterStats( EStatsType Type, EStatsDataType DataType, FString Description, FString SectionDescription, EStatsUnit StatsUnit );
	void Render(class UViewport* Viewport, class UEngine* Engine);
	void UpdateString( FString& Result, UBOOL Descriptions );
	void Clear();
	void CalcMovingAverage( INT StatIndex, DWORD CycleSize );
};


class ENGINE_API FEngineStats
{
public:

	INT		STATS_FirstEntry,
		
			STATS_Frame_TotalCycles,
			STATS_Frame_RenderCycles,

			STATS_Karma_CollisionFar,
			STATS_Karma_CollisionNear,
			STATS_Karma_Dynamics,

			STATS_BSP_RenderCycles,
			STATS_BSP_Sections,
			STATS_BSP_Nodes,
			STATS_BSP_Triangles,
			STATS_BSP_DynamicLightingCycles,
			STATS_BSP_DynamicLights,
			STATS_BSP_CollisionCycles,

			STATS_LightMap_Updates,
			STATS_LightMap_Cycles,

			STATS_Projector_RenderCycles,
			STATS_Projector_Projectors,
			STATS_Projector_Triangles,
			
			STATS_Stencil_RenderCycles,
			STATS_Stencil_Nodes,
			STATS_Stencil_Triangles,

			STATS_Visibility_SetupCycles,
			STATS_Visibility_MaskTests,
			STATS_Visibility_MaskRejects,
			STATS_Visibility_BoxTests,
			STATS_Visibility_BoxRejects,
			STATS_Visibility_TraverseCycles,
			STATS_Visibility_ScratchBytes,

			STATS_Terrain_RenderCycles,
			STATS_Terrain_CollisionCycles,
			STATS_Terrain_Sectors,
			STATS_Terrain_Triangles,
			STATS_Terrain_DrawPrimitives,

			STATS_DecoLayer_RenderCycles,
			STATS_DecoLayer_Triangles,
			STATS_DecoLayer_Decorations,

			STATS_Matinee_TickCycles,

			STATS_Mesh_SkinCycles,
			STATS_Mesh_ResultCycles,
			STATS_Mesh_LODCycles,
			STATS_Mesh_SkelCycles,
			STATS_Mesh_PoseCycles,
			STATS_Mesh_RigidCycles,
			STATS_Mesh_DrawCycles,
	
			STATS_Particle_SpriteSetupCycles,
			STATS_Particle_Particles,
			STATS_Particle_RenderCycles,

			STATS_StaticMesh_SortedSections,
			STATS_StaticMesh_SortedTriangles,
			STATS_StaticMesh_SortCycles,
			STATS_StaticMesh_Triangles,
			STATS_StaticMesh_Sections,
			STATS_StaticMesh_CollisionCycles,
			STATS_StaticMesh_RenderCycles,

			STATS_Stats_RenderCycles,

			STATS_Game_ScriptCycles,
			STATS_Game_ActorTickCycles,
			STATS_Game_FindPathCycles,
			STATS_Game_SeePlayerCycles,
			STATS_Game_SpawningCycles,
			STATS_Game_AudioTickCycles,
			STATS_Game_UnusedCycles,
			STATS_Game_NetTickCycles,
			STATS_Game_ParticleTickCycles,
			STATS_Game_CanvasCycles,
			STATS_Game_PhysicsCycles,
			STATS_Game_MoveCycles,
			STATS_Game_NumMoves,
			STATS_Game_MLCheckCycles,
			STATS_Game_MPCheckCycles,
			STATS_Game_UpdateRenderData,

			STATS_Fluid_SimulateCycles,
			STATS_Fluid_VertexGenCycles,
			STATS_Fluid_RenderCycles,

			STATS_Net_Ping,
			STATS_Net_Channels,
			STATS_Net_InUnordered,
			STATS_Net_OutUnordered,
			STATS_Net_InPacketLoss,
			STATS_Net_OutPacketLoss,
			STATS_Net_InPackets,
			STATS_Net_OutPackets,
			STATS_Net_InBunches,
			STATS_Net_OutBunches,
			STATS_Net_InBytes,
			STATS_Net_OutBytes,
			STATS_Net_Speed,
			STATS_Net_NumReps,
			STATS_Net_NumRPC,
			STATS_Net_NumPV,

			STATS_LastEntry;

	FEngineStats();
	void Init();
};

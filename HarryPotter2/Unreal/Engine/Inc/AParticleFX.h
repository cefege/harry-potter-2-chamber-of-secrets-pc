/*=============================================================================
	AParticleFX.h.
	Copyright 1999 DreamWorks Interactive. All Rights Reserved.
=============================================================================*/

	// Constructors.
	AParticleFX();

	// UObject interface.
	virtual void Destroy();
	virtual void InitExecution();
	virtual void PostEditChange();

	// AActor interface.
	virtual UPrimitive* GetPrimitive() const;
	virtual FCoords GetRenderBoundingBox( UBOOL Exact );
	UBOOL Tick( FLOAT DeltaTime, enum ELevelTick TickType );

	// ParticleFX interface.
	void CreateParticleList();
	int EmitParticles(FLOAT Tick = 0.0f);
	void UpdateParticles(FLOAT Tick = 0.0f);
	const AParticleFX* GetSysParams(char* Buffer) const;
	void GetParams(FParams& Params) const;
	bool AddParticle( int Id,  FVector& Position, FParams* Params );
	int RecomputeDeltas(INT Id);

	bool Update(FLOAT Tick = 0.0f);
	FLOAT MaxLifetime();

	FLOAT Lod( float ScreenFrac );
	FLOAT LodParticles( float ScreenFrac );
	FLOAT LodParticleDensity( struct FSceneNode* Frame, float ScreenFrac );
	static FLOAT	ParticleLOD;


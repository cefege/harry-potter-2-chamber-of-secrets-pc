/*=============================================================================
	AWind.h.
	Copyright 1999 DreamWorks Interactive. All Rights Reserved.
=============================================================================*/

	// Actor functions.
	AWind();
	virtual void Destroy();
	virtual UBOOL Tick( FLOAT DeltaTime, enum ELevelTick TickType );

	// Get the wind velocity at a location.
	FVector GetWind( const FVector& Loc );

	// Get the total wind velocity from all actors.
	static FVector GetTotalWind( ULevel* Level, const FVector& Loc );

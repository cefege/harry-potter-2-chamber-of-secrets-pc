/*=============================================================================
	AWind.h.
	Copyright 1999 DreamWorks Interactive. All Rights Reserved.
=============================================================================*/

	// Actor functions.
	AWind();
	virtual void Destroy();
	virtual UBOOL Tick( FLOAT DeltaTime, enum ELevelTick TickType );

	inline FLOAT Radius() const
	{
		return Square(INT(WindRadius));
	}
	inline FLOAT InnerRadiusFrac() const
	{
		return WindRadiusInner / 256.0f;
	}
	inline FLOAT FlucFraction() const
	{
		return WindFluctuation / 255.0f;
	}
	inline FLOAT FlucPeriod() const
	{
		return Max((INT)WindFlucPeriod, 1) / 64.0f;
	}

	// Get the wind velocity at a location.
	FVector GetWind( const FVector& Loc );

	// Get the total wind velocity from all actors.
	static FVector GetTotalWind( ULevel* Level, const FVector& Loc );

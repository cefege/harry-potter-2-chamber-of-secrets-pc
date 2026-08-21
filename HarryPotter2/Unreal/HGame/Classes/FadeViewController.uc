
class FadeViewController extends Actor;

var() bool		bFadeFlash;		// Should our fade flash back to the original color?
var() float		FadeTime;		// How long should it take to get to our target fade color
var() plane		FadeEnd;		// FadeEnd is set by the user (this is our target fade color)

var plane		FadeStart;		// FadeStart is obtained from Harry's region
var float		CurTime;		// Time while fading
var Harry		playerHarry;	// refrence to harry

event BeginPlay()
{
	Super.BeginPlay();
	
	playerHarry = Harry(Level.playerHarryActor);
	
	// We can only have one fadeViewController at a time
	DestroyAllFadeViewControllers();

	FadeStart = playerHarry.ConstantGlowFog;
}

function Init( float A, float R, float G, float B,  float fTime, optional bool bFlash )
{

	// Initilize our properties
	FadeEnd.W = FClamp(A, 0.0, 1.0);
	FadeEnd.X = FClamp(R, 0.0, 1.0);
	FadeEnd.Y = FClamp(G, 0.0, 1.0);
	FadeEnd.Z = FClamp(B, 0.0, 1.0);
	FadeTime  = fTime;
	bFadeFlash= bFlash;

	if( bFadeFlash )
	{
		FadeTime  = FadeTime/2;
		FadeStart = playerHarry.FlashFog;
		GotoState('stateFlash');
	}
	else
	{
		FadeEnd.W = -FadeEnd.W;
		FadeStart = playerHarry.ConstantGlowFog;
	}

	playerHarry.ClientMessage("Fade translated " $"R:" $FadeEnd.X $"G:" $FadeEnd.Y $"B:" $FadeEnd.Z $"A:" $FadeEnd.W );
}

function DestroyAllFadeViewControllers()
{
	local FadeViewController A;
	
	// Because we can only have one fade view controller at a time
	// we need to force any other controller to complete its fade/flash
	// then destroy it.
	foreach AllActors(class'FadeViewController', A )
	{
		if( A != self )
			A.Finish();
	}
}

function Finish()
{
	// empty on purpose
}

auto state stateFade
{
	event Tick( float fTimeDelta )
	{
		local float fSpeedAndTime;
		// update our timer
		CurTime += fTimeDelta;
		if( CurTime < FadeTime )
		{
			fSpeedAndTime = fTimeDelta / FadeTime;
			playerHarry.ConstantGlowFog.X += (FadeEnd.X - FadeStart.X)*( fSpeedAndTime );
			playerHarry.ConstantGlowFog.Y += (FadeEnd.Y - FadeStart.Y)*( fSpeedAndTime );
			playerHarry.ConstantGlowFog.Z += (FadeEnd.Z - FadeStart.Z)*( fSpeedAndTime );
			playerHarry.ConstantGlowFog.W += (FadeEnd.W - FadeStart.W)*( fSpeedAndTime );
		}
		else // we are done
		{
			Finish();
		}
	}
	
	function Finish()
	{
		playerHarry.ConstantGlowFog = FadeEnd;
		
		//DEBUG
/*		playerHarry.ClientMessage("Finishing fade at conGlowFog = "
			$"R" $playerHarry.ConstantGlowFog.X
			$"G" $playerHarry.ConstantGlowFog.Y
			$"B" $playerHarry.ConstantGlowFog.Z
			$"A" $playerHarry.ConstantGlowFog.W
			$"FlashFog = "
			$"R" $playerHarry.FlashFog.X
			$"G" $playerHarry.FlashFog.Y
			$"B" $playerHarry.FlashFog.Z
			$"A" $playerHarry.FlashFog.W );
*/		
		Destroy();
	}
}

state stateFlash
{
	event Tick( float fTimeDelta )
	{
		local float fSpeedAndTime;
		// update our timer
		CurTime += fTimeDelta;
		
		if( CurTime < FadeTime )
		{
			fSpeedAndTime = fTimeDelta / FadeTime;
			playerHarry.FlashFog.X += (FadeEnd.X - FadeStart.X)*( fSpeedAndTime );
			playerHarry.FlashFog.Y += (FadeEnd.Y - FadeStart.Y)*( fSpeedAndTime );
			playerHarry.FlashFog.Z += (FadeEnd.Z - FadeStart.Z)*( fSpeedAndTime );
			playerHarry.FlashFog.W += (FadeEnd.W - FadeStart.W)*( fSpeedAndTime );
		}
		else
		{
			// Set our current fadeColor to our EndFadeColor
			playerHarry.FlashFog = FadeEnd;
			
			if( !bFadeFlash )
				Destroy();
			
			// Because bFlash is true we will reach our destination in half the time
			bFadeFlash= false;
			FadeEnd   = FadeStart;
			FadeStart = playerHarry.FlashFog;
			FadeTime *= 2;
		}
	}
	
	function Finish()
	{
		if( bFadeFlash )
			playerHarry.FlashFog = FadeStart;
		else
			playerHarry.FlashFog = FadeEnd;
		
		//DEBUG
/*		playerHarry.ClientMessage("Finishing fade at conGlowFog = "
			$"R" $playerHarry.ConstantGlowFog.X
			$"G" $playerHarry.ConstantGlowFog.Y
			$"B" $playerHarry.ConstantGlowFog.Z
			$"A" $playerHarry.ConstantGlowFog.W
			$"FlashFog = "
			$"R" $playerHarry.FlashFog.X
			$"G" $playerHarry.FlashFog.Y
			$"B" $playerHarry.FlashFog.Z
			$"A" $playerHarry.FlashFog.W );
*/		
		Destroy();
	}
}

defaultproperties
{
     FadeEnd=(X=1.0,Y=1.0,Z=1.0,W=1.0)
     FadeTime=5.0
	 bHidden=true
}

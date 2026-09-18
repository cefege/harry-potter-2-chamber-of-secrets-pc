
//=============================================================================
// ThunderLightning.
// Edited version of DistanceLightning, by Lode Vandevenne
// Re-Edited version 06/10/02, by Janet Weddle
//=============================================================================

class ThunderLightning expands Light;
 
var int counter;
var() float flashTiming[5];
var bool bLightningActive;
var() name		stormName; // If you want to group these together or with another part of a storm



function BeginPlay()
{
	SetTimer(2+FRand()*6,False);
	LightType = LT_None;
	bLightningActive = false;

}

function Timer()
{
	if (LightType == LT_Flicker)
	{ 
		LightType = LT_None;
		bLightningActive = false;
		SetTimer(flashTiming[counter],False);
		counter++;
		if ( counter >= 5 )
			counter = 0;

//		SetTimer(9+FRand()*20,False); 
	}
	else  
	{
		LightType = LT_Flicker;
		bLightningActive = true;
		PlaySound( sound'HPSounds.Critters_sfx.firecrab_hit', SLOT_none );
		SetTimer(0.8+FRand()*0.5,False);
	}
}



defaultproperties
{
	bStatic=False
	LightBrightness=255
	LightRadius=255
	LightType=LT_Flicker
}


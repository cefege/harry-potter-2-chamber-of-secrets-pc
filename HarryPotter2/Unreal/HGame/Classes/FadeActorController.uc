
class FadeActorController extends Actor;

var() float		TimeEnd;
var() float		FadeEnd;

var	float		FadeStart;
var actor		FadingActor;
var float		TimeCur;		// Time while fading

var Harry		playerHarry;	// refrence to harry

event BeginPlay()
{
	Super.BeginPlay();
	
	playerHarry = Harry(Level.playerHarryActor);
}

function Init( actor A, float fEndOpacity, float fTime )
{
	playerHarry.ClientMessage( "********* Passed endopacity " $fEndOpacity $" time " $fTime );
	// Initilize our properties
	FadingActor = A;
	FadeEnd		= FClamp(fEndOpacity,		  0.0f, 1.0f);
	FadeStart	= FClamp(FadingActor.opacity, 0.0f, 1.0f);
	TimeEnd		= fTime;

	playerHarry.ClientMessage( "********* Going to set actor " $FadingActor $" from " $FadeStart $" to " $FadeEnd );
}

event Tick( float fTimeDelta )
{
	// update our timer
	TimeCur += fTimeDelta;
	
	if( TimeCur < TimeEnd )
	{
		FadingActor.opacity += (FadeEnd - FadeStart) * ( fTimeDelta / TimeEnd );
	}
	else 
	{	
		// we are done
		FadingActor.opacity = FadeEnd;
		
		playerHarry.ClientMessage( "********* Actor " $FadingActor $" from " $FadeStart $" to " $FadeEnd );
		Destroy();
	}
}

defaultproperties
{
	FadeStart=1.0f
	FadeEnd=0.0f
	TimeEnd=5.0f
	bHidden=true
}

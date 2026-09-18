
class FadeViewTrigger extends Triggers;


var() bool		bTriggerOnceOnly;	//

var() bool		bFlash;				// Do you want your fade to flash on then off?
var() float		FadeTime;			// How long should it take to get to our target fade color
var() float		A,R,G,B;			// FadeEnd is set by the user (this is our target fade color)

var FadeViewController	FadeController;

var bool		bTriggered;
var Harry		playerHarry;		// refrence to harry

event BeginPlay()
{
	Super.BeginPlay();
	
	playerHarry = Harry(Level.playerHarryActor);
	
	bTriggered	= False;
	
	Disable('Tick');
}

event Trigger( Actor Other, Pawn EventInstigator )
{
	if( bTriggerOnceOnly && bTriggered )
		return;
	
	bTriggered = true;
	
	// Spawn our FadeController it will do all the work
	FadeController = spawn(class'FadeViewController');
	
	// Convert 0-255 into 0-1
	A = FClamp(A/255, 0.0f, 1.0f);
	R = FClamp(R/255, 0.0f, 1.0f);
	G = FClamp(G/255, 0.0f, 1.0f);
	B = FClamp(B/255, 0.0f, 1.0f);

	FadeController.Init( A,R,G,B, FadeTime, bFlash );
	
	playerHarry.ClientMessage(" FadeStart = " 
		$FadeController.FadeStart.X $" " 
		$FadeController.FadeStart.Y $" " 
		$FadeController.FadeStart.Z $" " 
		$FadeController.FadeStart.W );

	playerHarry.ClientMessage(" FadeEnd = " 
		$FadeController.FadeEnd.X $" " 
		$FadeController.FadeEnd.Y $" " 
		$FadeController.FadeEnd.Z $" " 
		$FadeController.FadeEnd.W );
}

defaultproperties
{
	// FadeViewTrigger
	bFlash=true
	A=255.0f
	R=255.0f
	G=255.0f
	B=255.0f
	FadeTime=0.25f
	
	// Triggers
	bHidden=true
	bStatic=true
}

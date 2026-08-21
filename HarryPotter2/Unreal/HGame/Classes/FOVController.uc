
class FOVController extends Actor;

var() float		FOVTime;		// How long should it take to get to our target FOV
var() float		FOVEnd;			// FOVEnd is set by the user
var float		FOVStart;		// FOVStart is obtained from Harry's region

var float		CurTime;		// Time while fading
var Harry		playerHarry;	// refrence to harry

event BeginPlay()
{
	Super.BeginPlay();
	
	playerHarry = Harry(Level.playerHarryActor);
	
	// We can only have one FOVViewController at a time
	DestroyAllFOVControllers();

	FOVStart  = playerHarry.FOVAngle;
}

function Init( float SetFOVEnd, float SetTime )
{
	// Initilize our properties
	FOVTime   = SetTime;
	FOVEnd	  = SetFOVEnd;
	FOVStart  = playerHarry.FOVAngle;
	
	playerHarry.ClientMessage("FOVController -> FOVStart: " $FOVStart $" FOVEnd: " $FOVEnd $" FOVTime: " $FOVTime );
}

function DestroyAllFOVControllers()
{
	local FOVController A;
	
	// Because we can only have one FOV controller at a time
	// we need to force any other controller to complete its FOV/flash
	// then destroy it.
	foreach AllActors(class'FOVController', A )
	{
		if( A != self )
			A.Finish();
	}
}

function Finish()
{
	playerHarry.DesiredFOV = FOVEnd;		
	playerHarry.ClientMessage("FOVController -> Finish Called, Start: " $FOVStart $" FOVEnd: " $FOVEnd $" FOVTime: " $FOVTime );
	Destroy();
}

auto state stateUpdateFOV
{
	event Tick( float fTimeDelta )
	{
		local float fSpeedAndTime;
		// update our timer
		CurTime += fTimeDelta;
		if( CurTime < FOVTime )
		{
			fSpeedAndTime = FMin(fTimeDelta / FOVTime, 1.0f);
			playerHarry.DesiredFOV += (FOVEnd - FOVStart)*( fSpeedAndTime );
		}
		else // we are done
		{
			Finish();
		}
	}
}


defaultproperties
{
    FOVEnd=90.0f
    FOVTime=2.0f
	bHidden=true
}

//=============================================================================
// Director -- Controller of a mini-game, puzzle, etc.; base class
//=============================================================================
class FlyingFordDirector extends Director;


var FlyingCarHarry	PlayerHarry;
var MuggleMeterManager MuggleMeter;
var FlyingFordPathGuide guide;
var FlyingFordHedwig hedwig;
var Boeing747 plane;
var DynamicInterpolationPoint points[2];
var baseConsole	Console;

var int		SafeRefCount;
var int		TownRefCount;
var float	fDefaultRandomDialog;
var float	fRandomDialog;

var() float fOverTownMeter;
var() float fOverSheepMeter;
var() float fOverPlaneMeter;
var() float fResetMeter;
var() float HedwigMaxDistance;
var() float HedwigPrepivotDistanceFront;
var() float HedwigPrepivotDistanceUp;

// variables used for the wind zone : state gameWind
var float	fWindViolence;
var vector  vDirection;
var float  fTurbulence;
var vector distanceMinusZ;

// variables used for the lightning zone : state gamelightning
var FlyingFordLightning lightningZone;
var ThunderLightning	lightning;
var bool blightningStrike;
var vector strikeDirection;
var vector  tempDistance;

var float	fLightningViolence;
var int iLightningLoops;
var float fTimeBetweenchanges;


// The different locations the car can be in. Will start in safe
var enum CarLocations
{
	LOC_NONE,
	LOC_SAFE,
	LOC_TOWN,

} CarLocation;



//-------------------------------------------------------------------------------------------
// PreBeginPlay()
//-------------------------------------------------------------------------------------------

function PreBeginPlay()
{
	local DynamicInterpolationPoint p;
	local int counter;

	// Initialize
	Super.PreBeginPlay();

	// Find player that channels ClientMessages
	foreach AllActors( class'FlyingCarHarry', PlayerHarry )
		break;

	// Find the muggle meter
	foreach AllActors( class'MuggleMeterManager', MuggleMeter )
		break;

	// Find the plane
	foreach AllActors( class'Boeing747', plane )
		break;

	// Find both dynamicInterpolationPoints
//	foreach AllActors( class'DynamicInterpolationPoint', p )
//	{
//		points[counter] = p;
//		counter++;
//	}


	playerHarry.clientMessage("Make sure I get here");

	// spawn the hidden pawn that will follow the path and tell it what path. 
	guide = spawn(class'FlyingFordPathGuide',,,location+vec(200,50,0), rotation);
	guide.pathName = playerHarry.pathName;
	guide.AirSpeedNormal = playerHarry.AirSpeedNormal;

	log("*************What is the pathname for the car  :  " $playerHarry.pathName);

	// Give this info to the flying car
	playerHarry.guide = guide;

	// spawn FlyingFordHedwig
	hedwig = spawn(class'FlyingFordHedwig',,,location+vec(50,50,-50), rotation);
}

//-------------------------------------------------------------------------------------------
// PostBeginPlay()
//-------------------------------------------------------------------------------------------

function PostBeginPlay()
{

	Super.PostBeginPlay();

	// Start the mini-game with intro CutScene
	InitialState = 'GameIntro';

	// The car will initially be in a safe location
	CarLocation = LOC_SAFE;

	// Set Hedwigs owner to the guide
	hedwig.SetOwner(guide);
	hedwig.AttachToOwner();
	hedwig.bTrailerPrepivot = true;
	hedwig.Prepivot = vec(HedwigPrepivotDistanceFront,0,HedwigPrepivotDistanceUp);

}


// This is outside any state because when the game begins I should get an initial touch event and 
// I don't want to miss it. 
function OnTouchEvent( Pawn Subject, Actor Object )
{
	// Something touched something, and the event affects the flow of the
	// mini-game.  Update game state accordingly.

	if ( (object.Tag == 'FlyingFordSafe') )
	{
		IncrementSafeCount();
	}
	else if ( (object.Tag == 'FlyingFordTown') )
	{
		IncrementTownCount();
	}
	else if ( (object.Tag == 'FlyingFordWind') )
	{
		// wind will send a function call to the director StartTurbulence(violence, direction);
	}
	else if ( (object.Tag == 'FlyingFordWindTrigger') )
	{
		playerHarry.clientMessage("The trigger has been touched");
	}
	else if ( (object.Tag == 'FlyingFordLightning') )
	{
		// You have entered a lightning zone
		lightningZone = FlyingFordLightning(object);
		gotoState('GameLightning');
			
	}
	else
	{
		// Harry touched something unexpected. Unexpected touch event
		// Super.OnTouchEvent( Subject, Object );
	}


	// When something is touched reset the car location
	// If returns true reset what the HUD is doing because the car location has changed
	if ( SetCarLocation() )
	{
//		playerHarry.ClientMessage("Update HUD");
		UpdateHUD();
	}

}

function OnUnTouchEvent( Pawn Subject, Actor Object )
{
	// Something untouched something, and the event affects the flow of the
	// mini-game.  Update game state accordingly.

	if ( (object.Tag == 'FlyingFordSafe') )
	{
		DecrementSafeCount();
	}
	else if ( (object.Tag == 'FlyingFordTown') )
	{
		DecrementTownCount();
	}
	else if ( (object.Tag == 'FlyingFordLightning') )
	{
		// You have left a lightning zone return to normal game state
		// if you are in the state GameLightning. If you are in state
		// StruckByLightning it will return to state GamePlay when complete
		
		if ( IsInState('GameLightning') )
		{
			
			playerHarry.clientMessage("Return to state GamePlay from an UNTouch message  " $GetStateName());
			gotoState('GamePlay');
		}
			
	}
	else
	{
		// Harry untouched something unexpected. Unexpected touch event
		//Super.OnUnTouchEvent( Subject, Object );
	}


	// When something is untouched reset the car location
	// If returns true update what the HUD is doing because the car location has changed
	if ( SetCarLocation() )
	{
//		playerHarry.ClientMessage("Update HUD");
		UpdateHUD();
	}

}


function OnHitEvent( Pawn Subject )
{
	// Something hit part of the world (walls/floors), and the event affects
	// the flow of the mini-game.  Update game state accordingly.

	// This is the default handler for mini-game touch events; the descendant class
	// should override this function to handle all mini-game hit events that need
	// directoring, but it should also call this version for any hit events that
	// the descendant class doesn't know how to handle.

	// Unexpected hit event
	PlayerHarry.ClientMessage( Subject.Name$" hit an obstacle" );
}

function OnCutSceneEvent( Name CutSceneTag )
{
	// Unexpected CutScene event
	PlayerHarry.ClientMessage( "CutScene "$CutSceneTag$" triggered Director" );
}

function OnTriggerEvent( Actor Other, Pawn EventInstigator )
{
	// Unexpected trigger event
	PlayerHarry.ClientMessage( Other$" triggered Director with "$EventInstigator );

	if ( other != None )
	{
		PlayerHarry.ClientMessage("We have triggered an airplane");
		gotoState('GameAirplane');
	}
	else
	{
		// we have trigger with none. That SHOULD mean the game is over
//		gotoState('GameRestart');
	}

}

function Trigger( Actor Other, Pawn EventInstigator )
{
	// Something triggered a 'Director' event.  This could originate from an
	// actor encountering a standard trigger object, or a CutScene executing a
	// Trigger command from its script.

	// For standard trigger objects, the Other parameter will be the actor that
	// activated the trigger, and the EventInstigator will be "Other's" current
	// instigator.  The trigger object itself is not identified.

	// For CutScene triggers, the Other parameter will be the CutScene itself,
	// and the EventInstigator will be None.

	// This function simply splits the possibilities into two handlers.

	// NOTE: if you send a trigger from code 'other' and 'EventInstigator' are
	// both 'None'. 

	local CutScene	CutScene;

	// Added cutscript. This is the type that I receive when I write a script. 
	local CutScript CutScript;

//	PlayerHarry.ClientMessage( Other$" triggered Director with "$EventInstigator );
//	Log( Other$" triggered Director with "$EventInstigator );

	CutScene = CutScene( Other );
	CutScript = CutScript( Other );

	if ( CutScene != None || CutScript != None )
	{
		OnCutSceneEvent( CutScene.Tag );
	}
	else
	{
		OnTriggerEvent( Other, EventInstigator );
	}
}


function OnPlayerPossessed()
{
	// Called when player gets possessed by (attached to) the viewport (Player).
	// This is the first moment when the Player member of PlayerPawn is valid,
	// and thus a reference to the Console.
	Super.OnPlayerPossessed();

	// Get a reference to the console
//	playerHarry.clientMessage("Player Possessed");
	Log( "Player possessed" );
	Console = baseConsole( PlayerHarry.Player.Console );

	TriggerEvent( 'FlyingFordIntro', self, None );
}

function OnPlayerDying()
{
	// Called when player starts dying.

	PlayerHarry.ClientMessage( "Player dying..." );
}

function OnPlayersDeath()
{
	// Called when player dies.

	PlayerHarry.ClientMessage( "Player died; restarting game" );
	Level.Game.RestartGame();
}

function OnActionKeyPressed()
{
	// Called when the player's "Action" key/button is pressed.

	PlayerHarry.ClientMessage( "Action key pressed" );
}

function StartTurbulence(float violence, vector direction)
{
	fWindViolence = violence;
	vDirection = direction;
	gotoState('GameWind');
}

function StartLightning()
{
	local ThunderLightning tempObject;
	local name nameOfStorm;

	playerHarry.clientMessage("StartLightning has been entered");

	// find the corresponding ThunderLightning object
	nameOfStorm = lightningZone.stormName;

	// Look for the ThunderLightning with the same storm name
	foreach AllActors(class'ThunderLightning', tempObject)
	{
		if ( tempObject.stormName == nameOfStorm )
		{
			lightning = tempObject;
		}
	}

	fLightningViolence = lightningZone.fLightningViolence;
	iLightningLoops = lightningZone.iLightningLoops;
	fTimeBetweenchanges = lightningZone.fTimeBetweenchanges;
}


//-------------------------------------------------------------------------------------------
// Helper Functions
//---------------------------------------------------------------------------------------

function IncrementSafeCount()
{
	SafeRefCount++;
	log("Safe Count : " $SafeRefCount);
}

function DecrementSafeCount()
{
	SafeRefCount--;

	if ( SafeRefCount < 0 )
		SafeRefCount = 0;

	log("Safe Count : " $SafeRefCount);

}

function IncrementTownCount()
{
	TownRefCount++;
	log("Town Count : " $TownRefCount);
}

function DecrementTownCount()
{
	TownRefCount--;

	if ( TownRefCount < 0 )
		TownRefCount = 0;

	log("Town Count : " $TownRefCount);

}

// Checks SafeRefCount and TownRefCount to see where the car is and sets the appropriate constant
// Returns true if the location has changed.
function bool SetCarLocation()
{
	local CarLocations currentLocation;

	currentLocation = CarLocation;

	// Check Safe first. That will override any others
	if ( SafeRefCount > 0 )
	{
		CarLocation = LOC_SAFE;
	}
	else if ( TownRefCount > 0 )
	{
		CarLocation = LOC_TOWN;
	}
	else
	{
		CarLocation = LOC_NONE;
	}

	if ( currentLocation == CarLocation )
	{
		return false;
	}
	else
	{
		return true;
	}
}


function UpdateHud()
{

	switch (CarLocation)
	{
	case LOC_SAFE:
//		playerHarry.ClientMessage("Resetting the MuggleMeter");
		log("Resetting the MuggleMeter");
		MuggleMeter.MugglesOutOfRange(fResetMeter);
		break;
	case LOC_TOWN:
//		playerHarry.ClientMessage("MuggleMeter going up by fOverTownMeter");
		log("MuggleMeter going up by fOverTownMeter");
		MuggleMeter.MugglesInRange(fOverTownMeter);
		break;
	case LOC_NONE:
//		playerHarry.ClientMessage("MuggleMeter going up by fOverSheepMeter. Look there's a sheep!");
		log("MuggleMeter going up by fOverSheepMeter. Look there's a sheep!");
		MuggleMeter.MugglesInRange(fOverSheepMeter);
		break;
	default:
//		playerHarry.ClientMessage("We are in an unknown location");
		log("We are in an unknown location");
		break;
	}
}


//***************************************************************************************
//  Game States
//
// GameIntro	-	Playing the intro cut scene
// GamePlay		-	Flying the car 
// GameWon		-	Playing the arrival at Hogworts cut scene
// GameRestart	-	When the user has lost and the game restarts. 
// GameAirplane -	An airplane has been triggered.
// GameWind		-	The car has run into some wind 
// GameLightning-	The car has run into a lightning zone
//
//***************************************************************************************


// The intro part of the game where setup is done and the intro cut scene plays
state GameIntro
{
	// Note: appropriate cut-scene was triggered from OnPlayerPossessed

	function BeginState()
	{

	}

	function OnCutSceneEvent( Name CutSceneTag )
	{
		// Intro CutScene ended; start playing the game

		// Show the MuggleMeter
		MuggleMeter.BeginDetection();

		GotoState( 'GamePlay' );
	}
}


// The interactive part of the game
state GamePlay
{

	begin:

	playerHarry.clientMessage("We are in GamePlay.");

	hedwig.loopAnim('drop');


}



// The user has completed the game and has arrived at Hogworts
state GameWon
{

}


// The user has lost the game. The game will start from the beginning
state GameRestart
{

	begin: 


	// we should be able to reset all of the variables but there are problems with
	// getting multiple touches on objects (like wind) and so I set the bTouch variable
	// to true and leave it. I would need to go to all of these objects and set bTouch
	// back to false. Also would need to make sure I get my initial touch on the first
	// safe zone since it's not being spawned there (it should be okay).

	// For now I'm going to restart the game by restarting the level. Not as nice but 
	// everything will be reset perfectly. 
	OnPlayersDeath();

}

// An airplane has been triggered and will fly near the car. 
state GameAirplane
{
	function BeginState()
	{
		plane.StartTransPath();
//		local DynamicInterpolationPoint pTemp;

//		if ( points[0].Position < points[1].Position )
//		{
//			pTemp = points[0];
//			points[0] =  points[1];
//			points[1] = pTemp;
//		}
	}


	function MovePoints()
	{
		local vector pos1, pos2;
		local vector vCarDirection, vUp, vRight;
		local vector p1Ahead, p1Side;
		local vector p2Ahead, p2Side;

		// for now we will assume:
		//		point1 is 300 units ahead and 200 units right of the car
		//		point2 is 200 units ahead and 0 units right of the car
		//		Height adjustments can be made by the level designer

		vCarDirection = vector(playerHarry.rotation);
		vUp = vec(0,0,1);

		vRight = vCarDirection cross vUp;

		p1Ahead = playerHarry.location + (vCarDirection * 300);
		p1Side  = playerHarry.location + (vRight * 200);

		p2Ahead = playerHarry.location + (vCarDirection * 200);
		p2Side  = playerHarry.location + (vCarDirection * 0);

		points[0].SetLocation(p1Ahead + p1Side);
		points[1].SetLocation(p2Ahead + p2Side);
	}

	function SetPlaneOnPath()
	{
		plane.StartOnPath();
	}

	begin:

//	MovePoints();

//	sleep(4.0);

//	plane.DestroyTransPath();



//	SetPlaneOnPath();

}

state GameWind
{
	function BeginState()
	{
		fTurbulence = 0;
		playerHarry.loopAnim('flyingeratic');
		playerHarry.clientMessage("IN the beginning  :  " $playerHarry.vCurrentTetherDistance);
	}

	function EndState()
	{
		playerHarry.loopAnim('flying');
		playerHarry.clientMessage("IN the end  :  " $playerHarry.vCurrentTetherDistance);
	}

	function float windDirectionConst()
	{
		local vector vRight, vGuideDirection, vUp;

		vGuideDirection = vector(guide.rotation);
		vUp = vec(0,0,1);

		vRight = vGuideDirection cross vUp;

		if ( vDirection dot vRight > 0 )
			return -1;
		else
			return 1;

	}

	function vector windDirectionVector()
	{
		local vector vRight, vGuideDirection, vUp, vLeft;

		vGuideDirection = vector(guide.rotation);
		vUp = vec(0,0,1);

		vRight = vGuideDirection cross vUp;
		vLeft = -vRight;

		if ( vDirection dot vRight > 0 )
		{
			vRight.z = vDirection.z;
			return vRight;
		}
		else
		{
			vLeft.z = vDirection.z;
			return vLeft;
		}

	}


	function Tick(float DeltaTime)
	{
		Super.Tick(DeltaTime);

		if ( fTurbulence < vSize(fWindViolence * vDirection) )
		{
				fTurbulence += vSize(fWindViolence * vDirection) * DeltaTime;
				playerHarry.vTurbulence += fWindViolence * windDirectionVector() * DeltaTime;
		}
		else

		{
			distanceMinusZ = windDirectionVector();
			distanceMinusZ.z = 0;
			playerHarry.sideDistance += vSize(fWindViolence * distanceMinusZ) * windDirectionConst();

			playerHarry.upDistance += playerHarry.vTurbulence.z;
			
			playerHarry.vTurbulence = vec(0,0,0);
			
			gotoState('GamePlay');

		}
	}
		
	begin:

}

state GameLightning
{

	function BeginState()
	{
		StartLightning();
	}

	function Tick(float DeltaTime)
	{
		Super.Tick(DeltaTime);

//		playerHarry.clientMessage("How many ticks do I get in here");

		if ( lightning.bLightningActive == true )
		{
			// You have been hit by lightning
//			playerHarry.clientMessage("You have been HIT by lightning");

			gotoState('StruckByLightning');
		}
	}

	begin:

//	playerHarry.clientMessage("You have entered the lightning zone");

}

state StruckByLightning
{

	function Tick(float DeltaTime)
	{
		Super.Tick(DeltaTime);

		tempDistance = strikeDirection;
		tempDistance.z = 0;
		playerHarry.sideDistance += vSize(fLightningViolence * tempDistance) * sideDirectionConst();
	
		tempDistance = strikeDirection;
		tempDistance.x = 0;
		tempDistance.y = 0;
		playerHarry.upDistance += vSize(fLightningViolence * tempDistance) * upDirectionConst();
				
		playerHarry.vTurbulence = vec(0,0,0);
	}


	function float sideDirectionConst()
	{
		local vector vRight, vGuideDirection, vUp;

		vGuideDirection = vector(guide.rotation);
		vUp = vec(0,0,1);

		vRight = vGuideDirection cross vUp;

		if ( strikeDirection dot vRight > 0 )
			return -1;
		else
			return 1;

	}

	function float upDirectionConst()
	{
		local vector vGuideDirection;

		vGuideDirection = vector(guide.rotation);


		if ( vGuideDirection dot strikeDirection > 0 )
			return -1;
		else
			return 1;

	}

	function vector OutofControl()
	{

		local vector newSideDirection;
		local vector newUpDirection;
		local vector vRight, vGuideDirection, vUp;
		local float fRandPercentSide;
		local vector newVector;

		local float newYaw, newPitch;


		vGuideDirection = vector(guide.rotation);
		vUp = vec(0,0,1);
		vRight = vGuideDirection cross vUp;

		fRandPercentSide = Frand();

		if ( rand(2) == 0 )
		{
			newsideDirection = vRight * (fLightningViolence * fRandPercentSide);
			newYaw = (fLightningViolence * fRandPercentSide*500);
		}
		else
		{
			newsideDirection = -vRight * (fLightningViolence * fRandPercentSide);
			newYaw = -(fLightningViolence * fRandPercentSide*500);
		}

		if ( rand(2) == 0 )
		{
			newUpDirection = vec(0,0,1) * (fLightningViolence * (1-fRandPercentSide));
			
			if (fLightningViolence * (1-fRandPercentSide)*500 < playerHarry.PitchLimitUp)
			{
				newPitch = (fLightningViolence * (1-fRandPercentSide)*500);
			}
			else
			{
				newPitch = playerHarry.PitchLimitUp;
			}

		}
		else
		{
			newUpDirection = vec(0,0,-1) * (fLightningViolence * (1-fRandPercentSide));

			if (fLightningViolence * (1-fRandPercentSide)*500 < playerHarry.PitchLimitDown)
			{
				newPitch = -(fLightningViolence * (1-fRandPercentSide)*500);
			}
			else
			{
				newPitch = playerHarry.PitchLimitDown;
			}
			
		}

		playerHarry.fLightningYaw = newYaw;
		playerHarry.fLightningPitch = newPitch;

//		playerHarry.clientMessage("Lightning Yaw " $playerHarry.fLightningYaw);
//		playerHarry.clientMessage("Lightning Pitch " $playerHarry.fLightningPitch);

		newVector = newsideDirection + newUpDirection;

		return newVector;

	}

	begin:

//	Loop:

	while ( iLightningLoops > 0 )
	{

//		playerHarry.clientMessage("Struck by Lightning! Struck by Lightning!");
		strikeDirection = OutofControl();

		Sleep( fTimeBetweenchanges );

		iLightningLoops--;
	}
		
		
//		playerHarry.clientMessage("REturn to Game Play from StruckByLightning");
		playerHarry.fLightningYaw = 0;
		playerHarry.fLightningPitch = 0;
		gotoState('GamePlay');
		

//	goto 'Loop';

}


defaultproperties
{
	Tag='Director'
	Texture=Texture'Engine.S_Flag'
	DrawScale=3.0
	bHidden=true
	fDefaultRandomDialog=5

	fOverTownMeter=5
	fOverSheepMeter=2
	fOverPlaneMeter=5
	fResetMeter=10

	HedwigMaxDistance=400
	HedwigPrepivotDistanceFront=200
	HedwigPrepivotDistanceUp=100
}

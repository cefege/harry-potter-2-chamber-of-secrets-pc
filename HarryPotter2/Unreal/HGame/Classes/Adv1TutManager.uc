
class Adv1TutManager expands Director;

var()  float   LookAtRonConeAngle;
var()  name    Event_HarrySeesRon;
var()  name    Event_PushForward;
var()  name    Event_PushBackward;
var()  name    Event_PushStrafeLeft;
var()  name    Event_PushStrafeRight;

var    Ron     Ron;
var    Harry   playerHarry;

var    bool    bTriggerReceived;
var    bool    bConditionMet;

//***************************************************************************************
function PostBeginPlay()
{
	ForEach AllActors(class'Ron', Ron)
		break;

	ForEach AllActors(class'Harry', playerHarry)
	{
		playerHarry.Adv1TutManager = self;
		break;
	}
}

//***************************************************************************************
function ForwardPushed()
{
}
function BackwardPushed()
{
}
function StrafeLeftPushed()
{
}
function StrafeRightPushed()
{
}

//***************************************************************************************
auto state stateStart
{
	function Trigger( Actor Other, Pawn EventInstigator )
	{
		playerHarry.ClientMessage("**** Trigger, wait for look at ron");
		GotoState( 'stateWaitForHarryLookAtRon' );
	}
}

//***************************************************
state stateWaitForHarryLookAtRon
{
  Begin:
	do
	{
		if( (normal(Ron.Location - playerHarry.cam.Location) dot vector(playerHarry.cam.Rotation)) > cos(LookAtRonConeAngle*2*3.1416/360) )
		{
			playerHarry.ClientMessage("**** See Ron");
			break;
		}
		sleep(0.1);
	}until(false);

	TriggerEvent( Event_HarrySeesRon, self, playerHarry );
	GotoState( 'stateWaitForHarryMoveForward' );
}

//***************************************************
state stateWaitForHarryMoveForward
{
	function BeginState()
	{
		bTriggerReceived = false;
		bConditionMet = false;
	}

	function Trigger( Actor Other, Pawn EventInstigator )
	{
		playerHarry.ClientMessage("*** waitforharrymoveforward trigger");
		
		bTriggerReceived = true;
		playerHarry.bLockOutForward = false;
	}

	function ForwardPushed()
	{
		playerHarry.ClientMessage("*** pushforward");
		if( bTriggerReceived )
			bConditionMet = true;
	}

  Begin:

	do { sleep(0.1); }  until( bConditionMet );
	Sleep( 0.5 );
	playerHarry.bLockOutForward = true;
	TriggerEvent( Event_PushForward, self, playerHarry );
	GotoState( 'stateWaitForHarryMoveBackward' );
}

//***************************************************
state stateWaitForHarryMoveBackward
{
	function BeginState()
	{
		bTriggerReceived = false;
		bConditionMet = false;
	}

	function Trigger( Actor Other, Pawn EventInstigator )
	{
		bTriggerReceived = true;
		playerHarry.bLockOutBackward = false;
	}

	function BackwardPushed()
	{
		if( bTriggerReceived )
			bConditionMet = true;
	}

  Begin:

	do { sleep(0.1); }  until( bConditionMet );
	Sleep( 0.5 );
	playerHarry.bLockOutBackward = true;
	TriggerEvent( Event_PushBackward, self, playerHarry );
	GotoState( 'stateWaitForHarryStrafeLeft' );
}

//***************************************************
state stateWaitForHarryStrafeLeft
{
	function BeginState()
	{
		bTriggerReceived = false;
		bConditionMet = false;
	}

	function Trigger( Actor Other, Pawn EventInstigator )
	{
		bTriggerReceived = true;
		playerHarry.bLockOutStrafeLeft = false;
	}

	function StrafeLeftPushed()
	{
		if( bTriggerReceived )
			bConditionMet = true;
		else
			playerHarry.ClientMessage("Trigger not received yet");
	}

  Begin:

	do { sleep(0.1); }  until( bConditionMet );
	Sleep( 0.5 );
	playerHarry.bLockOutStrafeLeft = true;
	TriggerEvent( Event_PushStrafeLeft, self, playerHarry );
	GotoState( 'stateWaitForHarryStrafeRight' );
}

//***************************************************
state stateWaitForHarryStrafeRight
{
	function BeginState()
	{
		bTriggerReceived = false;
		bConditionMet = false;
	}

	function Trigger( Actor Other, Pawn EventInstigator )
	{
		bTriggerReceived = true;
		playerHarry.bLockOutStrafeRight = false;
	}

	function StrafeRightPushed()
	{
		if( bTriggerReceived )
			bConditionMet = true;
	}

  Begin:

	do { sleep(0.1); }  until( bConditionMet );
	Sleep( 0.5 );
	playerHarry.bLockOutStrafeRight = true;
	TriggerEvent( Event_PushStrafeRight, self, playerHarry );
	GotoState( 'stateKeepHarryLocked' );
}

//***************************************************
state stateKeepHarryLocked
{
	function Trigger( Actor Other, Pawn EventInstigator )
	{
		playerHarry.bLockOutForward = false;
		playerHarry.bLockOutBackward = false;
		playerHarry.bLockOutStrafeLeft = false;
		playerHarry.bLockOutStrafeRight = false;

		GotoState( 'stateDone1' );
	}
}

//***************************************************
state stateDone1
{
}

//***************************************************************************************
defaultproperties
{
	LookAtRonConeAngle=10
}
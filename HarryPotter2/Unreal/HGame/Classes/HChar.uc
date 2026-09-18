
class HChar expands HPawn;

// Used if HChar is an enemy that can display the enemy healthbar.
// Specifies which health bar to use.
enum EEnemyBar
{
	EnemyBar_Aragog,		// Fighting Aragog
	EnemyBar_Basilisk,      // Fighting Basilisk
	EnemyBar_Duellist,      // Duelling another wizard
    EnemyBar_Peeves,        // Fighting Peeves
    EnemyBar_Seeker,        // Quidditch seeker
    EnemyBar_None
};

var(EnemyHealth) EEnemyBar EnemyHealthBar;

var              name    SavedState;

//var(Movement)  float   GroundSpeed; //this is in Pawn.uc, and is your walk speed
var(Movement)    float   RunSpeed;    //this is your run speed

// --- Flip Push support
var(SpellEffects)	bool  bFlipPushable;			// if you flip this Char is he pushed
var(SpellEffects)	float fFlipPushForceXY;	// scale x,y value of vector
var(SpellEffects)	float fFlipPushForceZ;	// scale z value of vector

// --- Pickup actor support
var					actor aHolding;			// actor that we are holding

// generic fidget / idle support
var	name	CurrFidgetAnimName;
var	name	CurrIdleAnimName;
var int		FidgetNums;
var int		IdleNums;

var (Fidgets)	int  iMinIdleSeconds;
var (Fidgets)	int  iMaxIdleSeconds;

var (BumpLines) bool bUseBumpLine;
var (BumpLines) bool bBumpCaptureHarry;
var (BumpLines) string BumpLineSet;
var (BumpLines) string BumpLineSetPrefix;
var int curBumpLine;
var name SavedPreBumpState;
var rotator SavedPreBumpRot;
var float   LastBumpTime;

// If you want your HPawn to play a fall sound when falling, fill the soundFalling array with
// as many sounds that you want to randomly pick from.  If just soundFalling[0] is filled in,
// then that's the sound that will always play when the pawn falls. The array must be filled in
// contiguously.  For example, if soundFalling[0], soundFalling[1] and soundFalling[3] are setup,
// only soundFalling[0] or soundFalling[1] will be played.  Set fFallSoundDist to the 
// minimum fall distance that should cause a fall sound to play.
var   Sound soundFalling[4];     // A sound from this array will be randomly played when pawn falls
var() float fFallSoundDist;      // If fall distance is greater than this, fall sound plays
var   Sound soundCurrFalling;    // Fall sound currently playing

// --- Watch for Harry support

const	WATCH_FOR_HARRY_ARRAY_SIZE = 3;

var(WatchForHarry)	bool  bCouldWatchForHarry;	// if you are able to watch for Harry

var					int   HowManyBaseAnims;
var					int   HowManyBaseSounds;

var					int   HowManyAlarmAnims;
var					int   HowManyAlarmSounds;

var					float fCurrTime;
var					float fDuration;

var					vector	vTemp;

var(WatchForHarry)	name  BaseWatchAnim[3];	  	// which animation to play, if see Harry first time
var(WatchForHarry)	string BaseWatchSound[3];	// which sound to play, if see Harry first time

var(WatchForHarry)	name  BaseAlarmAnim[3];		// which animation to play, if see Harry second time
var(WatchForHarry)	string BaseAlarmSound[3];	// which sound to play, if see Harry second time

var(WatchForHarry)	float fWatchForHarryDist;	// max distance to watch
var(WatchForHarry)	float fCutSceneTime;		// time for playing cut scene
var(WatchForHarry)  float fNotifyOthersHearDistance; // Hearing range for other kids when this kids yells out.

var(WatchForHarry) name	  EventName;

var                 actor aListenToMe;

//-----------------------------------------------------------------------------------------------------------------------------------------------------
//-----------------------------------------------------------------------------------------------------------------------------------------------------
//-----------------------------------------------------------------------------------------------------------------------------------------------------
//-----------------------------------------------------------------------------------------------------------------------------------------------------

//------------------------------------------------------------------------------------------------------------

// SomeOtherHChar.NotifyOthersOfHarry() calls this to determine if this HChar should gotostate "ChaseHarry"
function bool ShouldStartLookingForHarry()
{
	return true;
}

function bool IsHuntingHarry()
{
	return false;
}

function bool CanSeeHarry(optional bool bLookingForHarry)
{
	local vector v;

	// Harry is a Goyle, they do not complain
	if(playerHarry.bIsGoyle)
		return false;

	// Harry is too far (only if you're not actively looking for harry.)
	if( !bLookingForHarry )
	{
		v = playerHarry.location - location;
		if(vsize(v) > fWatchForHarryDist)
			return false;
	}

	// there are some obsticles, and do 180 degree field of view
	if( ((playerHarry.Location - Location)*vect(1,1,0)) dot vector(rotation) < 0 )
		if( !LineOfSightTo(playerHarry) )
			return false;

	return true;
}

function float PlayRandomSoundAndAnimFirstTime()
{
	local int randNumber;
	local float duration;

	if(HowManyBaseSounds > 0)
	{
		randNumber = Rand(HowManyBaseSounds);
		duration = DeliverLocalizedDialog(BaseWatchSound[randNumber], true, 0);
	}
	else
		duration = 0.01;

	if(HowManyBaseAnims == 0)
		PlayAnim('Idle');
	else
	{
		randNumber = Rand(HowManyBaseAnims);
		PlayAnim( BaseWatchAnim[randNumber]);
	}

	return duration;
}	

function float PlayRandomSoundAndAnimSecondTime()
{
	local int randNumber;
	local float duration;

	if(HowManyAlarmAnims > 0)
	{
		if(HowManyAlarmSounds > 0)
		{
			randNumber = Rand(HowManyAlarmSounds);
			duration = DeliverLocalizedDialog(BaseAlarmSound[randNumber], true, 0);
		}
		else
			duration = 0.01;

		randNumber = Rand(HowManyAlarmAnims);
		PlayAnim( BaseAlarmAnim[randNumber]);
	}
	else
		duration = 0.01;

	return duration;
}	

//*******************************************************************************************
state StartFollowingHarry
{
	function bool ShouldStartLookingForHarry() { return false; }
	function bool IsHuntingHarry() { return true; }

	function tick(float deltaTime)
	{
		Global.Tick( deltaTime );
		DesiredRotation.Yaw = rotator(aListenToMe.Location - Location).Yaw;
	}
  Begin:
	Sleep( RandRange(1,2) );
	GotoState( 'followHarry' );
}

//*******************************************************************************************
state followHarry
{
	function bool ShouldStartLookingForHarry() { return false; }
	function bool IsHuntingHarry() { return true; }

  Begin:

	GroundSpeed = GroundRunSpeed;
	loopAnim( RunAnimName, , 0.75 );

	// move a bit in Harry's direction
	vTemp = playerHarry.location - location;
	vTemp = location + 20 * vTemp / vsize(vTemp);

	//TurnTo(vTemp);
	MoveTo(vTemp);
	desiredRotation.Yaw = Rotation.Yaw;

	if( CanSeeHarry( true ) )
		gotostate('followHarry');
	else
		GotoState('RandomLookForHarry');
}

//*******************************************************************************************
//Work on this!!!!!
state RandomLookForHarry
{
	function bool ShouldStartLookingForHarry() { return false; }
	function bool IsHuntingHarry() { return true; }

  Begin:

	GroundSpeed = GroundWalkSpeed;
	loopAnim( WalkAnimName, , 0.75 );

	vTemp = Location  +  normal(VRand()*vect(1,1,0)) * 100;

	//TurnTo(vTemp);
	MoveTo(vTemp);
	desiredRotation.Yaw = Rotation.Yaw + Rand(65536);
	Sleep( RandRange( 0.75, 1.5 ) );

	if( CanSeeHarry( true ) )
		gotostate('followHarry');
	else
		GotoState('RandomLookForHarry');
}

//*******************************************************************************************
state CaughtHarry
{
	function bool ShouldStartLookingForHarry() { return false; }
	function bool IsHuntingHarry() { return true; }
//  Begin:
//	Sleep(fCutSceneTime);
}

//*******************************************************************************************
state SaySomethingFirstTime
{
	function bool ShouldStartLookingForHarry() { return false; }
	function bool IsHuntingHarry() { return true; }

	function BeginState()
	{
		// Stop moving, if was moving
		Acceleration = vect(0,0,0);
		Velocity	 = vect(0,0,0);
	}

	function tick(float deltaTime)
	{
		Global.Tick( deltaTime );
		DesiredRotation.Yaw = rotator(playerHarry.Location - Location).yaw;
cm(name$" Tick 1");
	}

  begin:

	// turn to harry
	//TurnTo(playerHarry.location);

	playerHarry.clientMessage("Start to say something first time................." $self);
	fDuration =	PlayRandomSoundAndAnimFirstTime();
	Sleep( fDuration*0.2 );
	NotifyOthersOfHarry();
	Sleep( fDuration*0.8 );
	FinishAnim();
	Sleep( 0.25 );
	playerHarry.clientMessage("End   to say something first time................." $self);


	//if( CanSeeHarry() )
	//{
	//	loopAnim(WalkAnimName, , 0.75);
		gotoState('followHarry');
	//}
	//else
	//	RestoreState();
}

//*******************************************************************************************
function NotifyOthersOfHarry()
{
	local HChar   a;

	ForEach AllActors(class'HChar', a)
	{
		if(    a.bCouldWatchForHarry
		   &&  VSize(playerHarry.Location - Location) < fNotifyOthersHearDistance
		   && !a.ShouldStartLookingForHarry()
		  )
		{
			aListenToMe = self;
			a.GotoState( 'StartFollowingHarry' );
		}
	}
}

//*******************************************************************************************
/*
state SaySomethingSecondTime
{
	function BeginState()
	{
		// Stop moving, if was moving
		Acceleration = vect(0,0,0);
		Velocity	 = vect(0,0,0);
	}

	begin:

	// turn to harry
	TurnTo(playerHarry.location);

	playerHarry.clientMessage("Start to say something second time................" $self);
	fDuration =	PlayRandomSoundAndAnimSecondtime();
	Sleep(fDuration);
	FinishAnim();
	playerHarry.clientMessage("End   to say something second time................" $self);

	//  send event CallSnape
	if(EventName != 'none')
	{
		playerHarry.clientMessage("Trigger Event................" $EventName);
		TriggerEvent(EventName, none, none );
	}

	// wait till cutscene is over
	Sleep(fCutSceneTime);

	RestoreState();
}
*/

function Tick(float deltaTime)
{
	super.Tick(deltaTime);

	// If have falling sounds setup for this character
	if (soundFalling[0] != None)
		HandleFallSounds();

	// if are not watching for Harry at all, do nothing
	if(!bCouldWatchForHarry)
		return;

	// looking for Harry every 1 second
	fCurrTime += DeltaTime;
	if(fCurrTime < 1.0)
		return;

	fCurrTime = 0.0f;

	if( !IsHuntingHarry()  &&  CanSeeHarry( false ) )
	{
		SaveState();
		gotoState('saySomethingFirstTime');
		return;
	}
}

function PreBeginPlay()
{
	local string animName;
	local int	i;
	local name	nm;

	Super.PreBeginPlay();

	FidgetNums = 0;
	for ( i = 1; i <= 16; i++)
	{
		animName = "fidget_" $i;
		nm = StringToAnimName(animName);
		if(nm == '')
		{
			FidgetNums = i - 1;
			break;
		}
	}

	IdleNums = 0;
	for ( i = 1; i <= 16; i++)
	{
		animName = "idle_" $i;
		nm = StringToAnimName(animName);
		if(nm == '')
		{
			IdleNums = i - 1;
			break;
		}
	}

	HowManyBaseAnims  = 0;
	HowManyBaseSounds = 0;

	HowManyAlarmAnims  = 0;
	HowManyAlarmSounds = 0;

	// if are not waching for Harry, do nothing
	if(!bCouldWatchForHarry)
		return;

	for( i = 0; i < WATCH_FOR_HARRY_ARRAY_SIZE; i++)
	{
		if( BaseWatchAnim[i] == '')
			break;
	}

	HowManyBaseAnims = i;

	for( i = 0; i < WATCH_FOR_HARRY_ARRAY_SIZE; i++)
	{
		if( BaseWatchSound[i] == "")
			break;
	}

	HowManyBaseSounds = i;

	for( i = 0; i < WATCH_FOR_HARRY_ARRAY_SIZE; i++)
	{
		if( BaseAlarmAnim[i] == '')
			break;
	}

	HowManyAlarmAnims = i;

	for( i = 0; i < WATCH_FOR_HARRY_ARRAY_SIZE; i++)
	{
		if( BaseAlarmSound[i] == "")
			break;
	}

	HowManyAlarmSounds = i;
}

function name GetCurrFidgetAnimName()
{
	local string animName;
	local int	index;
	local name	nm;

	if(FidgetNums == 0)
		return IdleAnimName;
	
	index = 1 + Rand(FidgetNums);
	animName = "fidget_" $index;
	nm = StringToAnimName(animName);

	return nm;
}

function name GetCurrIdleAnimName()
{
	local string animName;
	local int	index;
	local name	nm;
	
	if(IdleNums == 0)
		return IdleAnimName;
	
	index = Rand(IdleNums + 1);

	if(index == 0)
		animName = "idle";
	else
		animName = "idle_" $index;

	nm = StringToAnimName(animName);

	return nm;
}

//************************************
//**** OBJECT MANIPULATION SUPPORT ***
//************************************

//function Fire( optional float F )
//{
//	//Probably should have some generic cast code here.
//}

event Bump(actor other)
{
	local HChar  a;
	local bool   bDoBump;

	if( other.IsA('harry') && bCouldWatchForHarry && !playerHarry.bIsGoyle )
	{
		//if( !IsInState('saySomethingSecondTime') )
		//	gotoState('SaySomethingSecondTime');

		//  send event CallSnape
		if(EventName != 'none')
		{
			playerHarry.clientMessage("Trigger Event................" $EventName);
			TriggerEvent(EventName, none, none );
			GotoState( 'CaughtHarry' );
		}
	
		return;	//dont do super.bump ???
	}

	if(bUseBumpLine && other==level.PlayerHarryActor)
	{
		//Make sure no other kid is currently doing a bump line
		bDoBump = true;
		ForEach AllActors(class'HChar', a)
			if( a.IsInState('DoingBumpLine') )
				{ bDoBump = false;     break; }
		if( bDoBump  &&  Level.TimeSeconds - LastBumpTime > 0.75)
			DoBumpLine();
		return;	//dont do super.bump ???
	}

	super.bump(other);
}

//******************************************************************************************************************
function DoBumpLine(optional bool bJustTalk, optional string AlternateBumpLineSet)
{
local string sSetID;
local string sSayTextID;
local string sSayText;

local sound dlgSound;
local float sndLen;
local TimedCue tcue;

	//If someones doing a "Just talk", then ignore the bUseBumpLine return
	if(!bUseBumpLine && !bJustTalk)
		return;

	if(CutNotifyActor!=None && !bJustTalk)		//already captured.
		return;

		//debug: Force set to default.
	//	BumpLineSet="BumpLineTest";

	if(BumpLineSet=="" && AlternateBumpLineSet=="")
		{
		level.playerHarryActor.ClientMessage("ERROR BUMPLINES:"$self $" has no BumpLineSet");
		return;
		}

	//If an alternate bumplineset has been supplied, use it.
	if( AlternateBumpLineSet != "" )
	{
		sSetID = AlternateBumpLineSet;
		level.playerHarryActor.ClientMessage("BUMPLINES:"$self $" looking for BumpLineSet:"$sSetID);
		sSayTextID = Localize( sSetID, "line"$Rand( int(Localize( sSetID, "Count","BumpSet" )) ),"BumpSet" );
	}
	else //otherwise use normal bump set string
	{
		if(BumpLineSetPrefix!="")
			sSetID=BumpLineSetPrefix$ "_" $BumpLineSet;
		else
			sSetID=BumpLineSet;

		level.playerHarryActor.ClientMessage("BUMPLINES:"$self $" looking for BumpLineSet:"$BumpLineSet);

		sSayTextID=Localize( sSetID, "line"$curBumpLine,"BumpSet" );
		curBumpLine++;

		//see if we went past the last number.
		if( instr(sSayTextID,"<") >-1)
			{
			curBumpLine=0;
			sSayTextID=Localize( sSetID, "line"$curBumpLine,"BumpSet" );
			}
			//if still no text then there is a problem.
		if( instr(sSayTextID,"<") >-1)
		{
			level.playerHarryActor.ClientMessage("ERROR BUMPLINES:"$self $" couldn't find BumpLineSet:"$BumpLineSet);
			return;
		}
	}

	level.playerHarryActor.ClientMessage("BUMPLINES:"$self $" looking for BumpLine ID:"$sSayTextID);

	sSayText=Localize( "all", sSayTextID,"BumpDialog" );

	//if still no text then there is a problem.
	if( instr(sSayText,"<?") > -1)
	{
		level.playerHarryActor.ClientMessage("ERROR BUMPLINES:"$self $" couldn't find BumpLine ID:"$sSayTextID $" from BumpLineSet:" $BumpLineSet);
		return;
	}

	SavedPreBumpState=GetStateName();
	SavedPreBumpRot=rotation;

//		if(true)
	if(bBumpCaptureHarry && level.playerHarryActor.CutNotifyActor!=None)
		{
		level.playerHarryActor.CutNotifyActor=Self;
		level.playerHarryActor.CutCommand("capture");
		}

	CutNotifyActor=self;

	//if( !bJustTalk )
	//	CutCommand("TurnTo harry","_BumpLineCue");

		//get Sound
	dlgSound = Sound( DynamicLoadObject("AllDialog."$sSayTextID, class'Sound') );
	if(dlgSound!=None)
		{
		sndLen=GetSoundDuration(dlgSound);
		PlaySound(dlgSound, , , , 100000, , true);
		}
	else
		{
			//make up a duration if no sound
		sndLen=(Len(sSayText)*0.01)+3.0;
		}

	//2 Should handle facial expression here **************
	sSayText = HandleFacialExpression( sSayText, sndLen );
	
	if( !bJustTalk )
	{
		//create a TimedCue to cue object after sndLen seconds.
		tcue=spawn(class 'TimedCue');
		tcue.CutNotifyActor=Self;		//Tell me when done. This is auto passed back to the CutNotifyActor if any.
										//Or it can be used by the talk to find out when the talk is finished.
		tcue.SetupTimer(sndLen+0.5,"_BumpLineCue"); //little extra time for slop
	}

		//show text
	level.playerHarryActor.MyHud.SetSubtitleText(sSayText, sndLen);


//	CutCommand("SAY " $sSayText,"_BumpLineCue");
	if( !bJustTalk )
		GotoState('DoingBumpLine');
}

state DoingBumpLine
{
	function BeginState()
	{
		Acceleration = vect(0,0,0);
		Velocity = vect(0,0,0);
		PlayAnim('idle', 1.0, 0.5);
	}

	event Bump(actor other)
		{
		super.bump(other);
		}

	function CutCue(string cue)
		{
//level.playerHarryActor.ClientMessage("HereThereAndEveryWhere");
		if(bBumpCaptureHarry)
			{
			level.playerHarryActor.CutCommand("release");
			level.playerHarryActor.CutNotifyActor=None;
			}

		CutNotifyActor=None;
		GotoState(SavedPreBumpState);
		DesiredRotation=SavedPreBumpRot;
		LastBumpTime = Level.TimeSeconds;
		}

  Begin:
	TurnTo( LocationSameZ( playerHarry.Location ) );
	Goto 'Begin';
}

function bool ObjectPickup( actor obj, name nHoldingBone )
{
	// error checking
	if( obj.Owner != none )
	{
		playerHarry.ClientMessage(" ERROR when " $Name $" is trying to pickup an object!" );
		return false; // failure ( someone else owns this object! )
	}
	
	// save our object as the object we are holding
	aHolding = obj;
	
	// affect our holding object so that we are now carrying it.
	aHolding.SetOwner( self );
	aHolding.AttachToOwner( nHoldingBone );
	
	// make sure there is no collision now that we are carrying this object
	aHolding.SetCollision( false, false, false );
	
	// Play pickup sound
	PlaySound(sound'HPSounds.magic_sfx.pickup11');
	
	return true; // success
}

function ObjectThrow( vector vThrow, bool bCollideActors, bool bCollideWorld )
{
	// error checking
	if( aHolding == None )
	{
		playerHarry.ClientMessage(" ERROR when " $Name $" is trying to throw an object!" );
		return; // failure
	}
	
	// set up the actor for throwing
	aHolding.SetPhysics(PHYS_Falling);
	aHolding.AnimBone		= 0;		// <- detach ourself from any holdingBone
	//olding.bCollideActors = bCollideActors;
	//Only mess with bCollideActors
	aHolding.SetCollision( bCollideActors );
	aHolding.bCollideWorld  = bCollideWorld;
	
	// simple throw the jellybean at harray
	aHolding.Velocity		= vThrow;
	
	// clear our holding actor var
	aHolding.SetOwner( None );
	aHolding	   = None;
}

//*****************************
//**** FLIP PUSHING SUPPORT ***
//*****************************
function bool HandleSpellFlipendo( optional baseSpell spell, optional vector vHitLocation )
{
	Super.HandleSpellFlipendo( spell, vHitLocation);
	
	if( bFlipPushable == true )
	{
		// Set our physics to falling
		SetPhysics(PHYS_Falling);
		
		// Set our velocity to be away from harry
		Velocity   = normal(location - playerHarry.location) * fFlipPushForceXY;
//		Velocity   = normal(location - vHitLocation) * fFlipPushForceXY;
		Velocity.z = fFlipPushForceZ;
	}
	return true;
}

function Landed( vector HitNormal )
{
	Super.Landed( HitNormal );
ClientMessage("HChar Landed");	
	if( bFlipPushable == true )
	{
		// When we land from the FlipSpell we need to set our Physics to Walking again
		SetPhysics(PHYS_Walking);

		// Make sure we are not going to move
		Acceleration = vect(0,0,0);
		Velocity	 = vect(0,0,0);
	}
}


//***********************************************************************************************************
//***********************************************************************************************************
//When these actions are done, they call the generic method OnEvent( 'ActionDone' );
function DoPickup(actor a)
{
}

//***********************************************************************************************************
//When these actions are done, they call the generic method OnEvent( 'ActionDone' );
function DoAttack(name AttackType)
{
}

//***********************************************************************************************************
//override this and make your HChar do more stuff.  When you're done, call OnEvent( 'ActionDone' );
function DoAction(name action)
{
}

//***********************************************************************************************************
//***********************************************************************************************************
//controller functions

//***********************************************************************************************************
function DoPossess()
{
}

//***********************************************************************************************************
function DoUnPossess()
{
}

//***********************************************************************************************************
function CanAttack()
{
}

//***********************************************************************************************************
//***********************************************************************************************************
//senses

//***********************************************************************************************************
function OnTouch()
{
}


//***********************************************************************************************************
function OnEvent(name EventName)
{
	if( EventName == 'ActionDone' )
	{
	}
}

//***********************************************************************************************************
//command is like "MoveTo Target Arg1 Arg2 ....."
//cue is the cue to send when the action is complete. Save this somewhere for when the command is finished.
//bFastFlag is for the fastforward stuff. Its not hooked up to anything yet tho.
function bool CutCommand(string command, optional string cue, optional bool bFastFlag)
{
	local string  sActualCommand;
	local string  sCutName;
	local actor   a;

//	playerHarry.ClientMessage(self $ " Got a Command:"$command);
	sActualCommand = ParseDelimitedString( command, " ", 1, false );

	  if( sActualCommand ~= "Set" )
		{
			return CutCommand_HandleSet( command, cue, bFastFlag );
		}

	return  super.CutCommand(command, cue, bFastFlag);
}

//*************************************************************************************************************************
//*************************************************************************************************************************
//*************************************************************************************************************************

function bool CutCommand_HandleSet(string command, optional string cue, optional bool bFastFlag)
{
	local actor        a;
	local string       sVarName,sVarValue;
	local int          i;

	sVarName = ParseDelimitedString( command, " ", 2, false );
	sVarValue = ParseDelimitedString( command, " ", 3, false );

	sVarName=caps(sVarName);

	switch(sVarName)
	{
	case "BUMPSET":
		cm(self $" Setting BumpLineSet to:" $sVarValue);
		BumpLineSet=sVarValue;
		break;
	case "BUMPPREFIX":
		cm(self $" Setting BumpLineSetPrefix to:" $sVarValue);
		BumpLineSetPrefix=sVarValue;
		break;
	}

	CutCue(cue);	//immediate finish
	return true;
}

//*************************************************************************************************************************
//*************************************************************************************************************************

auto state patrol
{
}

state stateIdle
{
  Begin:

	CurrFidgetAnimName	= GetCurrFidgetAnimName();
	CurrIdleAnimName	= GetCurrIdleAnimName();

	// we have Idle animation all the time (at least the 'Idle' one).
	if(FidgetNums != 0)
	{
		LoopAnim( CurrIdleAnimName, [TweenTime]0.5);
		Sleep( RandRange(iMinIdleSeconds, iMaxIdleSeconds) );
		FinishAnim();

		PlayAnim( CurrFidgetAnimName, [TweenTime]0.2);
		FinishAnim();
	}
	else
	{
		if(HasAnim(CurrIdleAnimName))
		{
			PlayAnim( CurrIdleAnimName );
			FinishAnim();
			Sleep(0.01);	  // just in case
		}
		else
			Sleep(0.1);
	}

	Goto 'Begin';
}

/*
state stateMovingToLoc
{
  Begin:
	LoopAnim( MoveToAnimSequence, 1.0, 0.2 );
	MoveTo( vMoveToLoc );
	Velocity = vect(0,0,0);
	Acceleration = vect(0,0,0);
	DesiredRotation.Yaw = Rotation.Yaw;
	if( bSnapToLocIfYouDontMakeIt  &&  VSize2d(Location-vMoveToLoc) > 16 )
		SetLocation2( vMoveToLoc );
	LoopAnim( IdleAnimName, 1.0, 0.2 );
	OnEvent( 'ActionDone' );
	DoCutCueNotify();
}
*/
//*************************************************************************************************************************
//*************************************************************************************************************************
//*************************************************************************************************************************

//***********************************************************************************************************
function SaveState()
{
	SavedState = GetStateName();
}

//***********************************************************************************************************
function RestoreState()
{
	GotoState( SavedState );
}
 
//***********************************************************************************************************
simulated function PlayFootStep()
{
	local sound step;
	local float decision;

	local Texture HitTexture;
	local int Flags;
	local sound Footstep1;
	local sound Footstep2;
	local sound Footstep3;

return;

	if ( FootRegion.Zone.bWaterZone )
	{
		PlaySound(WaterStep, SLOT_Interact, 1, false, 1000.0, 1.0);
		return;
	}

	HitTexture = TraceTexture(Location + (vect(0,0,-128)), Location, Flags );

	Footstep1 = Sound'HPSounds.FootSteps.HAR_foot_stone1';
	Footstep2 = Sound'HPSounds.FootSteps.HAR_foot_stone2';
	Footstep3 = Sound'HPSounds.FootSteps.HAR_foot_stone3';

	switch( HitTexture.FootstepSound )
	{
		case FOOTSTEP_Wood:
			Footstep1 = Sound'HPSounds.FootSteps.HAR_foot_wood1';
			Footstep2 = Sound'HPSounds.FootSteps.HAR_foot_wood2';
			Footstep3 = Sound'HPSounds.FootSteps.HAR_foot_wood3';
			break;

		case FOOTSTEP_Rug:
			Footstep1 = Sound'HPSounds.FootSteps.HAR_foot_rug1';
			Footstep2 = Sound'HPSounds.FootSteps.HAR_foot_rug2';
			Footstep3 = Sound'HPSounds.FootSteps.HAR_foot_rug3';
			break;

		case FOOTSTEP_Stone:
			Footstep1 = Sound'HPSounds.FootSteps.HAR_foot_stone1';
			Footstep2 = Sound'HPSounds.FootSteps.HAR_foot_stone2';
			Footstep3 = Sound'HPSounds.FootSteps.HAR_foot_stone3';
			break;

		case FOOTSTEP_cave:
			Footstep1 = Sound'HPSounds.FootSteps.HAR_foot_cave1';
			Footstep2 = Sound'HPSounds.FootSteps.HAR_foot_cave2';
			Footstep3 = Sound'HPSounds.FootSteps.HAR_foot_cave3';
			break;

		case FOOTSTEP_cloud:
			Footstep1 = Sound'HPSounds.FootSteps.HAR_foot_cloud1';
			Footstep2 = Sound'HPSounds.FootSteps.HAR_foot_cloud2';
			Footstep3 = Sound'HPSounds.FootSteps.HAR_foot_cloud3';
			break;

		case FOOTSTEP_wet:
			Footstep1 = Sound'HPSounds.FootSteps.HAR_foot_wet1';
			Footstep2 = Sound'HPSounds.FootSteps.HAR_foot_wet2';
			Footstep3 = Sound'HPSounds.FootSteps.HAR_foot_wet3';
			break;

		case FOOTSTEP_grass:
			Footstep1 = Sound'HPSounds.FootSteps.HAR_foot_grass1';
			Footstep2 = Sound'HPSounds.FootSteps.HAR_foot_grass2';
			Footstep3 = Sound'HPSounds.FootSteps.HAR_foot_grass3';  //bad sound
			break;

		case FOOTSTEP_metal:
			Footstep1 = Sound'HPSounds.FootSteps.HAR_foot_metal1';
			Footstep2 = Sound'HPSounds.FootSteps.HAR_foot_metal2';
			Footstep3 = Sound'HPSounds.FootSteps.HAR_foot_metal3';
			break;
	}

	decision = FRand();
	if ( decision < 0.34 )
		step = Footstep1;
	else if (decision < 0.67 )
		step = Footstep2;
	else
		step = Footstep3;

	PlaySound(step, SLOT_Interact,RandRange(0.7, 1.0), false, 1000.0, RandRange(0.9, 1.1));
}

// Return the health of the char
// range 0 to 1.0
function float GetHealth()
{
	return Health;
}

// Pick one of the random fall sounds.
function sound GetRandomFallSound()
{
	local int nActualSounds;

	for (nActualSounds=0; nActualSounds < ArrayCount(soundFalling); nActualSounds++)
	{
		if (soundFalling[nActualSounds] == None)
			break;
	}

	if (nActualSounds > 0)
		return soundFalling[Rand(nActualSounds)];
	else
		return None;
}

// Handle fall sound playing.  Note:  I considered using the "event Falling" for setting up the 
// fall sound, but there are cases when an object falls, but does not get that event.  For example,
// when a creature is flipendo'd, the physics is changed to PHYS_FALLING in script, but a falling
// event does not happen.  So we manually check the current physics state to determine
// when to start and stop the falling sound.
function HandleFallSounds()
{
	local vector vUnderLocation;

	// If there's not sound in the first element of the falling sound array, then
	// assume no falling sounds and there's nothing to do.
	if (soundFalling[0] == None)
		return;

	// If currently falling
	if (Physics == PHYS_Falling)
	{	
		// If not already playing falling sound
		if ((soundCurrFalling == None))
		{
			// If ground is a ways away, then play falling sound
			vUnderLocation = Location + vec(0,0,-fFallSoundDist);			
			if (FastTrace(vUnderLocation,Location))
			{
				soundCurrFalling = GetRandomFallSound();
				PlaySound(soundCurrFalling, SLOT_None);
			}
		}
	}

	// Not falling
	else
	{
		// If falling sound is playing, stop because we're not falling anymore.
		if (soundCurrFalling != None)
		{
			StopSound(soundCurrFalling, SLOT_None); 
			soundCurrFalling = None;
		}
	}
}

//********************************************************************
defaultproperties
{
	bFlipPushable=false
	fFlipPushForceXY=200
	fFlipPushForceZ=250
	
	aHolding=None

	bCanWalk=true
	bstatic=false
	DrawType=DT_Mesh
	Physics=PHYS_WALKING

    GroundSpeed=+200.000000
    AirSpeed=+00100.000000
    AccelRate=+01024.000000
    BaseEyeHeight=40.750000
    EyeHeight=40.750000
    Mass=100.000000
    Buoyancy=118.800003
	PeripheralVision=0.85
	SightRadius=1000.0

	ShadowClass=class'ActorShadow'

	CurrFidgetAnimName=none
	CurrIdleAnimName=none
	FidgetNums=0
	IdleNums=0

	iMinIdleSeconds=5
	iMaxIdleSeconds=10

	bCouldWatchForHarry=false

	fCutSceneTime=10.000000

	fWatchForHarryDist=512
	fNotifyOthersHearDistance=600

	EventName=CallSnape

	fFallSoundDist=100

	bCantStandOnMe=true

    EnemyHealthBar=EnemyBar_None
	
	bGestureFaceHorizOnly=false
}

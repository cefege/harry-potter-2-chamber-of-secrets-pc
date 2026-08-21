

class Basilisk expands baseBasilisk;

//skBucketMesh  'down'

//orient, lunge1, idle

var BasiliskAnimChannel  _AnimChannel;
var GenericColObj        _BasiliskHeadColObj;
var GenericColObj        _BasiliskBreastColObj;
var() float   BreastHitDamageMult;

var   bool    bAnimDone;

var   bool    bIdleState;      //toggles back and forth on trigger
var   bool    bDidFirstBattle;

var   ActorChaser  aEyeTarget;

var   bool    bTriggerSelfAtStartup;

var   name    StateNameSave;

var   int     NumIdleLoops;

var   bool    bUseEyeBeam;

// All these are in degrees
var   float   ActualYaw;  //Actual Yaw always tries to point towards Desired Yaw.
var   float   StartYaw;   // StartYaw is so I can use EaseBetween it and DesiredYaw
var   float   DesiredYaw;
var   float   YawTime;     //Passage of time for yaw travel
var   float   YawTimeDest;
//var

var   float   SlideDistance;
var   float   SlideSpeed;

var   float   CamXOffset;
var   float   CamZOffset;

//var   float   CurrentHeadYawRate;
var() float   HeadYawRate;
var() float   MaxHeadYaw;   //Furthest the head can rotate

//var() float   AttackPeriod;
var() float   b1_TimeBetweenAttackStart;
var() float   b1_TimeBetweenAttackEnd;
var() float   b1_EyeShootWarningStart;
var   float   b1_EyeShootWarningEnd;

var   SnakeBeam EyeBeam1;
var   SnakeBeam EyeBeam2;
var   float   BeamGrowTime;
var   float   BeamGrowTimeSpan;
//var   int     BeamPitch;
var   float   BeamLife;
var   float   BeamLifeSpan;
var   int     BeamYawRate_DegreesPerSec;
var   int     BeamYawStartOffset_Degrees;
var   float   BeamLength;
var   float   BeamHurtHarryTimer;
var() float   BeamDamage;
var() float   BeamChaseSpeed;
var() float   BeamChaseAccel;


var() float   AttackPeriod2;
var   float   AttackTimer;

var   float   HeadSpazSpeed;

//var() float   HarryInRangeDist;

var   float   HarryDamageTimer;

var() float   HeadCollisionRadius;
var   name    MainBoneName;

var   name    WaitingState;
var   name    BasilNextState;
//var   name    BasilNextState2;  //temporarily used to redirect action after the stateHit_2_ state.

var   float   TempYawSave;
var   float   TempFloat;
var   float   TempFloat2;
var   sound   TempSound;
var   name    TempName;
var   bool    TempBool;
var   float   LastAnimFrame;
var   float   TempHarryDist;

var   int     AttackAttemptCount;

var   bool    bEyeShootToggle;

var   float   HeadAttackNearest;
var   float   HeadAttackFarthest;
const         HeadAttackCount = 6;   //Number of head attack anims  (probably 4)
//var   name    HeadAttackAnimName[3];
//var   name    HeadAttack2AnimName[3];//The quick lunge out of hole anims
var() float   HeadAttackAnimRate;
var() float   b1_HeadRoarAnimRate_Start;
var() float   b1_HeadRoarAnimRate_End;

var   float   HeadAttackNearest_2;
var   float   HeadAttackFarthest_2;
const         HeadAttackCount_2 = 4;   //Number of head attack anims  (probably 4)

var() float   b2_SprayWarningAnimRate;

var   bool    bGotHitDuringIdle;
var   bool    bGotHitDuringWarmUp;

var   int     AttackFromHolePart;  //Counts the attack from hole anims, 1,2,3,4

var() name    HoleMarkerCommonTag;
var   vector  vRoomCenter;
var   actor   BasilStartPoint;
var   int     iLastHole;
var   int     NumHoles;
var   actor   HoleMarker[4];
var   int     CurrentHole;
var   bool    bBasilCameOutFirstHoleAlready;
var() int     OutHoleMaxRandYaw;
var() float   BasilSniffDistance;

var   bool    bListenToHarry;
var   bool    bHaveHeardHarry;
var   bool    bAttackAfterBeingHit;

var   baseSpell LastDodgedSpell;

var   float   SpellToHeadProximity;

var   float   FloorZ;

var   float   AcidSpitYawSpread;
var   float   AcidSpitYawRate;
var   int     AcidSpitCount;
var   int     AcidSpitCountTemp; //Just holds AcidSpitCount with some variance
var   float   AcidSpitFreq;
var   float   AcidSpitPeriod;    //calculated from freq
var   int     AcidSpitCounter;
var   float   AcidSpitStartYaw;
var   float   AcidSpitStartYawStepSize;
var   float   AcidSpitTargetDistributionWidth;
var   float   AcidSpitChaseHarryYawStart;   //these are actually ints
var   float   AcidSpitChaseHarryYawSpread;  //these are actually ints
//var   vector  AcidSpitRandomOffset;

var   float   AcidSpitStartFrame;
var   float   AcidSpitEndFrame;
var   float   AcidSpitTimeSinceLastSpit;//LastSpitTime;
var   actor   AcidSpitLastActor;

var   float   BasilSoundRadius;

//******************************
// Some basiliskSpell vars.  These are so the designer can mess with them...
var    float  SpellDamageAmount;
var    float  SpellInitialDrawScale;
var    float  SpellEndDrawScale;
//var()  float  SpellMaxTravelDistance;
var    float  SpellStartSpeed;
var    float  SpellEndSpeed;
//******************************

var   BasilEyeGlow  EyeGlowL;
var   BasilEyeGlow  EyeGlowR;
//var   

var   vector  LastNoiseLoc;
var   float   TimeSinceLastHearNoise;
var   float   HarryMakingNoiseTimer;  //"How long" harry's been making noise

var   float   MinDamageToBotherBasil;

var   int     NumSpellsDodged;

var   actor   aLastHole;

//var() vector  BasilHeadTargetOffset;

//******************************************************************************************
function PostBeginPlay()
{
	local actor a;
	local int   i;

	super.PostBeginPlay();

	WaitingState = 'stateWait1';

	//Point the head Straight forward.
	//PlayAnim( 'orient' );  //This actor's animchannel is always in anim orient
	//AnimFrame = 0.5;
	//AnimRate =  0;

	//HideBasil();

	_AnimChannel = BasiliskAnimChannel( CreateAnimChannel(class'BasiliskAnimChannel', AT_Replace, MainBoneName) );
	_AnimChannel._SetOwner( self );

	_BasiliskHeadColObj = spawn( class'GenericColObj', self );
	_BasiliskHeadColObj.AttachToOwner( 'Ani_bone26' );
	_BasiliskHeadColObj.eVulnerableToSpell = SPELL_None;//SPELL_Flipendo;
	_BasiliskHeadColObj.bIsHead = true;
	_BasiliskHeadColObj.SetCollisionSize( HeadCollisionRadius*DrawScale*0.5, HeadCollisionRadius*DrawScale );

	_BasiliskBreastColObj = spawn( class'GenericColObj', self );
	_BasiliskBreastColObj.AttachToOwner( 'Bone17' );
	_BasiliskBreastColObj.eVulnerableToSpell = SPELL_None;//SPELL_Flipendo;
	//_BasiliskBreastColObj.bIsHead = true;
	_BasiliskBreastColObj.SetCollisionSize( HeadCollisionRadius*DrawScale*0.5, HeadCollisionRadius*DrawScale );

	ForEach AllActors(class'actor', a, 'BasiliskFloorMarker')
		break;
	FloorZ = a.Location.z;

	ForEach AllActors(class'actor', a, HoleMarkerCommonTag)
		if( Vsize2d(a.Location - Location) < 70 )
			{ BasilStartPoint = a;    break; }

		//vRoomCenter = vRoomCenter  +  (a.Location - vRoomCenter)*(1/++i);
	//vRoomCenter.z = FloorZ;

	//fancyspawn(class'FireCrab', [SpawnLocation]vRoomCenter);

	AttackTimer = b1_TimeBetweenAttackStart;

	EyeGlowL = spawn(class'BasilEyeGlow', self);
	EyeGlowL.AttachToOwner( 'Bone144' );
	EyeGlowL.EnableEmission( false );
	EyeGlowR = spawn(class'BasilEyeGlow', self);
	EyeGlowR.AttachToOwner( 'Bone118' );
	EyeGlowR.EnableEmission( false );

	NumHoles = 4;
	for( i = 0; i < NumHoles; i++ )
	{
		ForEach AllActors(class'actor', a, name(HoleMarkerCommonTag $ i))
		{
			HoleMarker[i] = a;
			Log("************** hole:"$a.name);
		}
	}

	AcidSpitPeriod = 1/AcidSpitFreq;
}

//*********************************************************************************************************************
function Trigger( Actor Other, Pawn EventInstigator )
{
	playerHarry.ClientMessage("Basil got triggered");

	//If basil is active, make sure we now turn on bDidFirstBattle
	if( !bIdleState )
		bDidFirstBattle = true;

	bIdleState = !bIdleState;

	//If we're supposed to be going to an idle state, then do it
	if( bIdleState )
	{
		GotoState( 'stateIdle' );
	}
	else
	{
		playerHarry.HearHarryRecipient = self;

		if( !bDidFirstBattle )
			GotoState( 'stateInitBasil1' );
		else
			GotoState( 'stateInitBasil2' );
	}
}

//*************************************************************************************************************************
//This is for where the camera should be looking.
function vector GetCamTargetLoc()
{
	local vector v;
	local vector v1, v2, vH;
	local vector vLoc;

//change this so it doesn't move around, then the mouse control will feel better.
//then do the viewport clipping.

	if( !bDidFirstBattle )
	{
		//v = ( Location + ((vec(CamXOffset,0,CamZOffset)*DrawScale) >> Rotation) ) * 0.75  +  _BasiliskHeadColObj.Location * 0.25;
		v = ( Location + ((vec(CamXOffset,0,CamZOffset)*DrawScale) >> Rotation) );// * 0.75  +  _BasiliskHeadColObj.Location * 0.25;
		//v2 = (playerHarry.Location+vec(0,0,playerHarry.EyeHeight))  -  v;

		vH = playerHarry.Location + vec(0,0,playerHarry.EyeHeight);

		//v1 = v -  playerHarry.cam.Location;
		//v2 = vH - playerHarry.cam.Location;
		//
		//vLoc = playerHarry.cam.Location  +  normal(v1) + normal(v2);
		vLoc = v;
	}
	else
	{
		vLoc = Location*vect(1,1,0) + vec(0, 0, FloorZ + 50);
	}

	return vLoc;
}

//*************************************************************************************************************************
//This one is for where harry should be shooting his spells
function vector GetTargetLocation()
{
	local vector v;
	local float  f;

	f = 15;

	if( !bDidFirstBattle )
	{
		v = _BasiliskHeadColObj.Location + vec(RandRange(-f,f),RandRange(-f,f),RandRange(-f,f));
	}
	else
	{
		if( bHidden )
			v = Location*vect(1,1,0) + vec(0,0,FloorZ + 70) + vec(RandRange(-f,f),RandRange(-f,f),RandRange(-f,f));
		else
			//v = _BasiliskHeadColObj.Location + vec(RandRange(-f,f),RandRange(-f,f),RandRange(-f,f));
			v = _BasiliskHeadColObj.Location*vect(1,1,0) + vec(0,0,FloorZ + 70) + vec(RandRange(-f,f),RandRange(-f,f),RandRange(-f/4,f/4));
	}

	return v;
}

//******************************************************************************************
//This is where harry will face.
function vector GetHarryFaceLocation()
{
	if( !bDidFirstBattle )
		return _BasiliskHeadColObj.Location;
	else
		return Location*vect(1,1,0) + vec(0,0,FloorZ + 70);
}

//******************************************************************************************
//This one is for the loc that harry should be moving around
function vector GetHarryMovementCenter()
{
//	if( !bDidFirstBattle )
//		return Location;
//	else
		return Location*vect(1,1,0) + vec(0,0,FloorZ + 50);
}

//*************************************************************************************************************************
function bool SetCamMode()
{
	playerHarry.cam.rDestRotation = rotator( normal((Location - playerHarry.Location)*vect(1,1,0)) );
	return true;
}

//******************************************************************************************
function SnakePlayAnim( name Sequence, optional float Rate, optional float TweenTime, optional EAnimType Type, optional name RootBone )
{
	_AnimChannel.bAnimDone = false;

	if( Rate == 0 )
		Rate = 1.0;

	_AnimChannel.PlayAnim( Sequence, Rate, TweenTime, Type, RootBone );
}

function SnakeLoopAnim( name Sequence, optional float Rate, optional float TweenTime, optional float MinRate, optional EAnimType Type, optional name RootBone )
{
	_AnimChannel.bAnimDone = false;

	if( Rate == 0 )
		Rate = 1.0;

	_AnimChannel.LoopAnim( Sequence, Rate, TweenTime, MinRate, Type, RootBone );
}

//******************************************************************************************
function float GetHealth()
{
	return float(Health) / 100;
}

//******************************************************************************************
//Overridden in states.
function bool BasilAcksHit(baseSpell spell)
{
	return true;
}

//******************************************************************************************
//Overridden in states.
function float GetBasilDamageScalar()
{
	return 1.0;//0.25;
}

//******************************************************************************************
//Overridden in states.
function bool ShouldDodgeSpell()
{
	//cm(" size:"$VSize2d( playerHarry.Location - Location ) );
	return VSize2d( playerHarry.Location - Location )  >  200;
}

//******************************************************************************************
//event called when basil munches harry
function BasilMunchesHarry()
{
}

//******************************************************************************************
//This function is overridden so that the state doesn't get saved.
function SaveCurrentStateName()
{
	StateNameSave = GetStateName();
}

//******************************************************************************************
auto state stateStartup
{
  Begin:

	if( bTriggerSelfAtStartup )
	{
		bTriggerSelfAtStartup = false;
		//bDidFirstBattle = true;  //set this to false to test first basil.  Actually, do it in the editor
		Trigger(none, none);
		playerHarry.bFraserMode = true;
	}

	//HideBasil();
	PlayAnim('birth', 0);

	Sleep( 2 );
	//playerHarry.ClientMEssage("AnimRate:"$AnimRate);
	Goto 'Begin';
}

//******************************************************************************************
state stateIdle
{
  Begin:
	//play the anim channel version as well, with tween
	//LoopAnim( 'idle', , 3.0 );//, [Type]AT_Combine );
	Sleep(2);
}

//******************************************************************************************
state stateHidden
{
	function BeginState()
	{
		HideBasil();
	}

  Begin:
	//play the anim channel version as well, with tween
	//LoopAnim( 'idle', , 3.0 );//, [Type]AT_Combine );
	Sleep(2);
}

//*********************************************************************************************************************
function PawnHearHarryNoise()// float Loudness, Actor NoiseMaker )
{
	//playerHarry.ClientMessage("%%%%% Harry made noise");

	//if( Harry(NoiseMaker) == none )
	//	return;

	//You need to be listening to Harry.
	if( bListenToHarry )
	{
		bHaveHeardHarry = true;
		LastNoiseLoc = PlayerHarry.Location;//NoiseMaker.Location;
		TimeSinceLastHearNoise = 0;
	}
}

//*********************************************************************************************************************
function HideBasil()
{
	PlayAnim('birth', 0);
	SetLocation( Location + vect(0,0,-1000) );
	bHidden = true;
}

//******************************************************************************************
function RotateTo( float yaw, optional float time, optional float rate )
{
	DesiredYaw = yaw;

	if( DesiredYaw < -MaxHeadYaw )
		DesiredYaw = -MaxHeadYaw;
	else
	if( DesiredYaw >  MaxHeadYaw )
		DesiredYaw =  MaxHeadYaw;

	StartYaw =   ActualYaw;

	YawTime = 0;
	if( time != 0 )
		YawTimeDest = time;
	else
	if( rate != 0 )
		YawTimeDest = Abs( DesiredYaw - StartYaw ) / rate;
	else
		YawTimeDest = Abs( DesiredYaw - StartYaw ) / HeadYawRate;
}

//******************************************************************************************
function RotateToHarry( optional float time, optional float rate )
{
	RotateTo( DegreeRotToHarry(), time, rate );
}

//******************************************************************************************
function float DegreeRotToHarry(optional bool bReturnAsYaw)
{
	return DegreeRotToActor( playerHarry, bReturnAsYaw );
}

//******************************************************************************************
function float DegreeRotToActor(actor a, optional bool bReturnAsYaw)
{
	local int    YawToHarry;
	local vector SnakeLoc;
	local float  DegToHarry;

	SnakeLoc = Location + vector(Rotation) * 36;

	if( bReturnAsYaw )
		return Rotator((a.Location - SnakeLoc) * vect(1,1,0)).yaw & 0xFFFF;

	//Find engine yaw to harry, and make sure it's positive
	YawToHarry = (  Rotator((a.Location - SnakeLoc) * vect(1,1,0)).yaw
	              - Rotation.Yaw  +  0x20000
	             ) & 0xFFFF;

	if( YawToHarry >= 0x8000 )  //now make it -0x8000 to 0x7FFF
		YawToHarry -= 0x10000;

	//Convert to degrees, return
	DegToHarry = YawToHarry * 360.0 / 0x10000;
	//playerHarry.ClientMessage("DegToharry:"$DegToHarry);
	return DegToHarry;
}

//******************************************************************************************
function Tick(float dtime)
{
	local baseSpell a;
	local float     d;
	local vector    v;
	local vector    v2;
	local float     theta;

	if( bIdleState )
		return;

	TimeSinceLastHearNoise += dtime;

	//if TimeSinceLastHearNoise is less than say 0.7, then harry is "making noise."  Count up our timer.
	if( TimeSinceLastHearNoise < 0.7 )
		HarryMakingNoiseTimer += dtime;
	else
		HarryMakingNoiseTimer = 0;

	//If we're in rotate to desired mode, rotate to the last noise loc
	if( bRotateToDesired  &&  HarryMakingNoiseTimer > 0 )
		DesiredRotation = Rotator((LastNoiseLoc - Location) * vect(1,1,0));

	if( !bDidFirstBattle )
	{
		if( YawTime < YawTimeDest )
		{
			YawTime += dtime;
			if( YawTime > YawTimeDest )
				YawTime = YawTimeDest;

			ActualYaw = StartYaw   +   (DesiredYaw - StartYaw) * EaseBetween( YawTime / YawTimeDest );
		}
		else
		{
			ActualYaw = DesiredYaw;
		}

		AnimFrame = FClamp( (ActualYaw + 90.0)/180.0 * 0.967, 0.0, 0.967 );
		//playerHarry.ClientMessage("ActualYaw:"$ActualYaw$" DesiredYaw:"$DesiredYaw$" AnimFrame:"$AnimFrame);
	}
	else //battle part two.  Basil 2, if you will...
	{
		//If you should dodge a spell
		if( ShouldDodgeSpell() )
		{
			a = basewand(playerHarry.weapon).LastCastedSpell;
			//And there's a spell flying
			if( a != none  &&  a != LastDodgedSpell )
			{
				//And it's within spell dodging distance		   
				v = basewand(playerHarry.weapon).LastCastedSpell.Location - _BasiliskHeadColObj.Location;
				d = VSize(v);
				if( d < 350 )//270 ) //250
				{
					//And it's flying at you
					theta = atan( SpellToHeadProximity / d );
					//if( (normal(basewand(playerHarry.weapon).LastCastedSpell.velocity) dot (v/d))  >  cos(theta) )
					v2 = normal(basewand(playerHarry.weapon).LastCastedSpell.velocity)  cross  (v/d);
					if( VSize(v2)  <  sin(theta) )
					{
						LastDodgedSpell = a;
						SaveCurrentStateName();
						//StateNameSave = GetStateName();

						if( v2.z > 0 )
							GotoState( 'stateDodgeSpell_L' );
						else
							GotoState( 'stateDodgeSpell_R' );
					}
				}
			}
		}

	}

	if( playerHarry.PlayerIsAiming() )
		PawnHearHarryNoise();

	AttackTimer -= dtime;

	HarryDamageTimer += dtime;

	LastAnimFrame = AnimFrame;
}

//******************************************************************************************
function RealAnimEnd()
{
}

//******************************************************************************************
state stateInitBasil1
{
	function BeginState()
	{
		local BlockPlayer  a;

		//Make the blockplayers that are close by actually be block players
		ForEach AllActors(class'BlockPlayer', a)
			if( VSize(a.Location - playerHarry.Location) < 600 )
				a.SetCollision( , , true );
	}

  Begin:
	PlayAnim( 'orient' );  //This actor's animchannel is always in anim orient
	AnimFrame = 0.5;
	AnimRate =  0;
	SnakePlayAnim( 'birth' );
	//PlaySound coming out...
	FinishAnim();
	BasilNextState = 'stateWait1';
	WaitingState = 'stateWait1';
	GotoState( WaitingState );
}

//******************************************************************************************
state stateWait1
{
	function EndState()
	{
		SetTimer(0,false);
	}

	function BasilHitBySpell( baseSpell spell, vector HitLocation )
	{
		Global.BasilHitBySpell( spell, HitLocation );
		bGotHitDuringIdle = true;
		AttackTimer = 0;
	}

	function bool BasilAcksHit(baseSpell spell)
	{
		if( spell.Damage >= MinDamageToBotherBasil )
			return true;
		else
			return false;
	}

	//function BeginState()
	//{
	//	SetTimer( RandRange(0.5,1.0), false );
	//	PlayHissSound();
	//}

  Begin:

	bGotHitDuringIdle = false;

	//Log( "************************* <<:    "$(_BasiliskHeadColObj.Location - Location) << Rotation );//var() vector  BasilHeadTargetOffset;

	//Timer();

	AttackAttemptCount = 0;

	if( bUseEyeBeam )
		GotoState( 'stateEyeSpell' );

	do
	{
		SnakeLoopAnim( 'idle', RandRange(0.5,0.7), 0.7 );
		//SetRandomDesiredYawOffset( 55 );
		RotateToHarry( , 40 );
		do { Sleep(0.001); } until( ActualYaw == DesiredYaw );
	}until( AttackTimer <= 0 );

	RotateToHarry( , 120 ); // <=-- a bit quicker rate
	do { Sleep(0.001); } until( ActualYaw == DesiredYaw );

	//playerHarry.ClientMessage("disttoharry:"$vsize2d(playerHarry.Location-Location)$" HarryInRangeDist:"$HarryInRangeDist);

	//Do his mad lashing
	//if( FRand() < 0.12  ||  !playerHarry.IsInState( 'PlayerWalking' ) )
	//{
	//	playerHarry.ClientMessage("Basil: Do a Spaz");
	//	GotoState('stateSpaz');
	//}
	//else //if harry's up on the podium
	//if( DegreeRotToHarry() > -45 && DegreeRotToHarry() < 45  &&  playerHarry.Location.Z+playerHarry.FootOffsetZ - FloorZ > 50 )
	//{
	//	playerHarry.ClientMessage("Harry On Podium");
	//
	//	if( FRand() < 0.75 )
	//		GotoState('statePodiumAttack');
	//	else
	//		GotoState('stateEyeSpell');
	//}
	//else
	if( VSize2d( playerHarry.Location - Location )  <  HeadAttackFarthest + 20 )
	{
		playerHarry.ClientMessage("Harry in range for lunge");

		bGotHitDuringWarmUp = false;
		AttackAttemptCount = 0;
		//if( FRand() < 0.95 )
			GotoState('stateAttack');
		//else
		//	GotoState('stateAcidSpit');
	}
	else
	{
		GotoState('stateAcidSpit1');
	}
	////if( FRand() < 0.95 )
	//{
	//	playerHarry.ClientMessage("Basil: stateEyeSpell");
	//	bGotHitDuringWarmUp = false;
	//	AttackAttemptCount = 0;
	//	GotoState('stateEyeSpell');
	//}
	//else
	//{
	//	playerHarry.ClientMessage("Basil: stateAcidSpit");
	//	GotoState('stateAcidSpit');
	//}

	Goto 'Begin';
}

//********************************************************************************************************************************
//********************************************************************************************************************************

state stateAcidSpit1
{
	function BeginState()
	{
		AcidSpitTimeSinceLastSpit = AcidSpitPeriod;
	}

	function tick(float dtime)
	{
		global.Tick(dtime);

		//See if it's time to shoot acid
		if(   _AnimChannel.AnimSequence == 'Spit'
		   && _AnimChannel.AnimFrame >= 10.0/46.0  &&  _AnimChannel.AnimFrame <= 38.0/46.0
		  )
		{
			AcidSpitTimeSinceLastSpit += dtime;
			if( AcidSpitTimeSinceLastSpit >= AcidSpitPeriod )
			{
				while( AcidSpitTimeSinceLastSpit >= AcidSpitPeriod )
					AcidSpitTimeSinceLastSpit -= AcidSpitPeriod;
			
				CastSpitSpell(true, true);
			}
		}

		RotateToHarry();//DesiredRotation.Yaw = DegreeRotToHarry(true);
	}

	function bool BasilAcksHit(baseSpell spell)	{ return true; }//false; }
	function bool ShouldDodgeSpell()            { return false; }
	//function float GetBasilDamageScalar()       { return 1.0; }

  Begin:

	//AcidSpitChaseHarryYawStart = rotator(playerHarry.Location - Location).yaw - AcidSpitChaseHarryYawSpread/2;

	//PlayAnim('Spit', 0.75, 0.5);
	SnakePlayAnim( 'Spit', 0.75, 0.5 );

	while( !_AnimChannel.bAnimDone ) Sleep(0.00001);

	SetAttackTimer();
	GotoState( WaitingState );
}

//*********************************************************************************************************************
//*********************************************************************************************************************

//******************************************************************************************
state stateInitBasil2
{
  Begin:

	//	Sleep( 2 );
	//	bHidden = false;	
	//	playerHarry.ClientMessage("TrigEventWhenDefeated2:"$TrigEventWhenDefeated2);
	//	SendDefeatedTrigger2();
	//	GotoState( 'stateIdle' );

	playerHarry.StartBossEncounter( self, false, false, false, true, vect(0,0,0), SPELL_None, false );
	//playerHarry.Cam.SetModeByString( "Boss" );

	//PlayAnim( 'orient' );  //This actor's animchannel is always in anim orient
	//AnimFrame = 0.5;
	//AnimRate =  0;
	bRotateToDesired = true;
	SetPhysics( PHYS_Rotating );

	//_AnimChannel.Destroy();  Dang, this causes an engine crash...

	HideBasil();

	MakeSmokeEyeEffects();

	

	//SnakePlayAnim( 'birth' );
	//PlaySound coming out...
	//FinishAnim();
	WaitingState = 'stateWait2';
	BasilNextState = 'stateRetreat';
	GotoState( WaitingState );
}

//******************************************************************************************
function MakeSmokeEyeEffects()
{
	local BasilEyeSmoke    bs;

	bs = spawn( class'BasilEyeSmoke', self );
	bs.AttachToOwner( 'Bone144' );
	bs = spawn( class'BasilEyeSmoke', self );
	bs.AttachToOwner( 'Bone118' );
}

//******************************************************************************************
state stateWait2
{
  Begin:
	SetAttackTimer();
	bListenToHarry = true;

	NumSpellsDodged = 0;

	//TempFloat = GetSoundDuration( sound'HPSounds.critters_sfx.BasilAttackWarning00' );
	playerHarry.PlaySound( sound'HPSounds.critters_sfx.BasilAttackWarning00', SLOT_None, /*[Volume]1.0,*/ [Radius]BasilSoundRadius, [Pitch]0.75 );

	TempFloat = GetSoundDuration( sound'HPSounds.critters_sfx.Bow_taunt9' ) / 0.3;
	playerHarry.PlaySound( sound'HPSounds.critters_sfx.Bow_taunt9', SLOT_None/*, [Volume]0.8*/, [Radius]BasilSoundRadius, [Pitch]0.3 );
	sleep( TempFloat - 0.5 );

	TempFloat = GetSoundDuration( sound'HPSounds.critters_sfx.Bow_taunt1' ) / 0.6;
	playerHarry.PlaySound( sound'HPSounds.critters_sfx.Bow_taunt1', SLOT_None/*, [Volume]0.6*/, [Radius]BasilSoundRadius, [Pitch]0.6 );
	sleep( TempFloat - 0.75 );

	//sleep( PlayMovingThroughWallSound(0.2, true) );
	//sleep( PlayMovingThroughWallSound(0.4, true) );
	//sleep( PlayMovingThroughWallSound(0.6, true) );
	//sleep( PlayMovingThroughWallSound(0.8, true) );
	MoveToNewHole();//true);

	//probably shouldn't call FindClosestGrate() twice, but eh...
	if( FindClosestGrate() != none )
	{
		FindClosestGrate().OnEvent( 'rattle' );
		playerHarry.ShakeView( 0.5, 50, 50 );
		FindClosestGrate().PlaySound( sound'HPSounds.Adv11_cos.floor_grate_warning_rattle', SLOT_None, [Radius]BasilSoundRadius, [Pitch]0.9 );
		sleep( 1.0 );//PlayMovingThroughWallSound(1, true) );
	}

	//if( !bBasilCameOutFirstHoleAlready	)
	//{
	//	bBasilCameOutFirstHoleAlready = true;
	//	SetupNewAttack(false);
	//}
	//else
	//{
		//TODO: when basil's shooting, make him 'react' when harry hits him...
		//do
		//{
		//	sleep( PlayMovingThroughWallSound() );
		//} until( AttackTimer <= 0 );		
		//SetupNewAttack(true);

	//	MoveToNewHole( false );
	//}

	GotoState( 'stateComeOutAndSniff1' );
}

//******************************************************************************************
//function SetupNewAttack(bool bMoveToClosestHole) //bMoveToNextHole)
//{
//	//if( bMoveToNextHole )
//		MoveToNewHole( bMoveToClosestHole );
//
//	GotoState( 'stateComeOutAndSniff1' );
//	return;
//}

//***********************************************************************************************************************************
//***********************************************************************************************************************************
state stateComeOutAndSniff1
{
	function bool ShouldDodgeSpell() { return Global.ShouldDodgeSpell()  &&  AnimFrame >= 5.0/85.0; }
	function float GetBasilDamageScalar() { return 1.0; }

	function BeginState()
	{
		bHidden = false;
		bHaveHeardHarry = false;
		bAttackAfterBeingHit = true;
	}

  Begin:
	//See if the grate is there
	if( BustOffTheGrate() )
		playerHarry.ShakeView( 1.5, 150, 150 );
	else
		playerHarry.ShakeView( 0.75, 50, 50 );

	PlayAnim( 'OutHole', 0.8 );
	//_BasiliskHeadColObj.PlaySound( sound'HPSounds.critters_sfx.Basilisk_scream_suprise_popup', SLOT_None, , false, BasilSoundRadius, RandRange(0.8,1) );
	_BasiliskHeadColObj.PlaySound( sound'HPSounds.critters_sfx.Bas_OutHole', SLOT_None, , false, BasilSoundRadius, RandRange(0.8,1) );
	FinishAnim();

	GotoState( 'ComeOutAndSniff1a' );
}

//* * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * 
state ComeOutAndSniff1a
{
  Begin:
	if( bHaveHeardHarry )
	{
		//He wasn't hit by harry, so turn this off, otherwise if harry hits him while attacking, he'll attack right after his 'react'
		bAttackAfterBeingHit = false;
		
		GotoState('stateAttackFromHole');
	}

	GotoState('stateComeOutAndSniff2');
}

//* * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * 
state stateComeOutAndSniff2
{
	function Timer()
	{
		if( VSize2d(playerHarry.Location - _BasiliskHeadColObj.Location)  <  BasilSniffDistance )
		{
			//He wasn't hit by harry, so turn this off, otherwise if harry hits him while attacking, he'll attack right after his 'react'
			bAttackAfterBeingHit = false;
			GotoState('stateAttackFromHole');
		}
	}

	function EndState()
	{
		SetTimer(0, false);
	}

	function PawnHearHarryNoise()
	{
		global.PawnHearHarryNoise();
		//He wasn't hit by harry, so turn this off, otherwise if harry hits him while attacking, he'll attack right after his 'react'
		bAttackAfterBeingHit = false;
		GotoState('stateAttackFromHole');
	}

	function float GetBasilDamageScalar() { return 1.0; }

  Begin:
	SetTimer( 0.25, true );
	PlayAnim('Sniff', 1.0, 0.2);
	switch( Rand(3) )
	{	case 0:  _BasiliskHeadColObj.PlaySound( sound'HPSounds.critters_sfx.Basilisk_sniff1', SLOT_None, , false, BasilSoundRadius, RandRange(0.8,1) );    break;
		case 1:  _BasiliskHeadColObj.PlaySound( sound'HPSounds.critters_sfx.Basilisk_sniff2', SLOT_None, , false, BasilSoundRadius, RandRange(0.8,1) );    break;
		case 2:  _BasiliskHeadColObj.PlaySound( sound'HPSounds.critters_sfx.Basilisk_sniff3', SLOT_None, , false, BasilSoundRadius, RandRange(0.8,1) );    break;
	}
	FinishAnim();
	GotoState('stateComeOutAndSniff3');
}

//* * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * 
state stateComeOutAndSniff3
{
  Begin:

	PlayAnim('GoBack', 1.0, 0.2);
	FinishAnim();

	SetAttackTimer();
	HideBasil();
	GotoState( WaitingState );
}

//*********************************************************************************************************************
//*********************************************************************************************************************
function DoAttack2()
{
	bListenToHarry = false;

	// No more lunge for Basil2

	//if harry is within lunge distance, then do that, otherwise spit
	//if( Vsize2d(playerHarry.Location - Location) < HeadAttackFarthest_2 + 20 )
	//{
	//	//GotoState( 'stateAttackFromHole' );
	//	GotoState( 'stateAttack_2_' );
	//}
	//else //spit
	//{
		GotoState('stateAcidSpit');
	//}
}

state stateAttackFromHole
{
	function BeginState()
	{
		bGotHitDuringWarmUp = false;
	}

	function bool BasilAcksHit(baseSpell spell)	{ return false; }

	function BasilHitBySpell( baseSpell spell, vector HitLocation )
	{
		Global.BasilHitBySpell( spell, HitLocation );
		if( spell.Damage >= MinDamageToBotherBasil )
		{
			bGotHitDuringWarmUp = true;
			GotoState('stateHitThenAttack_2_');
		}
	}

  Begin:
	PlayAnim( 'hiss', 1.0, 0.2 );
	playerHarry.ShakeView( 1.5, 50, 50 );
	_BasiliskHeadColObj.PlaySound(sound'HPSounds.Critters_sfx.Basilisk_scream_blinded', SLOT_None, [Radius]BasilSoundRadius, [Pitch]RandRange(0.9,1.1) );
	FinishAnim();
	//GotoState('stateAttack_2_');
	DoAttack2();
}

//*** *** *** *** *** *** *** *** *** *** *** *** *** *** *** *** *** 
state stateHitThenAttack_2_
{
	function bool BasilAcksHit(baseSpell spell)	{ return false; }
	//function bool ShouldDodgeSpell()            { return true; }//false; }
	//function float GetBasilDamageScalar()       { return 1.0; }

  Begin:
	PlayAnim( 'Hiss', b2_SprayWarningAnimRate, 1.0 );
	playerHarry.ShakeView( 1.5, 50, 50 );
	FinishAnim();
	//GotoState('stateAttack_2_');
	DoAttack2();
}

//*** *** *** *** *** *** *** *** *** *** *** *** *** *** *** *** *** 
state stateAttack_2_
{
	function bool BasilAcksHit(baseSpell spell)	{ return false; }
	function bool ShouldDodgeSpell()            { return false; }
	//function float GetBasilDamageScalar()       { return 2.0; }

	function BasilHitBySpell( baseSpell spell, vector HitLocation )
	{
		Global.BasilHitBySpell( spell, HitLocation );
		if( spell.Damage >= MinDamageToBotherBasil )
			bGotHitDuringWarmUp = true;
	}

	function BasilMunchesHarry()
	{
		//If you just shot him while he's lunging, go to hit anim now.
		if( bGotHitDuringWarmUp )
			GotoState( 'stateHit_2_' );
	}

  Begin:

	if( bGotHitDuringWarmUp )
	{
		playerHarry.ClientMessage("####      Miss Harry");
		DesiredRotation.yaw = Rotator(playerHarry.Location - location).yaw + DegRotAwayFromHarry(true)*65536.0/360.0;
		//RotateTo( DegreeRotToHarry() + 13*DegRotAwayFromHarry(), 0.2 );
	}
	else
	{
		playerHarry.ClientMessage("#### DONT Miss Harry");
		if( VSize(playerHarry.Velocity) <= 20 )
			DesiredRotation.yaw = Rotator(playerHarry.Location - location).yaw;
		else
			DesiredRotation.yaw = Rotator(playerHarry.Location - location).yaw - DegRotAwayFromHarry()*65536.0/360.0;
	}

	//We'll use this variable to save whether you shot him while he was lunging at you.
	bGotHitDuringWarmUp = false;

	PlayLungeAnim_2_();
	playerHarry.ShakeView( 0.5, 100, 100 );
	PlayLungeSound();
	FinishAnim();

	PlayAnim('GoBack', 1.0, 0.1);
	FinishAnim();

	SetAttackTimer();
	HideBasil();
	GotoState( WaitingState );
}

//*****************************************************************
function PlayLungeAnim_2_()
{
	local int i;
	local int w;

	//Get the right anim based on harry's distance from the origin

	//Distance between two adjacent anim lunges
	w = (HeadAttackFarthest_2 - HeadAttackNearest_2) / (HeadAttackCount_2 - 1);
	i = ( VSize2d(playerHarry.Location - Location) - HeadAttackNearest_2 + w/2 + w/4)   /   w;  //Add on an extra w/4 so the head lunges "further".
	i = Clamp( i, 0, HeadAttackCount_2-1 );
	PlayAnim( name("snap"$i+1), HeadAttackAnimRate, 0.2 );
}

//********************************************************************************************************************************
//********************************************************************************************************************************


state stateAcidSpit
{
	function BeginState()
	{
		AcidSpitTimeSinceLastSpit = AcidSpitPeriod;
	}

	function tick(float dtime)
	{
		global.Tick(dtime);

		//See if it's time to shoot acid
		if(   AnimSequence == 'Spray'
		   && AnimFrame >= AcidSpitStartFrame  &&  AnimFrame <= AcidSpitEndFrame
		   //&& Level.TimeSeconds >= AcidSpitLastSpitTime + AcidSpitPeriod //1/AcidSpitFreq
		  )
		{
			AcidSpitTimeSinceLastSpit += dtime;
			if( AcidSpitTimeSinceLastSpit >= AcidSpitPeriod )
			{
				while( AcidSpitTimeSinceLastSpit >= AcidSpitPeriod )
					AcidSpitTimeSinceLastSpit -= AcidSpitPeriod;
			
				CastSpitSpell(true, true);
			}
		}

		DesiredRotation.Yaw = DegreeRotToHarry(true);
	}

	function bool BasilAcksHit(baseSpell spell)	{ return true; }//false; }
	function bool ShouldDodgeSpell()            { return false; }
	//function float GetBasilDamageScalar()       { return 1.0; }

  Begin:

	AcidSpitStartFrame = 18.0/61.0;
	AcidSpitEndFrame = 53.0/61.0;

	AcidSpitChaseHarryYawStart = rotator(playerHarry.Location - Location).yaw - AcidSpitChaseHarryYawSpread/2;

	LoopAnim('Spray', 0.75, 0.5);

	//AcidSpitCountTemp = AcidSpitCount * RandRange(0.75,1.25);//+ Rand(6)-3;
	//
	//if( DegRotAwayFromHarry() >= 0 ) //FRand() < 0.5 )//ActualYaw >= 0 )
	//{
	//	AcidSpitStartYaw = AcidSpitYawSpread/2;
	//	AcidSpitStartYawStepSize = -AcidSpitYawSpread/(AcidSpitCountTemp-1);
	//}
	//else //negative side of zero, go the plus direction
	//{
	//	AcidSpitStartYaw = /*ActualYaw -*/ AcidSpitYawSpread/2;
	//	AcidSpitStartYawStepSize =  AcidSpitYawSpread/(AcidSpitCountTemp-1);
	//}
	//
	//for( AcidSpitCounter = 0; AcidSpitCounter < AcidSpitCountTemp; AcidSpitCounter++ )
	//{
	//	//The speed we should move at is the gap width divided by time
	//	DesiredRotation.Yaw = ( DegreeRotToHarry(true)*360.0/65536.0 + AcidSpitStartYaw + AcidSpitStartYawStepSize*AcidSpitCounter ) * 65536.0/360.0;//, AcidSpitYawSpread/(AcidSpitCountTemp-1)/(1/AcidSpitFreq) );  // <=-- optimize?  nah...
	//	Sleep( 1/AcidSpitFreq );
	//
	//	//Cast the spit spell.
	//	if( AnimFrame >= 18.0/61.0  &&  AnimFrame <= 53.0/61.0 )
	//		CastSpitSpell( true );
	//}

	FinishAnim();


	PlayAnim('GoBack', 1.0, 0.5);
	FinishAnim();

	SetAttackTimer();
	HideBasil();
	GotoState( WaitingState );
}

//******************************************************************************************
/*
function CastSpitSpell( bool bAimAtHarry, optional bool bUseHeadYaw )
{
	local spellAcidSpit  a;
	local Rotator        r;
	local float          m;
	local float          Dist2d;
	local float          DistributionAngle;

	if( bAimAtHarry )
	{
		Dist2d = VSize2d(_BasiliskHeadColObj.location - playerHarry.Location);
		Dist2d = FMax(0.0001,Dist2d);

		DistributionAngle = 65536/2/3.1416*ATan( AcidSpitTargetDistributionWidth/2 / Dist2d );
		DistributionAngle = FClamp( DistributionAngle, -65536/8, 65536/8 );
		
		if( bUseHeadYaw )
			r.yaw = _BasiliskHeadColObj.rotation.yaw;
		else
			r.yaw = GetRealHeadYaw();
		r.yaw += RandRange(-DistributionAngle,DistributionAngle);

		m = Dist2d  /  (_BasiliskHeadColObj.location.z - FloorZ);  //run over rise
		r.pitch = 65536/2/3.1416*ATan( m ); //0 is straight down, 65536/4 is straight out 

		//playerHarry.ClientMessage("pitch:"$r.pitch$" speed:"$class'spellAcidSpit'.Default.Speed);

		r.pitch = r.pitch * 1.1; //magic number!  If you slow down the acid spell, raise this up.
		r.pitch = FMin( r.pitch, (90+45)*65536/360 );  
		r.pitch += RandRange(-DistributionAngle,DistributionAngle);//FClamp( 200/Dist2d, 0.2, 1 )   *   RandRange(-3000,3000);
		r.pitch -= 65536/4; //convert back to pitch realm
	}
	else  //Use head's rotation
	{
		r = _BasiliskHeadColObj.rotation;
	}

	a = spawn( class'spellAcidSpit', [SpawnLocation]_BasiliskHeadColObj.location, [SpawnRotation]r );
	a.FloorZ = FloorZ;

	_BasiliskHeadColObj.PlaySound( sound'HPSounds.Critters_sfx.Basilisk_spit_acid',  [Radius]BasilSoundRadius, [Pitch]RandRange(0.8,1.2) );
	a.                  PlaySound( sound'HPSounds.Critters_sfx.Basilisk_spit_acid2', [Radius]BasilSoundRadius, [Pitch]RandRange(0.8,1.2) );

}
*/

function CastSpitSpell( bool bAimAtHarry, optional bool bUseHeadYaw )
{
	local spellAcidSpit  a;
	local vector         v;
	local Rotator        r;
	local float          m;
	local float          Dist2d;
	local float          DistributionAngle;

	if( bAimAtHarry )
	{
		Dist2d = VSize2d(_BasiliskHeadColObj.location - playerHarry.Location);
		Dist2d = FMax(0.0001,Dist2d);

		r.yaw = AcidSpitChaseHarryYawStart + AcidSpitChaseHarryYawSpread * ((AnimFrame-AcidSpitStartFrame)/(AcidSpitEndFrame-AcidSpitStartFrame));
		v = _BasiliskHeadColObj.Location + vector( r ) * Dist2d;
		v.z = FloorZ;

		v = ComputeTrajectoryByTime( _BasiliskHeadColObj.Location, playerHarry.location, 1.3, -200 );
		r = rotator( v );
		a = spawn( class'spellAcidSpit', [SpawnLocation]_BasiliskHeadColObj.location, [SpawnRotation]r );

		v.z += 40 * cos( 8*Level.TimeSeconds );

		a.velocity = v;
	}
	else  //Use head's rotation
	{
		r = _BasiliskHeadColObj.rotation;
		a = spawn( class'spellAcidSpit', [SpawnLocation]_BasiliskHeadColObj.location, [SpawnRotation]r );
	}

	

	Log("*********** spawned spit:"$a.name$" v:"$v$" grav:"$Region.Zone.ZoneGravity.Z);
	a.FloorZ = FloorZ;

	//                                                                           Dont want slot_none for these two.  Want them rapid fire.
	_BasiliskHeadColObj.PlaySound( sound'HPSounds.Critters_sfx.Basilisk_spit_acid', /*SLOT_None,*/  [Radius]BasilSoundRadius, [Pitch]RandRange(0.8,1.2) );
	a.                  PlaySound( sound'HPSounds.Critters_sfx.Basilisk_spit_acid2', /*SLOT_None,*/ [Radius]BasilSoundRadius, [Pitch]RandRange(0.8,1.2) );

	if( !bDidFirstBattle )
		a.fPoolShrinkTimeMult = 0.2;
}

//*********************************************************************************************************************
//*********************************************************************************************************************

state stateDodgeSpell_L
{
	function bool ShouldDodgeSpell() { return false; }
	function SaveCurrentStateName()  { }

	function BeginState()
	{
		PlayAnim( 'LeanLeft', 1.0, 0.5 );
		GotoState( 'stateDodgeSpell' );
	}
}

//* * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * 
state stateDodgeSpell_R
{
	function bool ShouldDodgeSpell() { return false; }
	function SaveCurrentStateName()  { }

	function BeginState()
	{
		PlayAnim( 'LeanRight', 1.0, 0.5 );
		GotoState( 'stateDodgeSpell' );
	}
}

//* * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * 
state stateDodgeSpell
{
	function bool ShouldDodgeSpell() { return false; }
	//function bool BasilAcksHit(baseSpell spell)	{ return false; }
	function SaveCurrentStateName()  { }

  Begin:
	FinishAnim();

	NumSpellsDodged++;

	if( NumSpellsDodged >= 1+Rand(2) )
	{
		PlayAnim('GoBack', 1.0, 0.1);
		FinishAnim();
		SetAttackTimer();
		HideBasil();
		GotoState( WaitingState );
	}
	else
	{
		GotoState( StateNameSave );
	}
}

//*********************************************************************************************************************
//*********************************************************************************************************************

//******************************************************************************************
function bool BustOffTheGrate()
{
	local  actor  a;

	a = FindClosestGrate();

	if( a != none )
	{
		a.PlaySound( sound'HPSounds.Adv11_cos.floor_grate_explode_open', SLOT_None, [Radius]BasilSoundRadius, [Pitch]RandRange(0.9,1.1) );
		a.Trigger(self,self);
		return true;
	}

	return false;
}

// Doesn't really find closest grate, returns the grate that's right above basil, or none if
// the grate has already been popped.
function CHGrate FindClosestGrate()
{
	local CHGrate a;

	ForEach AllActors(class'CHGrate', a)
		if( VSize2d( a.Location - Location ) < 64 )
			if( !a.IsInState('OpenUp') )
				return a;//a.Trigger(self,self);

	return none;
}

//******************************************************************************************
function float PlayMovingThroughWallSound(optional float volume, optional bool bPlayFromHarry)
{
	local sound snd;
	local float pitch;
	local actor a;

	if( bPlayFromHarry )
		a = playerHarry;
	else
		a = self;

	switch( Rand(5) )
	{
		case 0:    snd = sound'HPSounds.critters_sfx.Basilisk_hiss_long1';         break;
		case 1:    snd = sound'HPSounds.critters_sfx.Basilisk_hiss_long2';         break;
		case 2:    snd = sound'HPSounds.critters_sfx.Basilisk_hiss_long3';         break;
	}

	if( volume == 0 )
		volume = RandRange(0.8,1);

	pitch = RandRange(0.8,1.0);
	a.PlaySound( snd, SLOT_None, volume, false, BasilSoundRadius, pitch );

	return GetSoundDuration(snd) / pitch  -  0.5;
}

//******************************************************************************************
function float PlayAboutToAttackSound()
{
	local float pitch;
	local sound snd;

	snd = sound'HPSounds.Critters_sfx.BasilAttackWarning00';

	pitch = RandRange(0.8,1.0);
	PlaySound( snd, SLOT_None, RandRange(0.6,1), false, BasilSoundRadius, pitch );

	return GetSoundDuration(snd) / pitch;
}

//*********************************************************************************************************************
function MoveToNewHole()//bool bClosestHole)
{
	local int     NumVisibleHoles;
	//local int     NumHoles;
	//local actor   Holes[20];
	//local actor   VisibleHoles[20];
	local vector  vDir;
	local actor   a;
	local float   ClosestHoleDist;
	local int     iClosestHole;
	local int     i;
	local rotator r;

	//if( Health <= 1 )//class'spellSwordFire'.default.fFullDamage )
	//{
	//	a = BasilStartPoint;
	//}
	//else
	if( true )//bClosestHole )// ||  !bBasilCameOutFirstHoleAlready )
	{
		//Now try moving just to the closest hole
		ClosestHoleDist = 1000000;

		for( i = 0; i < NumHoles; i++ )
		{
			if(   HoleMarker[i] != aLastHole
			   && VSize2d(HoleMarker[i].Location - playerHarry.Location) < ClosestHoleDist
			  )
			{
				ClosestHoleDist = VSize2d(HoleMarker[i].Location - playerHarry.Location);
				iClosestHole = i;
			}
		}

		aLastHole = HoleMarker[iClosestHole];
	}
	//else
	//{
	//	iLastHole =    CurrentHole;
	//	iClosestHole = CurrentHole;
	//
	//	iClosestHole++;
	//	if( iClosestHole >= NumHoles )
	//		iClosestHole = 0;
	//}

	CurrentHole = iClosestHole;

	SetLocation( HoleMarker[CurrentHole].Location );
	//SetRotation( HoleMarker[CurrentHole].Rotation );
	r = rotator((playerHarry.Location - Location)*vect(1,1,0));
	r.yaw += RandRange(-OutHoleMaxRandYaw, OutHoleMaxRandYaw);
	SetRotation( r );
	DesiredRotation = r;

	playerHarry.ClientMessage("Basil moved to hole:"$CurrentHole);

	//else
	//if( bClosestHole  ||  !bBasilCameOutFirstHoleAlready )
	//{
	//	//Make sure this is on
	//	bBasilCameOutFirstHoleAlready = true;
	//
	//	//Now try moving just to the closest hole
	//	ClosestHoleDist = 1000000;
	//
	//	ForEach AllActors( class'actor', a, HoleMarkerCommonTag )
	//	{
	//		if( VSize2d(a.Location - playerHarry.Location) < ClosestHoleDist )
	//		{
	//			ClosestHoleDist = VSize2d(a.Location - playerHarry.Location);
	//			ClosestHole = a;
	//		}
	//	}
	//
	//	//If same as last hole, find the two next closest holes, and randomly pick one.
	//	if( ClosestHole == aLastHole )
	//	{
	//		//I dont care how inneficient this is...
	//		ClosestHoleDist = 1000000;
	//		ForEach AllActors( class'actor', a, HoleMarkerCommonTag )
	//		{
	//			if(   a != ClosestHole
	//			   && VSize2d(a.Location - playerHarry.Location) < ClosestHoleDist
	//			  )
	//			{
	//				ClosestHoleDist = VSize2d(a.Location - playerHarry.Location);
	//				Holes[0] = a;
	//			}
	//		}
	//
	//		ClosestHoleDist = 1000000;
	//		ForEach AllActors( class'actor', a, HoleMarkerCommonTag )
	//		{
	//			if(   a != ClosestHole
	//			   && a != Holes[0]
	//			   && VSize2d(a.Location - playerHarry.Location) < ClosestHoleDist
	//			  )
	//			{
	//				ClosestHoleDist = VSize2d(a.Location - playerHarry.Location);
	//				Holes[1] = a;
	//			}
	//		}
	//
	//		ClosestHole = Holes[ Rand(2) ];
	//	}
	//
	//	a = ClosestHole;
	//	aLastHole = ClosestHole;
	//}
	//else
	//{
	//	vDir = normal( vector(playerHarry.Rotation) * vect(1,1,0) );
	//
	//	ForEach AllActors(class'actor', a, HoleMarkerCommonTag)
	//	{
	//		Holes[ NumHoles++ ] = a;
	//
	//		if( ( vDir  dot  normal((a.Location - playerHarry.Location) * vect(1,1,0)) )  >  0.5 ) //what is the fov, anyways?
	//			VisibleHoles[ NumVisibleHoles++ ] = a;
	//	}
	//
	//	if( NumHoles > 20 )
	//		playerHarry.ClientMessage("*************** ERROR ERROR ERROR: Too many holes...");
	//
	//	playerHarry.ClientMessage("NumHoles:"$NumHoles$" NumVHoles:"$NumVisibleHoles);
	//
	//	//If there are no visible holes, or just randomly sometimes, then just randomly pick one.
	//	if( NumVisibleHoles == 0  ||  FRand() < 0.2 )
	//		a = Holes[ Rand(NumHoles) ];
	//	else // There are visible holes, randomly pick one
	//		a = VisibleHoles[ Rand(NumVisibleHoles) ];
	//}
}

//******************************************************************************************
state stateSpaz
{
  Begin:
	TempYawSave = ActualYaw;

	if( ActualYaw < 0 )
	{
		PlayHissSound();
		RotateTo( 110, , HeadSpazSpeed );
		do { Sleep(0.001); } until( ActualYaw == DesiredYaw );

		PlayHissSound();
		RotateTo( -110, , HeadSpazSpeed );
		do { Sleep(0.001); } until( ActualYaw == DesiredYaw );

		//RotateTo( 110, , HeadSpazSpeed );
		//do { Sleep(0.001); } until( ActualYaw == DesiredYaw );
	}
	else
	{
		PlayHissSound();
		RotateTo( -110, , HeadSpazSpeed );
		do { Sleep(0.001); } until( ActualYaw == DesiredYaw );

		PlayHissSound();
		RotateTo( 110, , HeadSpazSpeed );
		do { Sleep(0.001); } until( ActualYaw == DesiredYaw );

		//RotateTo( -110, , HeadSpazSpeed );
		//do { Sleep(0.001); } until( ActualYaw == DesiredYaw );
	}

	PlayHissSound();
	RotateTo( TempYawSave, , HeadSpazSpeed );
	do { Sleep(0.001); } until( ActualYaw == DesiredYaw );

	SetAttackTimer();// = AttackPeriod / 2;
	AttackTimer /= 2;

	GotoState( WaitingState );
}

//******************************************************************************************
function PlayHissSound()
{
	local sound snd;

	switch( Rand(2) )
	{
		case 0:   snd = sound'HPSounds.Critters_sfx.basilisk_hiss_short1';    break;
		case 1:   snd = sound'HPSounds.Critters_sfx.basilisk_hiss_short2';    break;
	}

	_BasiliskHeadColObj.PlaySound( snd, SLOT_None, [Radius]BasilSoundRadius, [Pitch]RandRange(0.8,1.2) );
}

//******************************************************************************************************
//******************************************************************************************************
//state stateEyeSpell
//{
//	function bool BasilAcksHit(baseSpell spell)	{ return false;	}
//	function float GetBasilDamageScalar() { return 1.0; }
//
//	function BasilHitBySpell( baseSpell spell, vector HitLocation )
//	{
//		Global.BasilHitBySpell( spell, HitLocation );
//		if( spell.Damage >= MinDamageToBotherBasil )
//		{
//			bGotHitDuringWarmUp = true;
//			GotoState('stateHitThenEyeSpell');
//		}
//	}
//
//  Begin:
//	//Play some sound, anim
//	//Sleep( 0.25 );
//
//	SnakeLoopAnim( 'stare', 1.0, 1.0 );
//
//	if( bGotHitDuringIdle )
//		TempFloat = b1_EyeShootWarningEnd / 2;
//	else
//		TempFloat = b1_EyeShootWarningStart + (b1_EyeShootWarningEnd-b1_EyeShootWarningStart)*(1.0-Health/100.0);
//	
//	EyeGlowL.Glow( TempFloat );
//	EyeGlowR.Glow( TempFloat );
//
//	//_BasiliskHeadColObj.PlaySound(sound'HPSounds.critters_sfx.Basilisk_eyes_up&shoot', [Radius]BasilSoundRadius );
//
//	//do { Sleep(0.00001); } until( _AnimChannel.bAnimDone );
//	Sleep( TempFloat );
//	GotoState('stateEyeSpellFire');
//}
//
////*** *** *** *** *** *** *** *** *** *** *** *** *** *** *** *** 
//state stateHitThenEyeSpell
//{
//	function bool BasilAcksHit(baseSpell spell)	{ return false;	}
//
//  Begin:
//	SnakePlayAnim( 'InPain', 0.75, 1.0 );
//	playerHarry.ShakeView( 1.5, 100, 100 );
//	_BasiliskHeadColObj.PlaySound(sound'HPSounds.critters_sfx.Basilisk_scream_death', SLOT_None, [Radius]BasilSoundRadius );
//	do { Sleep(0.00001); } until( _AnimChannel.bAnimDone );
//	GotoState('stateEyeSpellFire');
//}
//
////*** *** *** *** *** *** *** *** *** *** *** *** *** *** *** *** 
//state stateEyeSpellFire
//{
//	function bool BasilAcksHit(baseSpell spell)
//	{
//		return false;
//	}
//
//  Begin:
//	if(   bGotHitDuringWarmUp  &&  !bGotHitDuringIdle //VSize(playerHarry.Velocity) >  20
//	   //|| bGotHitDuringWarmUp  &&  !bGotHitDuringIdle //VSize(playerHarry.Velocity) <= 20  &&  FRand() < 0.5
//	  )
//	{
//		playerHarry.ClientMessage("Miss Harry");
//		//RotateTo( DegreeRotToHarry() + 10*(Rand(2)*2-1), 0.2 );
//		CastEyeSpell(true);
//	}
//	else
//	{
//		playerHarry.ClientMessage("DONT Miss Harry");
//		//RotateToHarry(0.2);
//		CastEyeSpell(false);
//	}
//
//	//Sleep( 0.5 );
//
//	SetAttackTimer();
//	GotoState( WaitingState );
//}
//

//******************************************************************************************************
//******************************************************************************************************
state stateEyeSpell
{
	function bool BasilAcksHit(baseSpell spell)	{ return false; }//if( spell.Damage >= MinDamageToBotherBasil ) return true;	else return false;}

	//function float GetBasilDamageScalar() { return 1.0; }

	function BasilHitBySpell( baseSpell spell, vector HitLocation )
	{
		Global.BasilHitBySpell( spell, HitLocation );
		if(   //spell.Damage >= MinDamageToBotherBasil
		   /*&&*/ AttackAttemptCount < 3
		  )
		{
		//	bGotHitDuringWarmUp = true;
			GotoState('stateHitThenEyeSpell');
		}
	}

	function BeginState()
	{
		AttackAttemptCount++;
	}

  Begin:

	//Play some sound, anim
	//Sleep( 0.25 );

	SnakeLoopAnim( 'stare', 1.0, 1.0 );

	TempFloat = b1_EyeShootWarningStart;// + (b1_EyeShootWarningEnd-b1_EyeShootWarningStart)*(1.0-Health/100.0);
	
	EyeGlowL.Glow( TempFloat );
	EyeGlowR.Glow( TempFloat );

	switch( Rand(3) )
	{	case 0:   _BasiliskHeadColObj.PlaySound(sound'HPSounds.critters_sfx.Basilisk_Eyes_powerup1', SLOT_NONE, [Radius]BasilSoundRadius );   break;
		case 1:   _BasiliskHeadColObj.PlaySound(sound'HPSounds.critters_sfx.Basilisk_Eyes_powerup2', SLOT_NONE, [Radius]BasilSoundRadius );   break;
		case 2:   _BasiliskHeadColObj.PlaySound(sound'HPSounds.critters_sfx.Basilisk_Eyes_powerup3', SLOT_NONE, [Radius]BasilSoundRadius );   break;
	}

	AttackTimer = TempFloat/2;

	do
	{
		//SnakeLoopAnim( 'idle', RandRange(0.5,0.7), 0.7 );
		RotateTo( DegreeRotToActor(aEyeTarget), , 40 );
		do { Sleep(0.001); } until( ActualYaw == DesiredYaw );
	}until( AttackTimer <= 0 );

	//do { Sleep(0.00001); } until( _AnimChannel.bAnimDone );
	//Sleep( TempFloat/2 );

	GotoState('stateEyeSpellFire');
}

//*** *** *** *** *** *** *** *** *** *** *** *** *** *** *** *** 
state stateHitThenEyeSpell
{
	function bool BasilAcksHit(baseSpell spell)	{ return false;	}

	function Tick(float dtime)
	{
		local rotator   r;
		local vector    v, n;
		local float     d;

		Global.Tick(dtime);

		if( EyeBeam1 == none )
			return;

		if( EyeBeam1.DrawScale > 0 )
		{
			EyeBeam1.DrawScale -= 0.666 * dtime;
			EyeBeam2.DrawScale -= 0.666 * dtime;

			if( EyeBeam1.DrawScale <= 0 )
				StopEyeGlow();
		}

		if( EyeBeam1 == none )  //in case it's now gone from the StopEyeGlow
			return;

		EyeBeam1.SetLocation( BonePos( 'Bone144' ) );
		EyeBeam2.SetLocation( BonePos( 'Bone118' ) );

		EyeBeam1.SetRotation( _BasiliskHeadColObj.rotation );
		EyeBeam2.SetRotation( _BasiliskHeadColObj.rotation );
	}
	
	function EndState()
	{
		//Just make sure this happens
		StopEyeGlow();
	}

  Begin:
	EyeGlowL.Glow( 0 );
	EyeGlowR.Glow( 0 );

	SnakePlayAnim( 'InPain', 0.75, 1.0 );
	playerHarry.ShakeView( 1.5, 100, 100 );
	_BasiliskHeadColObj.PlaySound(sound'HPSounds.critters_sfx.Basilisk_scream_death', SLOT_None, [Radius]BasilSoundRadius, [Pitch]RandRange(0.9,1.1) );

	do { Sleep(0.00001); } until( _AnimChannel.bAnimDone );

	//If harry's in range, lunge at him now
	if( VSize2d( playerHarry.Location - Location )  <  HeadAttackFarthest + 20 )
	{
		playerHarry.ClientMessage("Harry in range for lunge  -  Hit then eye spell");
		bGotHitDuringWarmUp = false;
		AttackAttemptCount = 0;
		StopEyeGlow();
		GotoState('stateAttack');
	}

	GotoState('stateEyeSpell');//Fire');
}

//*** *** *** *** *** *** *** *** *** *** *** *** *** *** *** *** 
state stateEyeSpellFire
{
	function bool BasilAcksHit(baseSpell spell) { return false; }

	function BasilHitBySpell( baseSpell spell, vector HitLocation )
	{
		Global.BasilHitBySpell( spell, HitLocation );
		if(   //spell.Damage >= MinDamageToBotherBasil
		   /*&&*/ AttackAttemptCount < 3
		  )
		{
		//	bGotHitDuringWarmUp = true;
			GotoState('stateHitThenEyeSpell');
		}
	}

	function Tick(float dtime)
	{
		local rotator   r;
		local vector    v, n;
		local float     d;
		local bool      bHarryInfrontOfBeamEnd;

		Global.Tick(dtime);

		if( EyeBeam1 == none )
			return;

		BeamHurtHarryTimer += dtime;

		if( BeamGrowTime < BeamGrowTimeSpan )
		{
			BeamGrowTime += dtime;
			if( BeamGrowTime >= BeamGrowTimeSpan )
				BeamGrowTime = BeamGrowTimeSpan;

			EyeBeam1.DrawScale = 2.0 * BeamGrowTime / BeamGrowTimeSpan;
			//EyeBeam1.Wideness = FMin( 255.0, 128.0/EyeBeam1.DrawScale );
			EyeBeam2.DrawScale = EyeBeam1.DrawScale;
			//EyeBeam2.Wideness = EyeBeam1.Wideness;
		}

		EyeBeam1.SetLocation( BonePos( 'Bone144' ) );
		EyeBeam2.SetLocation( BonePos( 'Bone118' ) );

		//r.yaw =   ActualYaw * 65536.0 / 360.0  +  rotation.Yaw; //convert this to real yaw.
		//v = Location + vector(r)*TempHarryDist;
		//v.z = playerHarry.Location.z;
		//r.pitch = rotator(v - EyeBeam1.Location).pitch;
		
		//Look at the new eye target
		r = rotator( aEyeTarget.Location - EyeBeam1.Location );

		EyeBeam1.SetRotation( r );
		EyeBeam2.SetRotation( r );
		
		//Do harry collision
		if( BeamHurtHarryTimer > 0.5 )
		{
			d = vsize( playerHarry.Location - EyeBeam1.Location );
			d = d / BeamLength  *  playerHarry.CollisionRadius;
			n = vector(r);
			v = (playerHarry.Location - EyeBeam1.Location) cross n;
			if( (n  dot  (playerHarry.Location - n * BeamLength * EyeBeam1.DrawScale)) < 0 )
				bHarryInfrontOfBeamEnd = true;
			if(   vsize(v) < d  &&  bHarryInfrontOfBeamEnd )
			{
				playerHarry.TakeDamage( BeamDamage, self, playerHarry.Location, vect(0,0,0), '');
				BeamHurtHarryTimer = 0;
			}
			else
			{
				v = (playerHarry.Location - EyeBeam2.Location) cross n;
				if(   vsize(v) < d  &&  bHarryInfrontOfBeamEnd )
				{
					playerHarry.TakeDamage( BeamDamage, self, playerHarry.Location, vect(0,0,0), '');
					BeamHurtHarryTimer = 0;
				}
			}
		}

	}

	function EndState()
	{
		//StopEyeGlow();
		_BasiliskHeadColObj.StopSound(sound'HPSounds.critters_sfx.BAS_eye_beam_loop', SLOT_Interact, 1.0 );
	}

  Begin:

	RotateTo( DegreeRotToActor(aEyeTarget), , 40 );

	EyeBeam1 = spawn( class'SnakeBeam', self, [SpawnLocation]BonePos( 'Bone144' ) );
	EyeBeam1.DrawScale = 0;
	EyeBeam2 = spawn( class'SnakeBeam', self, [SpawnLocation]BonePos( 'Bone118' ) );
	EyeBeam2.DrawScale = 0;

	BeamGrowTime = 0.01;
	BeamGrowTimeSpan = 0.5;
	BeamLife = 0;
	BeamHurtHarryTimer = 10000;

	//_BasiliskHeadColObj.PlaySound(sound'HPSounds.Adv9Aragog.SS_ARA_BreathOut_0001', SLOT_None, [Radius]BasilSoundRadius );
	_BasiliskHeadColObj.PlaySound(sound'HPSounds.critters_sfx.BAS_eye_beam_loop', SLOT_Interact, [Radius]BasilSoundRadius, [Loop]true );

	do
	{
		SetAttackTimer();

		do
		{
			RotateTo( DegreeRotToActor(aEyeTarget), , 40 );
			do { Sleep(0.001); } until( ActualYaw == DesiredYaw );
		}until( AttackTimer <= 0 );

		if( VSize2d( playerHarry.Location - Location )  <  HeadAttackFarthest + 20 )
		{
			playerHarry.ClientMessage("Harry in range for lunge  -  eye spell");

			bGotHitDuringWarmUp = false;
			AttackAttemptCount = 0;
			StopEyeGlow();
			GotoState('stateAttack');
		}

	}until( false );

	//TempFloat = 0.25;

	////Try and sweep harry off the ledge.
	//TempFloat2 = DegreeRotToHarry();
	//TempBool = TempFloat2 < 0;
	//TempHarryDist = VSize2d( playerHarry.Location - Location );
	//
	//if( TempBool )
	//	RotateTo( TempFloat2 + BeamYawStartOffset_Degrees*0.5, TempFloat, 0 );
	//else
	//	RotateTo( TempFloat2 - BeamYawStartOffset_Degrees*0.5, TempFloat, 0 );
	//
	//Sleep( TempFloat );
	//
	//if( TempBool )
	//	RotateTo( TempFloat2 - BeamYawStartOffset_Degrees*1.5, 0, BeamYawRate_DegreesPerSec );
	//else
	//	RotateTo( TempFloat2 + BeamYawStartOffset_Degrees*1.5, 0, BeamYawRate_DegreesPerSec );
	//
	//Sleep( BeamLifeSpan );

	//GotoState('stateEyeSpellFire');

	SetAttackTimer();
	GotoState( WaitingState );
}

//**************************************************************************************
function StopEyeGlow()
{
	if( EyeBeam1 != none )
	{
		EyeBeam1.destroy();
		EyeBeam2.destroy();
		EyeBeam1 = none;
		EyeBeam2 = none;
	}

	EyeGlowL.Glow( 0 );
	EyeGlowR.Glow( 0 );
}

//******************************************************************************************************
//******************************************************************************************************
state stateAttack
{
	function bool BasilAcksHit(baseSpell spell)	{ return false;	}
	//function float GetBasilDamageScalar() { return 1.0; }

	function BasilHitBySpell( baseSpell spell, vector HitLocation )
	{
		Global.BasilHitBySpell( spell, HitLocation );
		if(   spell.Damage >= MinDamageToBotherBasil
		   && AttackAttemptCount < 2
		  )
		{
			bGotHitDuringWarmUp = true;
			GotoState('stateHitThenAttack');
		}
	}

	function BeginState()
	{
		AttackAttemptCount++;
	}

  Begin:
	//Turn towards where harry is, and attack at the same time, and make them take the same amount of time
	RotateToHarry( 1 );

	//if( bGotHitDuringIdle )
	//	TempFloat = 3;
	//else
	//	TempFloat = 1.0  +  2.0 * (1.0-Health/100.0);
	TempFloat = b1_HeadRoarAnimRate_Start + (b1_HeadRoarAnimRate_End - b1_HeadRoarAnimRate_Start)*(1.0-Health/100.0);

	SnakePlayAnim('taunt', TempFloat, 2.0);
	Sleep(0.2);
	_BasiliskHeadColObj.PlaySound(sound'HPSounds.critters_sfx.Basilisk_roar', SLOT_None, [Radius]BasilSoundRadius, [Pitch]RandRange(0.9,1.1) );
	playerHarry.ShakeView( 1.5, 50, 50 );
	do { Sleep(0.00001); } until( _AnimChannel.bAnimDone );
	GotoState('stateAttackLunge');
}

//*** *** *** *** *** *** *** *** *** *** *** *** *** *** *** *** *** 
state stateHitThenAttack
{
	function bool BasilAcksHit(baseSpell spell)	{ return false;	}

  Begin:
	SnakePlayAnim( 'InPain', 0.75, 1.0 );
	playerHarry.ShakeView( 1.5, 75, 75 );
	_BasiliskHeadColObj.PlaySound(sound'HPSounds.critters_sfx.Basilisk_scream_death', SLOT_None, [Radius]BasilSoundRadius, [Pitch]RandRange(0.9,1.1) );
	do { Sleep(0.00001); } until( _AnimChannel.bAnimDone );
	GotoState('stateAttackLunge');
}

//*** *** *** *** *** *** *** *** *** *** *** *** *** *** *** *** *** 
state stateAttackLunge
{
	function bool BasilAcksHit(baseSpell spell)	{ return false;	}
	//function float GetBasilDamageScalar() { return 1.0; }

  Begin:
	if(   bGotHitDuringWarmUp  &&  !bGotHitDuringIdle//!VSize(playerHarry.Velocity) >  20
	   //|| bGotHitDuringWarmUp  &&  VSize(playerHarry.Velocity) <= 20  &&  FRand() < 0.5
	  )
	{
		playerHarry.ClientMessage("Miss Harry");
		RotateTo( DegreeRotToHarry() + DegRotAwayFromHarry(true), 0.2 );

		PlayLungeAnim();
		playerHarry.ShakeView( 0.5, 50, 50 );
		PlayLungeSound();
	}
	else
	{
		playerHarry.ClientMessage("DONT Miss Harry");
		RotateToHarry(0.2);

		PlayLungeAnim();
		playerHarry.ShakeView( 0.5, 50, 50 );
		PlayLungeSound();

		Sleep( 0.2 );
		RotateToHarry(0.2);
	}

	do { Sleep(0.00001); } until( _AnimChannel.bAnimDone );

	//if( FRand() < 0.15 )
	//	Goto 'Begin';

	SetAttackTimer();
	GotoState( WaitingState );
}

//******************************************************************************************************
//******************************************************************************************************
function float DegRotAwayFromHarry(optional bool bMissHarry)
{
	local float  z;

	z = (normal(playerHarry.location-location) cross playerHarry.velocity).z;
	z = -z * 0.05; // magic multiplier

	if( bMissHarry )
		z += (Rand(2)*2-1) * 23.0; //magic miss multiplier

	return z;//FClamp( z, -30.0, 30.0 ) / 30.0;
}

//******************************************************************************************
function bool AnimSequenceIsLunge()
{
	local int i;
	local bool bWasLunge;

	return true;

	//for( i = 0; i < HeadAttackCount; i++ )
	//	if( _AnimChannel.AnimSequence == HeadAttackAnimName[i] )
	//		return true;

	//for( i = 0; i < HeadAttackCount; i++ )
	//	if( _AnimChannel.AnimSequence == HeadAttack2AnimName[i] )
	//		return true;
	
	return false;
}

//******************************************************************************************
function PlayRoarSound()
{
	local sound snd;

	//PlayMovingThroughWallSound();

	//switch( Rand(6
	//_BasiliskHeadColObj.PlaySound(sound'HPSounds.Hub1_sfx.Basil3Attack00', [Radius]BasilSoundRadius );
}

//******************************************************************************************
/*
state stateIdleThenAttack
{
  Begin:
	ActualYaw = 0;
	PlayAnim( 'orient', 0, 0.2, AT_Combine );

	//Turn towards where harry is
	RotateToHarry( , RandRange(30,60) );

	SnakePlayAnim('Birth');
	do { Sleep(0.00001); } until( _AnimChannel.bAnimDone );

  Loop1:
	SnakeLoopAnim('idle',0.3,3);

	Sleep( 0.5 );

	RotateTo(ActualYaw-RandRange(10,40), , RandRange(30,60) );
	do { Sleep(0.001); } until( ActualYaw == DesiredYaw );

	Sleep( 0.5 );

	RotateTo(ActualYaw+RandRange(10,40), , RandRange(30,60) );
	do { Sleep(0.001); } until( ActualYaw == DesiredYaw );

	Sleep( 0.5 );

  Loop2:
	RotateToHarry( , RandRange(30,60) );
	switch( Rand(6) )
	{	case 0:   TempSound = sound'HPSounds.critters_sfx.Basilisk_attack1';    break;
		case 1:   TempSound = sound'HPSounds.critters_sfx.Basilisk_attack2';    break;
		case 2:   TempSound = sound'HPSounds.critters_sfx.Basilisk_attack3';    break;
		case 3:   TempSound = sound'HPSounds.critters_sfx.Basilisk_attack4';    break;
		case 4:   TempSound = sound'HPSounds.critters_sfx.Basilisk_attack5';    break;
		case 5:   TempSound = sound'HPSounds.critters_sfx.Basilisk_attack6';    break;
	}
	_BasiliskHeadColObj.PlaySound( TempSound, [Radius]BasilSoundRadius, [Pitch]RandRange(0.8,1.2) );

	PlayLungeAnim();
	do { Sleep(0.00001); } until( _AnimChannel.bAnimDone );

	//Random, see if we should do another attack
	if( FRand() < 0.3 )
		Goto 'Loop2';

	GotoState( 'stateRetreat' );
}
*/
//*********************************************************************************************************************
function PlayLungeSound()
{
	local sound TempSound;

	switch( Rand(6) )
	{	case 0:   TempSound = sound'HPSounds.critters_sfx.Basilisk_attack1';    break;
		case 1:   TempSound = sound'HPSounds.critters_sfx.Basilisk_attack2';    break;
		case 2:   TempSound = sound'HPSounds.critters_sfx.Basilisk_attack3';    break;
		case 3:   TempSound = sound'HPSounds.critters_sfx.Basilisk_attack4';    break;
		case 4:   TempSound = sound'HPSounds.critters_sfx.Basilisk_attack5';    break;
		case 5:   TempSound = sound'HPSounds.critters_sfx.Basilisk_attack6';    break;
	}
	_BasiliskHeadColObj.PlaySound( TempSound, SLOT_None, [Radius]BasilSoundRadius, [Pitch]RandRange(0.8,1.2) );
}

//*********************************************************************************************************************
function PlayLungeAnim()
{
	local int  i;
	local int  w;
	local name n;

	//Get the right anim based on harry's distance from the origin

	//Distance between two adjacent anim lunges
	w = (HeadAttackFarthest - HeadAttackNearest) / (HeadAttackCount - 1);
	i = ( VSize2d(playerHarry.Location - Location) - HeadAttackNearest + w/2 + w*0.3)   /   w;  //Add on an extra w*0.3 so the head lunges "further".
	i = Clamp( i, 0, HeadAttackCount-1 );

	//if( i == 1 )
	//	n = 'lunge_1b';
	//else
		n = name("lunge_"$i+1);
playerHarry.ClientMessage("Play LungeAnim:"$n);	
	SnakePlayAnim( n, HeadAttackAnimRate, 0.2 );
}

//*********************************************************************************************************************
//part can be 1, 2, 3, or 4   4 is the roar
function PlayLungeFromHoleAnim(int part)
{
	local int    i;
	local int    w;
	local string s;

	//Get the right anim based on harry's distance from the origin

	//Distance between two adjacent anim lunges
	w = (HeadAttackFarthest - HeadAttackNearest) / (HeadAttackCount - 1);
	i = ( VSize2d(playerHarry.Location - Location) - HeadAttackNearest + w/2 + w/4)   /   w;  //Add on an extra w/4 so the head lunges "further".
	i = Clamp( i, 0, HeadAttackCount-1 );
	
	//SnakePlayAnim( HeadAttack2AnimName[i], 1.0, 0.2 );
	if( part < 4 )
		s = "snap_"$i+1$"_"$part;
	else
		s = "roar_"$i+1;

	SnakePlayAnim( name(s), HeadAttackAnimRate, 0.2 );
	playerHarry.ClientMessage("Attack anim:"$s);
}

//*********************************************************************************************************************
state statePodiumAttack
{
	function BeginState()
	{
		local float deg;

		deg = DegreeRotToHarry();

		if( deg < 0 )
			SnakePlayAnim('lungeright', HeadAttackAnimRate, 0.2);
		else
			SnakePlayAnim('lungeleft', HeadAttackAnimRate, 0.2);
	}

  Begin:
	//FinishAnim();
	Sleep( 2 );
	GotoState( WaitingState );
}

//******************************************************************************************
state stateHit
{
	function BeginState()
	{
		if( bDidFirstBattle )
			GotoState( 'stateHit_2_' );
	}

	function bool BasilAcksHit(baseSpell spell)
	{
		return false;
	}

  Begin:
	//Play a hit anim
	AnimFrame = 0;
	SnakePlayAnim( 'InPain', 0.75, 1.0 );
	
	//switch( Rand(3) )
	//{	case 0:   TempSound = sound'HPSounds.critters_sfx.Basilisk_ouch1';    break;
	//	case 1:   TempSound = sound'HPSounds.critters_sfx.Basilisk_ouch2';    break;
	//	case 2:   TempSound = sound'HPSounds.critters_sfx.Basilisk_ouch3';    break;
	//}
	//PlaySound( TempSound, SLOT_None, [Radius]BasilSoundRadius );
	_BasiliskHeadColObj.PlaySound(sound'HPSounds.critters_sfx.Basilisk_scream_death', SLOT_None, [Radius]BasilSoundRadius, [Pitch]RandRange(0.9,1.1) );
	playerHarry.ShakeView( 1.5, 50, 50 );

	RotateTo( ActualYaw-20, , 160 );
	do { Sleep(0.001); } until( ActualYaw == DesiredYaw );

	for( TempFloat = 40; Abs(TempFloat) > 1.125; TempFloat *= -0.65 )
	{
		RotateTo( ActualYaw+TempFloat, 0.1 );//, 160 );
		do { Sleep(0.001); } until( ActualYaw == DesiredYaw );
	}

	do { Sleep(0.00001); } until( _AnimChannel.bAnimDone );

	GotoState( BasilNextState );
}

//******************************************************************************************
state stateHit_2_
{
	function Tick(float dtime)
	{
		global.tick( dtime );

		if( Rand(12)==0 )
		{
			DesiredRotation.Yaw += (Rand(2)*2-1) * 4000;
			CastSpitSpell( false );
		}
	}

	function bool BasilAcksHit(baseSpell spell) { return false; }
	function bool ShouldDodgeSpell()            { return false; }

  Begin:

	//switch( Rand(3) )
	//{	case 0:   TempSound = sound'HPSounds.critters_sfx.Basilisk_ouch1';    break;
	//	case 1:   TempSound = sound'HPSounds.critters_sfx.Basilisk_ouch2';    break;
	//	case 2:   TempSound = sound'HPSounds.critters_sfx.Basilisk_ouch3';    break;
	//}
	_BasiliskHeadColObj.PlaySound(sound'HPSounds.critters_sfx.imp_ouch_02', SLOT_None, [Radius]BasilSoundRadius, [Pitch]0.3333 );
	_BasiliskHeadColObj.PlaySound(sound'HPSounds.critters_sfx.SPI_medium_hiss2', SLOT_None, [Radius]BasilSoundRadius );
	_BasiliskHeadColObj.PlaySound(sound'HPSounds.critters_sfx.Basilisk_scream_death', SLOT_Talk, [Radius]BasilSoundRadius );
	//PlaySound( TempSound, SLOT_None, [Radius]BasilSoundRadius );

	//Play a hit anim
	PlayAnim( 'React', 1.0, 2.0 );
	playerHarry.ShakeView( 2.0, 125, 125 );
	FinishAnim();

	if( bAttackAfterBeingHit )
	{
		bAttackAfterBeingHit = false;
		GotoState('stateAttackFromHole');
	}

	//if( BasilNextState2 != '' )
	//{
	//	TempName = BasilNextState2;
	//	BasilNextState2 = '';
	//	GotoState( TempName );
	//}

	GotoState( BasilNextState );
}

//******************************************************************************************
function SetAttackTimer()
{
	local float t;

	if( !bDidFirstBattle )
	{
		t = b1_TimeBetweenAttackStart + (b1_TimeBetweenAttackStart - b1_TimeBetweenAttackEnd) * Health/100;
		AttackTimer = t + RandRange(-t/3, t/3);
	}
	else
	{
		t = RandRange(1.0,AttackPeriod2);
		AttackTimer = t;// + RandRange(-t/3, t/3);
	}
}

//******************************************************************************************
function CastEyeSpell(bool bMissHarry)
{
	local BasiliskSpell spell;
	local vector        v;
	local float         d;
	local float         angleSpread;
	local float         angleOffset;

	angleSpread = 3;
	bEyeShootToggle = !bEyeShootToggle;
	if( bEyeShootToggle )
		angleOffset = 0;
	else
		angleOffset = -angleSpread;

	if( bMissHarry )
		angleOffset += DegRotAwayFromHarry() + 17.0*(Rand(2)*2-1);
	else
		angleOffset -= DegRotAwayFromHarry();

	d = VSize(playerHarry.Location - _BasiliskHeadColObj.location);

	v = normal( playerHarry.Location + vec(0,0,playerHarry.BaseEyeHeight/3)  -  BonePos('Bone144') );
	spell = spawn( class'BasiliskSpell', [SpawnLocation]BonePos('Bone144') );
	//spell = spawn( class'BasiliskSpell', [SpawnLocation]_BasiliskHeadColObj.location );
	spell.Init(  v
	           , SpellDamageAmount
	           , SpellInitialDrawScale
	           , SpellEndDrawScale
	           , d + 200 //need to add some, otherwise it wont hit him half the time.
	           , SpellStartSpeed
	           , SpellEndSpeed
	           , angleOffset*65536/360
	          );
	//spell.RotationRate = rot(0,0,8000);
	spell.bActive = true;


	v = normal( playerHarry.Location + vec(0,0,playerHarry.BaseEyeHeight/3)  -  BonePos('Bone118') );
	spell = spawn( class'BasiliskSpell', [SpawnLocation]BonePos('Bone118') );
	//spell = spawn( class'BasiliskSpell', [SpawnLocation]_BasiliskHeadColObj.location );
	spell.Init(  v
			   , SpellDamageAmount
			   , SpellInitialDrawScale
			   , SpellEndDrawScale
	           , d + 200//FClamp(d, 200, SpellMaxTravelDistance)
			   , SpellStartSpeed
			   , SpellEndSpeed
	           , (angleOffset + angleSpread)*65536/360
			  );
	//spell.RotationRate = rot(0,0,-8000);
	spell.bActive = true;

	spell.PlaySound(sound'HPSounds.Critters_sfx.Basilisk_eyes_shoot', SLOT_None, [Radius]BasilSoundRadius );
}

//******************************************************************************************
state stateRetreat
{
	function bool ShouldDodgeSpell()            { return false; }
	function bool BasilAcksHit(baseSpell spell) { return false; }

  Begin:
	//RotateTo( 0, , 150 );

	PlayAnim('GoBack', 1, 0.3);
	//do { Sleep(0.00001); } until( _AnimChannel.bAnimDone );
	FinishAnim();
	HideBasil();
	GotoState( WaitingState );

}

//******************************************************************************************
function int GetRealHeadYaw()
{
	local int  i;
	i = Rotation.Yaw + ActualYaw*65536/360;
	return i & 65535;
}

//******************************************************************************************
function SetRandomDesiredYawOffset( int YawOffset )
{
	local float  yaw;

	yaw = ActualYaw + RandRange( -YawOffset, YawOffset );

	if( yaw < -MaxHeadYaw )
		yaw = -MaxHeadYaw;
	else
	if( yaw >  MaxHeadYaw )
		yaw =  MaxHeadYaw;

	RotateTo( yaw );
}

//******************************************************************************************
//function bool HandleSpellFlipendo( vector vHitLocation )
function BasilHitBySpell( baseSpell spell, vector HitLocation )
{
	local int DamageAmount;
	local bool bDoStateHit;

	if( spellSwordFire(spell) == none )
		return;

	bDoStateHit = true;

	if( bIdleState )
		return;

	DamageAmount = spellSwordFire(spell).Damage;

	//playerHarry.ClientMessage("1 Basil Takes "$DamageAmount$" damage");

	//if( !bDidFirstBattle )
	//{
		//if( !BasilAcksHit( spell ) )
		//{
		//	bDoStateHit = false;
		//	DamageAmount *= 0.25;
		//}

		DamageAmount *= GetBasilDamageScalar();

	//}
	//else  //second half of battle
	//{
	//	//Also, for certain anims, dont goto state hit, also tone down the damage amount.
	//	if(   AnimSequence == 'birth'  ||  _AnimChannel.AnimSequence == 'birth'
	//	   || AnimSequence == 'retreat'||  _AnimChannel.AnimSequence == 'retreat'
	//	  )
	//	{
	//		bDoStateHit = false;
	//		DamageAmount = 1;
	//	}
	//}
	//playerHarry.ClientMessage("2 Basil Takes "$DamageAmount$" damage");
	Health -= DamageAmount;

	//if( bDidFirstBattle )
	//	Health = 0;

	if( Health <= 0 )
	{
		Health = 0;
		BeatBoss();
	}
	else
	{
		if( BasilAcksHit( spell ) )//bDoStateHit )
		{
			GotoState( 'stateHit' );
		}
		else
		{
			//At least play a sound acknowledging the hit.
			if( !bDidFirstBattle )
			{
				switch( Rand(3) )
				{	case 0:   TempSound = sound'HPSounds.critters_sfx.Basilisk_ouch1';    break;
					case 1:   TempSound = sound'HPSounds.critters_sfx.Basilisk_ouch2';    break;
					case 2:   TempSound = sound'HPSounds.critters_sfx.Basilisk_ouch3';    break;
				}
				PlaySound( TempSound, SLOT_None, [Radius]BasilSoundRadius );
			}

		}
	}

	playerHarry.ClientMessage(" Basil Health:"$Health);

	return;
}

//******************************************************************************************
function StartBossEncounter()
{
	super.StartBossEncounter();

	playerHarry.makeTarget();
	playerHarry.spellCursor.bSpellCursorAlwaysOn = true;
	playerHarry.bStrafe = 1;
	playerHarry.bAutoCenterCamera = false;
	playerHarry.cam.fDistanceScalarMin = 0.05;

	if( !bDidFirstBattle )
	{
		playerHarry.cam.fCurrentMinPitch = -4000;
		playerHarry.cam.fCurrentMaxPitch =  10000;
		playerHarry.AimRotOffset = rot(3500,0,0);

		aEyeTarget = playerHarry.spawn( class'ActorChaser', playerHarry );
		aEyeTarget.AirSpeed = BeamChaseSpeed;
		aEyeTarget.AccelRate = BeamChaseAccel;
	}
	else
	{
		playerHarry.cam.fCurrentMinPitch = -7000;
		playerHarry.cam.fCurrentMaxPitch =  7000;
		playerHarry.AimRotOffset = rot(0,0,0);
	}

}

//******************************************************************************************
function BeatBoss()
{
	local SnakeVenomPool  pool;
	local spellAcidSpit   spit;

	if( !bDidFirstBattle )
	{
		//Make sure eye beem stuff is off.
		StopEyeGlow();

		aEyeTarget.Destroy();
		aEyeTarget = none;

		SendDefeatedTrigger();
	}
	else
	{
		SendDefeatedTrigger2();
	}

	//Get rid / turn off harmfull stuff
	ForEach AllActors(class'SnakeVenomPool', pool)
		pool.iDamage = 0;
	ForEach AllActors(class'spellAcidSpit', spit)
		{ spit.bSpawnPool = false;      spit.iDamage = 0; }

	playerHarry.spellCursor.bSpellCursorAlwaysOn = false;
	playerHarry.spellCursor.EnableEmission( false );  //bug that I cant fix, so I'll do it here
	playerHarry.bAutoCenterCamera = true;
	playerHarry.TurnOffSpellCursor();

	playerHarry.StopBossEncounter();
	bDidFirstBattle = true;
	GotoState( 'stateIdle' );
	bIdleState = true;
}

//*********************************************************************************************************************
function ColObjTouch( actor other, GenericColObj ColObj )
{
	local sound snd;

	if( Harry(other) != none  &&  ColObj == _BasiliskHeadColObj )
	{
		playerHarry.ClientMessage(" Basil ColObjTouch:"$other.name);

		if( HarryDamageTimer > 0.333 )
		{
			HarryDamageTimer = 0;

			//Different damage and sounds based on whether head or not
			if( ColObj.bIsHead )
			{
				//PlaySound( sound'HPSounds.critters_sfx.basil_bite', [Radius]BasilSoundRadius );
				switch( Rand(5) ) { case 0: snd = sound'HPSounds.critters_sfx.pix_bite1'; break;
				                    case 1: snd = sound'HPSounds.critters_sfx.pix_bite2'; break;
				                    case 2: snd = sound'HPSounds.critters_sfx.pix_bite3'; break;
				                    case 3: snd = sound'HPSounds.critters_sfx.pix_bite4'; break;
				                    case 4: snd = sound'HPSounds.critters_sfx.pix_bite5'; break;
				                  }
				PlaySound( snd, SLOT_None, [Radius]BasilSoundRadius, [Pitch]RandRange(0.6,0.8) );
				Harry(other).TakeDamage( HeadDamage, self, ColObj.Location, vect(0,0,0), '');
				BasilMunchesHarry();
			}
			else
			{
				//PlaySound
				Harry(other).TakeDamage( TailDamage, self, ColObj.Location, vect(0,0,0), '');
			}
		}
	}
	else
	if( SpellSwordFire(other) != none )
	{
		playerHarry.ClientMessage(" Basil ColObjTouch:"$other.name);

		if( !bHidden )
		{
			if( ColObj == _BasiliskBreastColObj )
				SpellSwordFire(other).Damage *= BreastHitDamageMult;
			//playerHarry.ClientMessage("Basil Hit.  head:"$ColObj.bIsHead$" Damage:"$SpellSwordFire(other).Damage);
			BasilHitBySpell( baseSpell(other), other.Location );
		}

		//other.Destroy();
	}
}

//*********************************************************************************************************************
function MoveToRandomVisibleHole()
{
	local int    NumVisibleHoles;
	local int    NumHoles;
	local actor  Holes[20];
	local actor  VisibleHoles[20];
	local vector vDir;
	local actor  a;
	local float  ClosestNonVisibleHoleDist;
	local int    ClosestNonVisibleHoleIdx;

	ClosestNonVisibleHoleDist = 1000000;

	vDir = normal( vector(playerHarry.Rotation) * vect(1,1,0) );

	ForEach AllActors(class'actor', a, HoleMarkerCommonTag)
	{
		if( ( vDir dot normal((a.Location - playerHarry.Location) * vect(1,1,0)) )  >  0.5 ) //what is the fov, anyways?
		{
			VisibleHoles[ NumVisibleHoles++ ] = a;
		}
		else	//Hole isn't visible, see if it's the closest one yet
		{
			if( VSize2d(a.Location - playerHarry.Location)  <  ClosestNonVisibleHoleDist )
			{
				ClosestNonVisibleHoleDist = VSize2d(a.Location - playerHarry.Location);
				ClosestNonVisibleHoleIdx = NumHoles;
			}
		}			

		Holes[ NumHoles++ ] = a;
	}


	if( FRand() < 0.2 )
	{
		a = Holes[ Rand(NumHoles) ];
	}
	else
	if(   NumVisibleHoles == 0
	   || NumVisibleHoles == 1 && FRand() < 0.5 //Even if theres one visible hole, 50% of the time, pick the one closest to harry.
	  )
	{
		a = Holes[ ClosestNonVisibleHoleIdx ];
	}
	else // There are visible holes, randomly pick one
	{
		a = VisibleHoles[ Rand(NumVisibleHoles) ];
	}

	SetLocation( a.Location );
}

//******************************************************************************************
function TweakSetting(string s)
{
	local string  command;
	local string  v1;

	command = ParseDelimitedString( s, " ", 1, false );
	v1 = ParseDelimitedString( s, " ", 2, false );

	//playerHarry.ClientMessage("Boss TweakSetting:"$command);
	//playerHarry.ClientMessage("Boss TweakSetting:"$v1);

	if( command ~= "SpellStartSpeed" )
	{
		playerHarry.ClientMessage("Basil SpellStartSpeed:"$SpellStartSpeed);
		if( v1 != "" )
		{
			playerHarry.ClientMessage("      SpellStartSpeed:"$float(v1));
			SpellStartSpeed = float(v1);
		}			
	}
	else
	if( command ~= "CamZOffset" )
	{
		playerHarry.ClientMessage("Basil CamZOffset:"$CamZOffset);
		if( v1 != "" )
		{
			playerHarry.ClientMessage("      CamZOffset:"$float(v1));
			CamZOffset = float(v1);
		}			
	}
	else
	if( command ~= "EyeBeam" )
	{
		bUseEyeBeam = true;
	}
	else
	{
		super.TweakSetting( s );
	}
}

//******************************************************************************************
defaultproperties
{
	Drawscale=1.8

	Mesh=SkeletalMesh'HPModels.skBasiliskMesh'

	bIdleState=true;

	MaxHeadYaw=85
	HeadYawRate=25

	SlideDistance=30
	SlideSpeed=200

	//AttackPeriod=4
	AttackPeriod2=2.0

	HeadSpazSpeed=200

	MainBoneName="Ani_Bone08"

	HoleMarkerCommonTag="BasiliskHoleMarker"

	SpellDamageAmount=10
	SpellInitialDrawScale=2//0.5
	SpellEndDrawScale=2//4
	//SpellMaxTravelDistance=1500
	SpellStartSpeed=950 //1800
	SpellEndSpeed=155

	BreastHitDamageMult=0.3333

	AcidSpitYawSpread=40
	AcidSpitFreq=20
	AcidSpitCount=18
	AcidSpitTargetDistributionWidth=250

	AcidSpitChaseHarryYawSpread=4000

	HeadAttackNearest=192 //50
	HeadAttackFarthest=512 // 672 //465
	//HarryInRangeDist=530 //690 //470

	HeadAttackNearest_2=128
	HeadAttackFarthest_2=320

	HeadAttackAnimRate=0.7
	b1_HeadRoarAnimRate_Start=3.0;
	b1_HeadRoarAnimRate_End=0.5;
	//HeadAttackCount=4
	//HeadAttackAnimName(0)="lunge_1"
	//HeadAttackAnimName(1)="lunge_2"
	//HeadAttackAnimName(2)="lunge_3"

	//HeadAttack2AnimName(0)="snap_1"
	//HeadAttack2AnimName(1)="snap_2"
	//HeadAttack2AnimName(2)="snap_3"
	//HeadAttack2AnimName(3)="snap_4"

	HeadCollisionRadius=70

	BasilSoundRadius=1000000

	b1_TimeBetweenAttackStart=1
	b1_TimeBetweenAttackEnd=0.25

	b1_EyeShootWarningStart=2.5
	b1_EyeShootWarningEnd=1.5

	BeamLifeSpan=2
	BeamYawRate_DegreesPerSec=12
	BeamYawStartOffset_Degrees=15
	BeamLength=340
	BeamDamage=10
	
	//New vars
	BeamChaseSpeed=90
	BeamChaseAccel=150

	b2_SprayWarningAnimRate=2.0

	OutHoleMaxRandYaw=9000

	MinDamageToBotherBasil=3

	CentreOffset=(x=30,y=0,z=130)

	BasilSniffDistance=350

	SpellToHeadProximity=80

	CamXOffset=100
	CamZOffset=50

	bGestureFaceHorizOnly=true
}


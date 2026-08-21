
class Basilisk2 expands baseBasilisk;

var() name    HoleMarkerCommonTag;
//var() float   HoleLookAheadDistance;

var() float   IdleTimerStart;
//var   float   IdleTimer;

var() float   AttackTime;

var   vector  LastNoiseLoc;
var   float   TimeSinceLastHearNoise;
var   float   HarryMakingNoiseTimer;  //"How long" harry's been making noise
//var   float   HarryNoiseFreq;         //Average Noises per second

//All as normal ints
var   int     ActualYaw;  //Actual Yaw always tries to point towards Desired Yaw.
var   int     StartYaw;   // StartYaw is so I can use EaseBetween it and DesiredYaw
var   int     DesiredYaw;
var   int     YawTime;     //Passage of time for yaw travel
var   int     YawTimeDest;
var() float   HeadYawRate; //This one's in degrees

var() float   HeadAttackNearest;
var() float   HeadAttackFarthest;
const         HeadAttackCount = 6;   //Number of head attack anims  (probably 6)
var   name    HeadAttackAnimName[6];

var   Rotator TempRot;

//*********************************************************************************************************************
function PostBeginPlay()
{
	local GenericColObj  a;

	super.PostBeginPlay();

	a = spawn( class'GenericColObj', self );
	a.AttachToOwner( 'bone_tent03' );

	a = spawn( class'GenericColObj', self );
	a.AttachToOwner( 'bone_tent05' );
	a.eVulnerableToSpell = SPELL_Flipendo;

	a = spawn( class'GenericColObj', self );
	a.AttachToOwner( 'bone_tent07' );

	a = spawn( class'GenericColObj', self );
	a.AttachToOwner( 'bone_tent09' );

	a = spawn( class'GenericColObj', self );
	a.AttachToOwner( 'bone_tent11' );
	a.eVulnerableToSpell = SPELL_Flipendo;
	a.bIsHead = true;
}

//*********************************************************************************************************************
function Trigger( Actor Other, Pawn EventInstigator )
{
	if( IsInState( 'stateIdle' ) )
	{
		playerHarry.HearHarryRecipient = self;
		GotoState( 'stateWaiting' );
	}
}

//******************************************************************************************
function RotateTo( int yaw, optional float time, optional float rate )
{
	ActualYaw = ActualYaw & 0xFFFF;
	yaw       = yaw       & 0xFFFF;

	DesiredYaw = yaw;
	StartYaw =   ActualYaw;

	if( Abs(DesiredYaw - StartYaw) > 32767 )
	{
		if( DesiredYaw > StartYaw )
			DesiredYaw -= 0x10000;
		else
			DesiredYaw += 0x10000;
	}
	//else  //good to go!

	YawTime = 0;
	if( time != 0 )
		YawTimeDest = time;
	else
	if( rate != 0 )
		YawTimeDest = Abs( DesiredYaw - StartYaw )*360/0x10000 / rate;
	else
		YawTimeDest = Abs( DesiredYaw - StartYaw )*360/0x10000 / HeadYawRate;

	bRotateToDesired = false;
}

//******************************************************************************************
function RotateToHarry( optional float time, optional float rate )
{
	RotateTo( IntRotToHarry(), time, rate );
}

//******************************************************************************************
function float IntRotToHarry()
{
	local int    YawToHarry;

	//Find engine yaw to harry, and make sure it's positive
	YawToHarry = (  Rotator((playerHarry.Location - Location) * vect(1,1,0)).yaw
	              - Rotation.Yaw  +  0x20000
	             ) & 0xFFFF;

	//if( YawToHarry >= 0x8000 )  //now make it -0x8000 to 0x7FFF
	//	YawToHarry -= 0x10000;

	return YawToHarry;// * 360.0 / 0x10000;
}

//*********************************************************************************************************************
function Tick( float dtime )
{
	TimeSinceLastHearNoise += dtime;

	//if TimeSinceLastHearNoise is less than say 0.7, then harry is "making noise."  Count up our timer.
	if( TimeSinceLastHearNoise < 0.7 )
		HarryMakingNoiseTimer += dtime;
	else
		HarryMakingNoiseTimer = 0;

	//Do our rotation stuff
	if( !bRotateToDesired )
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

		SetBasilYaw( ActualYaw );
	}
}

//*********************************************************************************************************************
//Use this to force Basil's yaw to something new
function SetBasilYaw( int yaw )
{
	local Rotator r;

	r = Rotation;
	r.yaw = yaw;
	SetRotation( r );
	ActualYaw = yaw;
	DesiredYaw = yaw;
	DesiredRotation = r;
}

//*********************************************************************************************************************
function HideBasil()
{
	SetLocation( Location + vect(0,0,-200) );
}

//*********************************************************************************************************************
function ColObjTouch( actor other, GenericColObj ColObj )
{
	//We only look for harry touches
	if( Harry(other) == none )
		return;

	//Different damage and sounds based on whether head or not
	if( ColObj.bIsHead )
	{
		//PlaySound
		Harry(other).TakeDamage( HeadDamage, self, ColObj.Location, vect(0,0,0), '');
	}
	else
	{
		//PlaySound
		Harry(other).TakeDamage( TailDamage, self, ColObj.Location, vect(0,0,0), '');
	}
}

//******************************************************************************************
function bool HandleSpellFlipendo( optional baseSpell spell, optional vector vHitLocation )
{
	GotoState( 'stateHit' );
	return true;
}

//*********************************************************************************************************************
auto state stateIdle
{
  Begin:
	sleep(2);
	HideBasil();

Trigger(none, none);

}

//*********************************************************************************************************************
function PawnHearHarryNoise()// float Loudness, Actor NoiseMaker )
{
	//playerHarry.ClientMessage(Noisemaker$" made noise");

	//if( Harry(NoiseMaker) == none )
	//	return;

	LastNoiseLoc = PlayerHarry.Location;//NoiseMaker.Location;
	TimeSinceLastHearNoise = 0;

	//If we're in rotate to desired mode, rotate to the last noise loc
	if( bRotateToDesired )
	{
		DesiredRotation = Rotator((LastNoiseLoc - Location) * vect(1,1,0));
	}
}

//*********************************************************************************************************************
state stateWaiting
{
	function BeginState()
	{
		TimeSinceLastHearNoise = 0;
		HarryMakingNoiseTimer = 0;
		SetTimer( IdleTimerStart + RandRange( -IdleTimerStart/3, IdleTimerStart/2 ), false );
	}

	event PawnHearHarryNoise()// float Loudness, Actor NoiseMaker)
	{
		//if( Harry(NoiseMaker) == none )
		//	return;

		//If it's been a while since we last attacked harry based on hearing him, go for it...
		if(   TimeSinceLastHearNoise > AttackTime*1.5
		   || TimeSinceLastHearNoise > AttackTime      &&  HarryMakingNoiseTimer > 1
		   || HarryMakingNoiseTimer > 2
		  )
			DoHarryAttack( PlayerHarry.Location - LastNoiseLoc );

		Global.PawnHearHarryNoise();// Loudness, NoiseMaker );
	}

	function Timer()
	{
		//Do Tail swing, 
		//or double head lunge random attack, 
		//or tail up in air wiggle, 
		//or tail up, wiggle, then pound ground.
		//or come out and look around.
		switch( 4)//Rand(5) )
		{
			case 0:
				GotoState('stateTailSwing');				break;
			case 1:
				GotoState('stateRandomAttack');				break;
			case 2:
				GotoState('stateTailWiggle');				break;
			case 3:
				GotoState('stateTailPound');				break;
			case 4:
				GotoState('stateLookAround');				break;
		}
	}

  Begin:
	do
	{
		//PlaySound( random sound, random volume );
		Sleep( RandRange( 1, 2 ) );
	}until(false);
}

//*********************************************************************************************************************
state stateTail
{
	function BeginState()
	{
		//Mesh = SkeletalMesh'HPModels.skBasiliskMesh';
	}
	function EndState()
	{
		Mesh = SkeletalMesh'HPModels.skBasiliskMesh';
	}
}

//*********************************************************************************************************************
state stateTailSwing expands stateTail
{
	//Also, if harry moves while swinging, do another swing or two
  Begin:
	MoveToRandomVisibleHole();
	TempRot = Rotator( (playerHarry.Location - Location)*vect(1,1,0) );

	TempRot.yaw += RandRange( -32767, 32767 );
	SetBasilYaw( TempRot.yaw );

	PlayAnim( 'lunge1' );
	AnimFrame = 36.0/64.0;
	Sleep( 1.5 );

	HideBasil();
	GotoState( 'stateWaiting' );
}

//*********************************************************************************************************************
state stateRandomAttack
{
	//Also, if harry moves while swinging, do another attack or two
  Begin:
	MoveToRandomVisibleHole();
	TempRot = Rotator( (playerHarry.Location - Location)*vect(1,1,0) );
	TempRot.yaw += RandRange( -32767, 32767 );
	SetBasilYaw( TempRot.yaw );

	PlayAnim( 'lunge1' );
	AnimFrame = 36.0/64.0;
	Sleep( 1.5 );

	HideBasil();
	GotoState( 'stateWaiting' );
}

//*********************************************************************************************************************
state stateTailWiggle expands stateTail
{
  Begin:
	MoveToRandomVisibleHole();

	PlayAnim( 'lunge1' );
	FinishAnim();

	HideBasil();
	GotoState( 'stateWaiting' );
}

//*********************************************************************************************************************
state stateTailPound expands stateTail
{
  Begin:
	MoveToRandomVisibleHole();
	TempRot = Rotator( (playerHarry.Location - Location)*vect(1,1,0) );
	TempRot.yaw += RandRange( -32767, 32767 );
	SetBasilYaw( TempRot.yaw );

	PlayAnim( 'lunge1' );
	Sleep( 0.5 );
	//PlaySound( 'thump' );
	//CameraShake();

	HideBasil();
	GotoState( 'stateWaiting' );
}

//*********************************************************************************************************************
state stateLookAround
{
	//Also, if harry moves while swinging, do another attack or two
  Begin:
	MoveToRandomVisibleHole();
	TempRot = Rotator( (playerHarry.Location - Location)*vect(1,1,0) );
	TempRot.yaw += (Rand(2)*2-1) * RandRange( 8000-2000, 8000+2000 );
	SetBasilYaw( TempRot.yaw );

	PlayAnim( 'lunge1' );
	AnimFrame = 36.0/64.0;
	Sleep( 1.5 );

	HideBasil();
	GotoState( 'stateWaiting' );
}

//*********************************************************************************************************************
state stateHit
{
  Begin:
	Sleep( 2 );
	HideBasil();
	GotoState( 'stateWaiting' );
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

	vDir = normal( vector(playerHarry.Rotation) * vect(1,1,0) );

	ForEach AllActors(class'actor', a, HoleMarkerCommonTag)
	{
		Holes[ NumHoles++ ] = a;

		if( ( vDir dot normal((a.Location - playerHarry.Location) * vect(1,1,0)) )  >  0.5 ) //what is the fov, anyways?
			VisibleHoles[ NumVisibleHoles++ ] = a;
	}

	if( NumHoles > 20 )
		playerHarry.ClientMessage("*************** ERROR ERROR ERROR: Too many holes...");

	//If there are no visible holes, or just randomly sometimes, then just randomly pick one.
	if( NumVisibleHoles == 0  ||  FRand() < 0.2 )
		a = Holes[ Rand(NumHoles) ];
	else // There are visible holes, randomly pick one
		a = VisibleHoles[ Rand(NumVisibleHoles) ];

	SetLocation( a.Location );
}

//*********************************************************************************************************************
//vDir is the direction harry is moving.
function DoHarryAttack( vector vDir )
{
	local vector vLoc;
	local Actor  ClosestActor, a;
	local float  fClosestDist;

	/*
	vLoc = playerHarry.Location;

	if( VSize(vDir) > 0 )
		vLoc += normal(vDir) * HoleLookAheadDistance;

	//Find closest hole to vLoc
	fClosestDist = 100000;
	ForEach AllActors(class'Actor', a, HoleMarkerCommonTag)
	{
		if( VSize2d( a.Location - vLoc ) < fClosestDist )
		{
			fClosestDist = VSize2d( a.Location - vLoc );
			ClosestActor = a;
		}
	}
	*/
	//Find closest hole to lastnoiseloc
	fClosestDist = 100000;
	ForEach AllActors(class'Actor', a, HoleMarkerCommonTag)
	{
		if( VSize2d( a.Location - LastNoiseLoc ) < fClosestDist )
		{
			fClosestDist = VSize2d( a.Location - LastNoiseLoc );
			ClosestActor = a;
		}
	}

	//Move basil to the marker, and start up our business...
	SetLocation( ClosestActor.Location );
	TempRot = Rotator( (LastNoiseLoc - ClosestActor.Location) * vect(1,1,0) );
	SetBasilYaw( TempRot.yaw );

	GotoState('stateAttackHarry');
}

//*********************************************************************************************************************
state stateAttackHarry
{
	function BeginState()
	{
		bRotateToDesired = true;
		DesiredRotation = Rotation;
	}

	function EndState()
	{
		bRotateToDesired = false;
	}

  Begin:
	//PlaySound();
	PlayLungeAnim();
	FinishAnim();
Sleep( 2 );
	HideBasil();
	GotoState( 'stateWaiting' );
}

//*********************************************************************************************************************
function PlayLungeAnim()
{
	local int i;
	local int w;

	//Get the right anim based on harry's distance from the origin

	//Distance between two adjacent anim lunges
	w = (HeadAttackFarthest - HeadAttackNearest) / (HeadAttackCount - 1);
	i = ( VSize2d(playerHarry.Location - Location) - HeadAttackNearest + w/2 )   /   w;
	i = Clamp( i, 0, HeadAttackCount-1 );
	
	PlayAnim( HeadAttackAnimName[i], 1.0, 0.2 );
}

//*********************************************************************************************************************
defaultproperties
{
	Mesh=SkeletalMesh'HPModels.skBasiliskMesh'
	Physics=PHYS_Rotating

	bCollideActors=true
	bBlockActors=false
	bBlockPlayers=false
	bCollideWorld=false

    RotationRate=(Pitch=4096,Yaw=5000,Roll=3072)

	bRotateToDesired=false

	HoleMarkerCommonTag="BasiliskHoleMarker"
	//HoleLookAheadDistance=150
	IdleTimerStart=5

	AttackTime=3

	HeadYawRate=45

	HeadAttackNearest=370
	HeadAttackFarthest=1700
	//HeadAttackCount=6
	HeadAttackAnimName(0)="lunge1"
	HeadAttackAnimName(1)="lunge1"
	HeadAttackAnimName(2)="lunge1"
	HeadAttackAnimName(3)="lunge1"
	HeadAttackAnimName(4)="lunge1"
	HeadAttackAnimName(5)="lunge1"
}

//===============================================================================
//  firecrab
//===============================================================================


class firecrab extends HChar;


// *** Variables
var vector vHome;
var vector vPush;
var float fHighestZ;
var bool bFalling;

var vector vMoveDir;
var Rotator vMoveDirRot;

var sound WalkingSound;
var sound RoarSound;
var sound AttackSound;

var float OldFlipendoXY;
var float OldFlipendoZ;

var() float TimeUntilNextFireDefault;
var float TimeUntilNextFire;

var() float fAttackRange;

var() int   iNumSpellHitsToFlipDefault;
var   int   iNumSpellHitsToFlip;

var() bool bFallDistanceCheck;

var() float fTimeSpentOnBack;
var   float fTimeOnBack;		// seems silly but I need to change this if hit by a thrown object before it hits the Handle spell

// *** Constants

const		BOOL_DEBUG_AI	= false;	// if true this will spit out debug AI info


//** Functions

function PreBeginPlay()
{

	Super.PreBeginPlay();

	// Changed this to only large firecrabs and not small ones
	// because after tuning some of the  firecrabs were larger than before and 
	// and wouldn't fit into the same areas. Puzzles were breaking, People were yelling, Mass Hysteria!
	if ( DrawScale != Default.Drawscale && self.IsA('FirecrabLarge') )
	{
		SetCollisionSize(Default.CollisionRadius*DrawScale/Default.DrawScale, Default.CollisionHeight*DrawScale/Default.DrawScale);
	}

	AmbientSound = WalkingSound;

}

function PostBeginPlay()
{
	Super.PostBeginPlay();

	SetPhysics(PHYS_Walking);

	loopAnim('idle');

	vHome = location;

	TimeUntilNextFire = TimeUntilNextFireDefault;

	iNumSpellHitsToFlip = iNumSpellHitsToFlipDefault;

	fHighestZ = location.z;

	// This is set based on where the spell hit came from. If it's from a spell I use the 
	// defaults. If it's from a thrown object I increase the amount of time. If it's being hit
	// multiple times I increment the time
	fTimeOnBack = fTimeSpentOnBack;
}

// Play the sound when the firecrab gets hit with a spell
function playHitSound()
{
	local sound hitSound;
	local int randNum;

	randNum = rand(3);

	switch (randNum)
	{
	case 0:
		hitSound = sound'HPSounds.Critters_sfx.firecrab_ouch_A';
		break;
	case 1:
		hitSound = sound'HPSounds.Critters_sfx.firecrab_ouch_B';
		break;
	case 2:
		hitSound = sound'HPSounds.Critters_sfx.firecrab_ouch_C';
		break;
	}

	PlaySound( hitSound, SLOT_None, [Volume]RandRange(0.6, 1.0), [Radius]10000, [Pitch]RandRange(0.8, 1.2),, false );
}

//*************************************************************************************************************************
event TakeDamage( int Damage, Pawn EventInstigator, vector HitLocation, vector Momentum, name DamageType)
{
	if( DamageType == 'ZonePain' )
	{
		Destroy();
	}
}

function PlayerCutCapture()
{
	if ( !IsInState('stayFlipped') )
	{
		gotoState('CutIdle');
	}
}

state CutIdle
{
	begin:

	TimeUntilNextFire = 9999999999;
	GotoState('patrol');
}

function PlayerCutRelease()
{
	TimeUntilNextFire = TimeUntilNextFireDefault;
}

function bool OnALedge(Vector loc)
{
	local Vector vLocation, vUnderLocation;

	vLocation = loc;
	vUnderLocation = vLocation;
	vUnderLocation = vUnderLocation + vec(0,0,-35);

	if ( FastTrace(vUnderLocation,vLocation) )
	{
		return true;
	}

	return false;

}

function vector pushDirection()
{
	local float fRotation[16];
	local int	count, rotationCount;
	local rotator Facing;
	local vector tempLocation, tempCollision;
	local int	index;

	Facing = rotator(playerHarry.location - location);

	for ( count=1; count<=16; count++ )
	{
		Facing.yaw = (65536 / 16) * count;
		tempLocation = location + (vector(Facing) * collisionRadius);

		// Will be true at the points the firecrab is on the ground
		if ( !OnALedge(tempLocation) )
		{
			fRotation[rotationCount] = (65536 / 16) * count;
			rotationCount++;
		}
	}

	for ( count=0; count<rotationCount; count++ )
	{
//		playerHarry.clientMessage("Rotation  : " $fRotation[count]);
	}

	if ( rotationCount <= 2 )
	{
		// Only two points are hanging on the edge so drop it into the pit
		return vec(0,0,-1);
	}
	else if ( rotationCount <= 9 )
	{

		// Since the rotationCounter is incremented only when that point is on the ground the 
		// rotationCounter div 2 will be the point where you should push to get it into the pit
		index = rotationCount / 2;

		Facing = rotator(playerHarry.location - location);
		
		Facing.yaw = fRotation[index];
		tempLocation = location + (vector(Facing) * collisionRadius);

		return normal(location-tempLocation);
	
	}
	else
	{
		// There is a problem. The center is over a pit but more than half of the firecrab
		// is on the ground. Leave as is (maybe it's a doughnut)
		return vec(0,0,0);
	}


}


// Return the vector that the given actor is facing
static function vector GetFacing( actor A )
{
	return vec(1,0,0) >> A.Rotation;
}


function Landed( vector HitNormal )
{	
	local float fFallDistanceZ;

	Super.Landed(HitNormal);

	if ( DrawScale != Default.Drawscale && self.IsA('FirecrabLarge') )
	{
		SetCollisionSize(Default.CollisionRadius*DrawScale/Default.DrawScale, Default.CollisionHeight*DrawScale/Default.DrawScale);
	}

	// How far did the firecrab fall
	fFallDistanceZ = (fHighestZ-location.z);

	if ( fFallDistanceZ > 25 && bFallDistanceCheck == true )
	{
		gotoState('stayFlipped');
	}


playerHarry.clientMessage("How far did the firecrab fall : " $fFallDistanceZ);

	// Came from the Falling off the ledge state
	if ( bFalling == true )
	{
		if ( fFallDistanceZ < 25 )
		{
			gotoState('FallOverLedge');
		}
	}
	
}


function Falling()
{
	Super.Falling();

	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("I'm falling and I can't get up " );

}

function HitByThrownObject( int Damage, HPawn instigatedBy, Vector hitlocation, 
							Vector momentum, name ObjectType )
{
	fTimeOnBack = fTimeSpentOnBack * 4;

	// No matter what the size of the crab. If it gets hit by a thrown object it gets flipped over. 
	iNumSpellHitsToFlip = 1;

	HandleSpellRictusempra( none, hitlocation );

}


function Trigger( actor Other, pawn EventInstigator )
{
//	PlayerHarry.ClientMessage( Other$" triggered Firecrab with "$EventInstigator );

	// This firecrab has set off the trigger and should stay flipped over
	if ( Other == self )
	{
		gotoState('stayFlipped');
	}

}


//** Generic States

state FallOverLedge
{
	function BeginState()
	{
		fHighestZ = location.z;

		// check if you are coming back to this state from another fall from a ledge
		if ( bFalling == true )
		{
			if ( OnALedge(location) )
			{
				vPush = pushDirection();
			}
		}

		bFalling = true;
	}

	function Tick(float DeltaTime)
	{
		Super.Tick(DeltaTime);

		MoveSmooth( vPush * (groundSpeed*3) * DeltaTime);

//		if ( Physics == PHYS_Falling )
//		{
//			gotoState('stayFlipped');
//		}
	}

	begin:

}

// If a trigger is sent to the firecrab leave it flipped over. 
state stayFlipped
{
	begin:

	// Make sure that they can still be moved slightly
	eVulnerableToSpell = SPELL_Flipendo;

	// If a small firecrab hits this state it means that they are on the edge of a pit or in a 
	// pit with a trigger in it. Either way you don't want them to get spelled out of the pit. 
	if ( self.IsA('FirecrabSmall') )
	{
		fFlipPushForceXY = 0.5 * default.fFlipPushForceXY;
		fFlipPushForceZ = 0.4 * default.fFlipPushForceZ;
	}

	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage( Name $ " : State StayFlipped ");

	// make sure the crab doesn't try to attack harry again
	TimeUntilNextFire = 999999;

	// loop the flipped over animation
	loopAnim('onBack');

onBackloop:

	// Keep resetting the time until next fire
	TimeUntilNextFire = 999999;

	sleep(1);
	goto 'onBackloop';

}


defaultproperties
{
     fAttackRange=400
     iNumSpellHitsToFlip=2
     fTimeSpentOnBack=5
     WalkingSound=Sound'HPSounds.Critters_sfx.firecrab_walk'
	 RoarSound=sound'HPSounds.Critters_sfx.firecrab_roar'
	 AttackSound=sound'HPSounds.Critters_sfx.firecrab_Attack'
     bThrownObjectDamage=True
     GroundSpeed=60
     AirSpeed=60
     AccelRate=4000
     SightRadius=600
     PeripheralVision=1
     BaseEyeHeight=20
     EyeHeight=20
     IdleAnimName=breath
     RunAnimName=Walk
     eVulnerableToSpell=SPELL_Rictusempra
     Mesh=SkeletalMesh'HPModels.skfirecrabMesh'
     DrawScale=2
     AmbientGlow=110
	 bBlockActors=True
     Mass=130
	 RotationRate=(Pitch=100000,Yaw=100000,Roll=100000)
	 TimeUntilNextFireDefault=2
	 bFalling=false
	 bFallDistanceCheck=True
}

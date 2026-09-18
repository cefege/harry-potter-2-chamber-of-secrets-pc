class Spider expands HChar;

var vector vDir;
var rotator rRotationVector;
var vector vDirectionVector;

var bool  bAttacking;
var int   check;
var name  savedState;

var() int numSpellsDefault;		// the number of spellhits the spider can take (only LARGE)
var int numSpells;				// the current number of spellhits taken

var() float leaveDeadSpider;	// The amount of time to leave a SMALL dead spider

var SpiderMarker currentMarker;		// The current marker that the spider is walking to 
var float		 forward;			// Used to count down the amount of forward distance so far

var() float normalSpeed;		// The speed the spider walks normally
var() float attackSpeed;		// The speed that the spider will move toward Harry to attack (web speed should be faster)
var() bool  canWander;			// If false the spider will pick the closest spot and wait
var() bool  waitForTrigger;		// If true the spider will sit where it is placed and wait for a sign
var() float forwardDistance;	// If 0: will go directly to marker. If !0 will walk forward in the current rotation and then go to marker
var() string groupName;			// The group of markers this spider will walk to. 
var() float  jumpDistance;		// The amount the spider will jump off of the wall before falling

var float  drawingScale;		// The drawScale of the spiders (used only from spawner)
var() bool	 bJumper;			// If true this is a jumping spider and will be placed on the wall to jump off and surprise Harry
var() float  jumpingDistanceFromHarry;	// If greater than this distance use the jumping anim to get close to Harry. 
var() float fDamageAmount;		// The amount of damage done by this spider

var SpiderLarge myFriends[6];	// The list of friends for the large spider. Only one will attack at once
var int			 numFriends;	// The number of friend for this spider
var int			 counter;		// counter for looking at friends

var bool bRandomSize;			// Check if the size of the spider should be random or not. 

var bool atTheEdge;				// Check if we are at the edge of our marker area. Seem to wander out when attacking
var float EdgeCounter;			// Keep track of how long we are returning from the edge (1-2 seconds is enough)

enum enumPreAttackAnim
{
	ATTACK_NONE,
	ATTACK_JUMP,
	ATTACK_REAR,
};

var() enumPreAttackAnim ePreAttackAnim;

var sound squishSound;



//************ Generic Functions ************************************************************
//*******************************************************************************************

function preBeginPlay()
{
	Super.preBeginPlay();

	if ( bJumper == true )
	{
//		SetPhysics(PHYS_Flying);
		SetPhysics(PHYS_None);
		bCollideWorld = false;
	}
	else
	{
		SetPhysics(PHYS_Walking);
	}

	if ( bRandomSize == True )
	{
		DrawScale *= RandRange(0.7, 1.5);
	}

	if ( self.IsA('SpiderAttendent') )
	{
		SetCollision( [NewColActors]true, [NewBlockActors]false);
	}
}

function PostBeginPlay()
{
//	local SpiderLarge tempSpider;

	Super.PostBeginPlay();

	UpdateFriendsList();

//	if ( self.IsA('SpiderLarge') )
//	{
//		// Look for the Spiders with the same group
//		foreach AllActors(class'SpiderLarge', tempSpider)
//		{
//			if ( tempSpider != self )
//			{
//				if ( tempSpider.groupName == groupName )
//				{
//					myFriends[numFriends] = tempSpider;
//					numFriends++;
//				}
//			}
//		}
//	}
}

function PlayerCutCapture()
{
	savedState = GetStateName();

	if ( (savedState != 'OutForTheCount') )
	{
		gotoState('CutIdle');
	}
}

function PlayerCutRelease()
{
	if ( (savedState != 'OutForTheCount') )
	{
		gotoState(savedState);
	}
}


function UpdateFriendsList()
{
	local SpiderLarge tempSpider;

	if ( self.IsA('SpiderLarge') )
	{
		// Look for the Spiders with the same group
		foreach AllActors(class'SpiderLarge', tempSpider)
		{
			if ( tempSpider != self )
			{
				if ( tempSpider.groupName == groupName )
				{
					myFriends[numFriends] = tempSpider;
					numFriends++;
				}
			}
		}
	}
}


function playSquishSound()
{
	local int randNum;

	randNum = rand(2);

	switch (randNum)
	{ 
	case 0:
		squishSound = sound'HPSounds.Critters_sfx.Spider_small_squish';
		break;
	case 1:
		squishSound = sound'HPSounds.Critters_sfx.Spider_small_squish2';
		break;
	default:
		// just in case the skies fall
		squishSound = sound'HPSounds.Critters_sfx.Spider_small_squish';
		break;
	}

	PlaySound( squishSound, SLOT_None, [Volume]RandRange(0.6, 1.0), [Radius]10000, [Pitch]RandRange(0.8, 1.2),, false );

}


// This is a generic class so both types of spiders needs to be tested. 
// Large spiders attack when harry is close (it doesn't matter if he's still or not
// Small spiders only attack when harry is still (like stuck in web)
function bool AttackHarryCheck()
{
	local bool attack;
	local vector vTargetDir;

	attack = false;
	vTargetDir	= playerHarry.location - location;

	if ( self.IsA('SpiderSmall') )
	{
		// small spider
		if ( vSize(playerHarry.velocity) < 5 && vsize(vTargetDir) < SightRadius )
		{
			attack = true;
		}

	}
	else if ( self.IsA('SpiderLarge') )
	{
		// large spider
		if ( vsize(vTargetDir) < SightRadius )
		{
			attack = true;
		}
	}

	return attack;

}

function Touch(actor other)
{
	if ( self.IsA('SpiderSmall') && !IsInState('Attack') && other.IsA('Harry') )
	{
		if ( vSize(playerHarry.velocity) != 0 )
		{
			// if harry is moving he squishes the spiders. Leave them for a bit and then destroy
			playerHarry.clientMessage("Squish");
			playSquishSound();
			gotoState('DeadSpider');			
		}
	}
	else
	{
		Super.Touch(other);
	}
}


function InitSpider()
{
	local SpiderMarker marker;

	foreach AllActors( class'SpiderMarker', marker )
	{
		if ( marker.bCenter == true && marker.groupName == groupName)
		{
			currentMarker = marker;
			break;
		}
	}

	if ( currentMarker == None )
	{
		log(self.name$ ": There is NO marker with the same name in this level");
	}

	groundSpeed = normalSpeed + frand()*10;

	forward = forwardDistance;
 
	numSpells = numSpellsDefault;

	if ( DrawScale != Default.Drawscale )
	{
		SetCollisionSize(Default.CollisionRadius*DrawScale/Default.DrawScale, Default.CollisionHeight*DrawScale/Default.DrawScale);
	}
}

function vector getNewDirection()
{

	local vector vToMarker, vUp, vRight;
	local vector vDirection;
	local vector vToMarkerEdge;

	vToMarker = normal(currentMarker.location - location);
	vUp = vec(0,0,1);

	vToMarkerEdge = vToMarker * currentMarker.collisionRadius;

	vRight = vToMarker cross vUp;

	if ( rand(2) == 0 )
	{
		vDirection = currentMarker.location + vToMarkerEdge + (vRight * (fRand() * 200));
	}
	else
	{
		vDirection = currentMarker.location + vToMarkerEdge + (-vRight * (fRand() * 200));
	}

	return vDirection;





}

function Tick(float DeltaTime)
{
	Super.Tick(DeltaTime);

	if ( bDespawned )
	{
		gotoState('FellInAHole');
	}
}


// **************** States **********************

state CutIdle
{
	begin:

	if ( (savedState != 'OutForTheCount') )
	{
		loopAnim('idle');
	}

	acceleration = vect(0,0,0);
	velocity = vect(0,0,0);
}

auto state BeginSpider
{

	begin:

	if ( self.IsA('SpiderAttendent') )
	{
		gotoState('preAttackSetup');
	}
	else
	{

		InitSpider();

		loopAnim('idle');

		if ( waitForTrigger == false )
		{
			gotoState('checkForwardDistance');
		}
		else
		{
			gotoState('waitingForTrigger');
		}
	}

}

state checkForwardDistance
{
	begin:

	if ( forward != 0 )
	{
		gotoState('WalkForward');
	}
	else
	{
		gotoState('moveToMarker');
	}


}

state moveToMarker
{

	function Tick(float DeltaTime)
	{
		Global.Tick(DeltaTime);

		// if supposed to attack harry and you're not supposed to be walking forward. 
		if ( AttackHarryCheck() == true && forward == 0 && !IsInState('CutIdle') )
		{
			gotoState('preAttackCheck');
		}

		if ( Velocity == vec(0,0,0) )
		{
			loopAnim('idle');
			gotoState('Wander');
		}

	}


	function HitWall( vector HitNormal, actor HitWall )
	{
		Super.HitWall(HitNOrmal,HItWall);

	
		if ( HitWall != playerHarry )
		{
//		playerHarry.clientMessage("hit a wall");
			acceleration *= HitNormal*10;
			DesiredRotation = rotator(HitNormal);
			SetRotation( rotator(HitNormal) );			

			gotoState('Wander');
		}
	}

	function unTouch(actor other)
	{
		Super.UnTouch(other);

		if ( Other.IsA('SpiderMarker' ) )
		{
			// Check if it's this spiders' Marker
			if ( SpiderMarker(other) == currentMarker )
			{
				// stop
				Velocity = vect(0,0,0);
				Acceleration = vect(0,0,0);

				gotoState('AtMarker');
				
			}
		}
	}


	begin:

	loopAnim('Walk');

	// turn toward marker
	vDirectionVector = (currentMarker.location - location);
	if ( rand(2) == 0 )
	{
		vDir = location + (vDirectionVector * (vDirectionVector cross vec(0,0,1) * (fRand() * 205)));
	}
	else
	{
		vDir = location - (vDirectionVector * (vDirectionVector cross vec(0,0,1) * (fRand() * 205)));
	}

	// JANET
	MoveTo(vDir);

}

state WalkForward
{
	function BeginState()
	{
		loopAnim('Walk');	
	}


	function Tick(float DeltaTime)
	{
		Global.Tick(DeltaTime);

		forward -= vSize(velocity) * DeltaTime;
		
		if ( forward <= 0 )
		{
			forward = 0;
			gotoState('moveToMarker');
		}

	}

	begin:

	// Get the spider pointed in the right direction and move it
	MoveTo(location + (vector(rotation) * forwardDistance));
	
}

state AtMarker
{
	
begin:

	if ( canWander == true )
	{
		gotoState('wander');
	}
	else
	{
		gotoState('wait');
	}

}


state Wander
{
	function Tick(float DeltaTime)
	{
		Global.Tick(DeltaTime);

		if ( AttackHarryCheck() == true && !IsInState('CutIdle') )
		{
			gotoState('preAttackCheck');
		}

		if ( vSize(velocity) < groundSpeed/3.0f )
		{
			SetLocation(OldLocation);
			gotoState('ImLost');
		}
	}


	function HitWall( vector HitNormal, actor HitWall )
	{
		Super.HitWall(HitNOrmal,Hitwall);

		if ( HitWall != playerHarry )
		{
			SetLocation(OldLocation);
			gotoState('Wander');
		}
	}


	// If you touch harry attack him.
	// if you touch another spider let physics deal with it. 
	function touch(actor other)
	{

		Global.Touch(other);
		
		if ( Other.IsA('SpiderSmall') || Other.IsA('SpiderLarge') )
		{
				// Let physics deal with this
		}
		else
		{
			gotoState('wander');
		}

	}

	function bump( actor other)
	{
		touch(other);
	}

	function unTouch(actor other)
	{
		Super.unTouch(other);

		if ( Other.IsA('SpiderMarker' ) )
		{
			// Check if it's this spiders' Marker
			if ( SpiderMarker(other) == currentMarker )
			{
				// stop
				Velocity = vect(0,0,0);
				Acceleration = vect(0,0,0);

				// wait a random time at this spot
				gotoState('randomWait');
			}
		}
	}

	begin:

	groundSpeed = normalSpeed;

	if ( atTheEdge == true )
	{
		gotoState('walkAway');
	}
	else
	{	
		loopAnim('Walk',0.85);
	
		// turn toward marker
		vDir = getNewDirection();

		// JANET
		MoveTo(vDir);
	}
	
}

state walkAway
{
	function BeginState()
	{
		EdgeCounter = (fRand() * 1.5) + 1.5;
	}

	function Tick(float DeltaTime)
	{
		Super.Tick(DeltaTime);
		
		EdgeCounter -= DeltaTime;

		if ( EdgeCounter <= 0 )
		{
			acceleration = vect(0,0,0);
			velocity = vect(0,0,0);

			gotoState('wander');
		}
	}

	begin:

//playerHarry.clientMessage("Just Walk Away");

	atTheEdge = false;

	loopAnim('walk',0.85);
	
	// turn toward marker
	// JANET
	MoveToward(currentMarker);

}

state wait
{
	function Tick(float DeltaTime)
	{
		Global.Tick(DeltaTime);

		if ( AttackHarryCheck() == true  && !IsInState('CutIdle') )
		{
			gotoState('preAttackCheck');
		}
	}


	begin:

	loopAnim('idle');

	// stop
	Velocity = vect(0,0,0);
	Acceleration = vect(0,0,0);
}

state jumpOffWall
{

	function Tick(float DeltaTime)
	{
		Global.Tick(DeltaTime);

		if ( jumpDistance > 0 )
		{
			SetLocation(location + (vDirectionVector*(200 * deltaTime)));
			jumpDistance -= (200 * deltaTime);
		}
		else
		{
			gotoState('StartLandingFromJump');
		}

	}

	function vector findWallNormal()
	{
		local vector newNormal;
		local vector X, Y, Z;

		GetAxes( rotation, X, Y, Z );

		NewNormal = Z;

		return newNormal;

	}

	begin:

	vDirectionVector = findWallNormal();

	playAnim('walk2jump');

}

state StartLandingFromJump
{
	function Landed(vector HitNormal)
	{
		gotoState('LandFromJump');
	}

	begin:

	SetPhysics(PHYS_Walking);

	bCollideWorld = true;

	rRotationVector = rotator(playerHarry.location - location);
	rRotationVector.pitch = 0;

	desiredRotation = rRotationVector;

}

state LandFromJump
{
	begin:

//( (1-Velocity.Z/400)* Mass/Base.Mass, Self,Location,0.5 * Velocity , 'stomped')
//TakeDamage( int Damage, Pawn EventInstigator, vector HitLocation, vector Momentum, name DamageType);

	playAnim('Landed');
	finishAnim();

	gotoState('checkForwardDistance');

}


state waitingForTrigger
{
	function Trigger( actor Other, pawn EventInstigator )
	{
		if ( bJumper == false )
		{
			gotoState('checkForwardDistance');
		}
		else
		{
			gotoState('jumpOffWall');
		}
	}

	begin:

	loopAnim('idle');

}

state randomWait
{
	begin:

	loopAnim('idle');

	sleep(frand()*2);

	gotoState('Wander');
}

state ImLost
{
	begin:

	MoveToward(playerHarry);
	sleep(1.5);

	gotoState('wander');
}

state FellInAHole
{
	begin:

	Destroy();
}



// Default Props for the Spider
//*************************************************************************************************************************
defaultproperties
{
	Mesh=SkeletalMesh'HPModels.skSpiderLargeMesh'
	DrawType=DT_Mesh;
	Physics=PHYS_Walking
	DrawScale=1;
	Menuname="Spider";
	GroundSpeed=100;
	AirSpeed=200;
	AirControl=2.0;
	AccelRate=4000;
//    Mass=60;
	Mass=1;
	BaseEyeHeight=30;
	EyeHeight=30;
    Buoyancy=118.800003;
	RotationRate=5000;
	ambientglow=200;
	bRotateToDesired=True
	CollisionRadius=40
	CollisionHeight=30

	IdleAnimName="walk"
	bCollideWorld=true
	bcollideactors=true
	bProjTarget=true
	eVulnerableToSpell=SPELL_None
	RotationRate=(Pitch=100000,Yaw=100000,Roll=100000)

	normalSpeed=75
	attackSpeed=100
	MaxStepHeight=+0010.000000

	waitForTrigger=False
	forwardDistance=0
	bThrownObjectDamage=True

	bDespawnable=true

	canWander=True
	numSpellsDefault=1
	leaveDeadSpider=0.2
	jumpDistance=75
	jumpingDistanceFromHarry=250
	bDespawnable=true
	bRandomSize=False
}


class Bowtruckle extends HChar;


// Animations

// Idle			- 60
// Attack		- 61
// React		- 78
// Taunt		- 61
// Walk			- 31
// WalkExcited	- 21
 
var BowTruckleTwig objectToThrow;

var vector	vHome;			// starting position
var vector	vNewPos;		// random   position

var float	ftemp;

var int 	NumMeshs;

var bool	bInCutScene;

struct BowtruckleParams
{
	var() class<ParticleFX>	Died;
	var() class<ParticleFX>	Hit;
	var() class<ParticleFX>	Twig;
};

struct BarkDistParams
{
	var() float	Min;
	var() float	Max;
};

struct AccuracyParams
{
	var() float	Far;
	var() float	Close;
};

struct DamageParams
{
	var() int	ByBowTruckle;
	var() int	ByTwig;
};

var()	BowtruckleParams Particles;
var()	Mesh 			 Meshs[8];

var() float	TauntProbability;
var() float	TwigScale;
var() float	ThrowDelay;
var() float	startAttack;
var() float	startExcited;
var() float	travelFromHome;
var() float	ThrowTime;

var() DamageParams		Damage;
var() AccuracyParams	Accuracy;
var() BarkDistParams	BarkDist;

function PreBeginPlay()
{
	local int i;

	Super.PreBeginPlay();
	
	// set our home location to be were we started
	vHome = location;

	if(	ThrowDelay <= 0)
		ThrowDelay = 0.01;
//	if(	ThrowDelay > 1.5)
//		ThrowDelay = 1.5;

	NumMeshs = 0;
	for( i = 0; i < 8; i++)
	{                                                     
		if( Meshs[i] == none )              
		{                                                 
			NumMeshs = i;                     
			break;                                        
		}                                                 
	}                     

	// we need something, in case they will forget to set it right
	if(NumMeshs == 0)
	{
		Meshs[0] = SkeletalMesh'HPModels.skWiggentreeBarkMesh';
		NumMeshs = 1;
	}
}

function Mesh GetRandomMesh()
{
	local Mesh lMesh;
	local int index;

	index = Rand(numMeshs);
	lMesh = Meshs[index];

	return lMesh;
}

function GetObjectToThrow()
{
	objectToThrow = Spawn(class'BowTruckleTwig', , , , );
	if(objectToThrow == none)
		return;

	objectToThrow.SetCollision( false, false, false );
	objectToThrow.SetOwner( self );
	objectToThrow.bRotateToDesired = false;

	objectToThrow.AttachToOwner('Bip01 R Hand');

	objectToThrow.Mesh = GetRandomMesh();
	objectToThrow.Particles = Particles.Twig;
	objectToThrow.Damage = Damage.ByTwig;
	objectToThrow.DrawScale = TwigScale;

	aHolding = objectToThrow;
}

function bool HandleSpellDiffindo( optional baseSpell spell, optional vector vHitLocation )
{
	Super.HandleSpellDiffindo( spell, vHitLocation );

	// goto state HitBy Flip
	gotostate('stateHitBySpell');
	
	return true; // true == create spell effects
}

function DestroyActor(Actor a)
{
	Spawn(Particles.Died, , , location, rot(0, 0, 0));

	a.eVulnerableToSpell = SPELL_None;
	a.bHidden = true;
	a.destroy();
}

function vector RandomPosition(vector newpos, float accuracy)
{
	local rotator r;
	local vector d, v, rv;
	local float spread;

	spread = (1 - accuracy) * 2048.0;

	d.x = newpos.x - location.x;
	d.y = newpos.y - location.y;
	d.z = 0;

	r = rotator(d);
	r.Yaw += RandRange(-spread, spread);

	v = vector(r);
	rv = location + v * vsize(d);
	rv.z = newpos.z;

	return rv;
}

function DropBark(vector loc, float height)
{
	local class<Actor>	classtype;
	local actor			a;
	local vector		sloc;
	local int			num;
	local float			angle, length;


	sloc = loc;
	sloc.z += height + 30;

 	num = 1;		// num = 1 + Rand(2);
	while (num > 0)
	{
		a = Spawn(Class'HProps.WiggentreeBark', , , sloc, RotRand ());

		// we do not want to despawn bark (just in case)
		HPawn(a).bDespawnable = false;

		// random velocity, at least 100 up, and 100 sidewise
		angle	= RandRange(0.0000, 6.2832);	// from 0 degrees till 360 degrees
		length	= RandRange(BarkDist.Min, BarkDist.Max);

		a.velocity.x  = length * cos(angle);
		a.velocity.y  = length * sin(angle);
		a.velocity.z  = 100 + FRand() * 100;	

		a.DrawScale = TwigScale;

		num--;
	}
}

function bool CloseToHome()
{
	if(vsize(location - vHome) < travelFromHome)
		return true;

	return false;
}

function bool FarFromHarry()
{
	// in cut scene, do just Idle
	if(bInCutScene)
		return true;

	if( vsize(playerHarry.location - location) > SightRadius )
		return true;

	return false;
}

function PlaySoundOuch()
{
	switch( Rand(7) )
	{
		case 0:	PlaySound(Sound'HPSounds.BOW_ouch1'); break;
		case 1:	PlaySound(Sound'HPSounds.BOW_ouch2'); break;
		case 2:	PlaySound(Sound'HPSounds.BOW_ouch3'); break;
		case 3:	PlaySound(Sound'HPSounds.BOW_ouch4'); break;
		case 4:	PlaySound(Sound'HPSounds.BOW_ouch5'); break;
		case 5:	PlaySound(Sound'HPSounds.BOW_ouch6'); break;
		case 6:	PlaySound(Sound'HPSounds.BOW_ouch7'); break;
	}
}

function PlaySoundTaunt()
{
	switch( Rand(9) )
	{
		case 0:	PlaySound(Sound'HPSounds.BOW_taunt1'); break;
		case 1:	PlaySound(Sound'HPSounds.BOW_taunt2'); break;
		case 2:	PlaySound(Sound'HPSounds.BOW_taunt3'); break;
		case 3:	PlaySound(Sound'HPSounds.BOW_taunt4'); break;
		case 4:	PlaySound(Sound'HPSounds.BOW_taunt5'); break;
		case 5:	PlaySound(Sound'HPSounds.BOW_taunt6'); break;
		case 6:	PlaySound(Sound'HPSounds.BOW_taunt7'); break;
		case 7:	PlaySound(Sound'HPSounds.BOW_taunt8'); break;
		case 8:	PlaySound(Sound'HPSounds.BOW_taunt9'); break;
	}
}

function PlaySoundAttack()
{
	switch( Rand(4) )
	{
		case 0:	PlaySound(Sound'HPSounds.BOW_attack1'); break;
		case 1:	PlaySound(Sound'HPSounds.BOW_attack2'); break;
		case 2:	PlaySound(Sound'HPSounds.BOW_attack3'); break;
		case 3:	PlaySound(Sound'HPSounds.BOW_attack4'); break;
	}
}

function PlaySoundSurprise()
{
	switch( Rand(5) )
	{
		case 0:	PlaySound(Sound'HPSounds.BOW_surprise1'); break;
		case 1:	PlaySound(Sound'HPSounds.BOW_surprise2'); break;
		case 2:	PlaySound(Sound'HPSounds.BOW_surprise3'); break;
		case 3:	PlaySound(Sound'HPSounds.BOW_surprise4'); break;
		case 4:	PlaySound(Sound'HPSounds.BOW_surprise5'); break;
	}
}

// --------------------------------------------------------------------------------------------

function Bump( actor other )
{
	//We only look for harry touches
	if( Harry(other) == none )
		return;

	// if already attacks Harry, do nothing
	if(GetStateName() == 'stateAttackHarry')
		return;

	Harry(other).TakeDamage( Damage.ByBowTruckle, none, vect(0,0,0), vect(0,0,0), '');

	gotoState('stateAttackHarry');
}

function HitWall(vector HitNormal, actor HitWall)
{
	if(	IsInState('stateGoHome') )
		return;

	gotoState('stateGoHome');
}

function PlayerCutCapture()
{
	bInCutScene = true;
}

function PlayerCutRelease()
{
	bInCutScene = false;
}

function Tick(float deltaT)
{
	local float distance;

	super.Tick(deltaT);

	// in cut scene, do nothing
	if(bInCutScene)
		return;

	// if too far from Home, do nothing
	distance = vsize(vHome - location);
	if( distance > startExcited)
		return;

	// if too far from Harry, do nothing
	distance = vsize(playerHarry.location - location);
	if( distance > SightRadius )
		return;


	if( !IsInState('stateIdle') && !IsInState('stateGoSomeWhere') )
		return;

	if( distance > startExcited )
	{
		// throw or taunt
		if(	FRand() < TauntProbability )
			gotoState('stateTaunt');
		else
			gotoState('stateThrow');

		return;
	}

	if( distance > startAttack )
	{
		gotoState('stateThrow');
		return;
	}

	gotoState('stateGoToHarry');
}

// --------------------------------------------------------------------------------------------
// *** States

// stateIdle
// stateGoHome			*
// stateGoSomeWhere
// stateThrow			*
// stateTaunt			*
// stateAttackHarry		*
// stateHitBySpell		*
// stateGotoHarry		*

auto state stateIdle
{
	begin:

	// Stop moving
	Acceleration = vect(0,0,0);
	Velocity	 = vect(0,0,0);

	if(!FarFromHarry())
	{	
		Sleep(0.01);

		// if far from home, go home
		if( vsize(vHome - location) > startExcited )
			gotostate('stateGoHome');

		goto 'begin';
	}

	// goto our home location
	LoopAnim('idle');
	Sleep(RandRange(2.0, 3.0));
	FinishAnim();

	if( !CloseToHome() )
		gotostate('stateGoHome');
	else
		gotostate('stateGoSomeWhere');
}

state stateGoHome
{
	begin:

	LoopAnim('walk');

	// Turn to Home
	TurnTo(vHome);
	desiredRotation.Yaw = Rotation.Yaw;

	// Go to our home location
	MoveTo(vHome);
	
	gotostate('stateIdle');
}

state stateGoSomeWhere
{
	begin:

	vNewPos	= vHome + travelFromHome * VRand();
	vNewPos.Z= vHome.Z;

	LoopAnim('walk');

	// Turn to a new location
	TurnTo(vNewPos);
	desiredRotation.Yaw = Rotation.Yaw;

	// Go to a new location
	MoveTo(vNewPos);
	
	gotostate('stateIdle');
}

state stateThrow
{
	begin:

	// Stop moving
	Acceleration = vect(0,0,0);
	Velocity	 = vect(0,0,0);

	// Turn to Harry
	TurnTo(playerHarry.location);
	desiredRotation.Yaw = Rotation.Yaw;

	GetObjectToThrow();

	PlaySoundSurprise();

	PlayAnim('Attack');
	Sleep(0.5);

	// Throw object

	vNewPos = ComputeTrajectoryByTime(objectToThrow.location, playerHarry.location, ThrowTime, -256);

	// do not throw very accurate
	ftemp = vsize(playerHarry.location - location);
	if(	ftemp > startExcited )
	{
		vNewPos = RandomPosition(vNewPos, Accuracy.Far);
	}
	else if(	ftemp > startAttack )
	{
		vNewPos = RandomPosition(vNewPos, Accuracy.Close);
	}

	ObjectThrow( vNewPos, true, true );

	// if ThrowDelay is big enough, 
	// play animation till the very end
	if(	ThrowDelay > 1.5)
		FinishAnim();

	// otherwise stop it
	else
	{
		Sleep(ThrowDelay);
		AnimRate = 0;
	}

	gotostate('stateIdle');
}

state stateTaunt
{
	begin:

	// Stop moving
	Acceleration = vect(0,0,0);
	Velocity	 = vect(0,0,0);

	// Turn to Harry
	TurnTo(playerHarry.location);
	desiredRotation.Yaw = Rotation.Yaw;

	PlaySoundTaunt();

	PlayAnim('taunt');
	FinishAnim();

	gotostate('stateIdle');
}

state stateAttackHarry
{
	begin:

	// Stop moving
	Acceleration = vect(0,0,0);
	Velocity	 = vect(0,0,0);

	// Turn to Harry
	TurnTo(playerHarry.location);
	desiredRotation.Yaw = Rotation.Yaw;

	PlaySoundAttack();

	PlayAnim('react');
	FinishAnim();

	// move a bit from Harry, to be able to bump again
	vNewPos = location + 4 * (location - playerHarry.location) / vsize(location - playerHarry.location);
	MoveTo(vNewPos);

	gotostate('stateIdle');
}

state stateHitBySpell
{
	begin:

	// Stop moving
	Acceleration = vect(0,0,0);
	Velocity	 = vect(0,0,0);

	PlaySoundOuch();

	// disable spell
	eVulnerableToSpell = SPELL_None;

	Spawn(Particles.Hit, , , location, rot(0, 0, 0));

	DropBark(location, CollisionHeight);

	// if hold something, destroy it
	if(aHolding != none)
		aHolding.Destroy();

	DestroyActor(self);

	// unable spell
	eVulnerableToSpell=SPELL_Diffindo;

	gotostate('stateIdle');
}

state stateGotoHarry
{
	begin:

	LoopAnim('walkExcited');

	// Turn to Harry
	TurnTo(playerHarry.location);
	desiredRotation.Yaw = Rotation.Yaw;

	// Go to Harry
	MoveTo(playerHarry.location);
	
	gotostate('stateIdle');
}

defaultproperties
{
	Mesh=SkeletalMesh'HPModels.skBowtruckleMesh'

	AmbientGlow=65

	CollisionRadius=18		// 15*1.2
	CollisionHeight=42		// 35*1.2

	eVulnerableToSpell=SPELL_Diffindo

	startAttack=128
	startExcited=256
	SightRadius=512
	travelFromHome=100

	GroundSpeed=220			// it was default = 200, but Greg asked increase it a bit

	DrawScale=1.2
	TwigScale=1.0

	ThrowDelay=1.5
	ThrowTime=1.5

	TauntProbability=0.5

	bInCutScene=false

	Damage=(ByBowTruckle=1,ByTwig=2)
	Accuracy=(Close=0.5,Far=0.5)
	BarkDist=(Min=30,Max=60)

	Particles=(Died=class'Sticks3',Hit=class'Sticks2',Twig=class'Sticks1')

	bThrownObjectDamage=True
}
 
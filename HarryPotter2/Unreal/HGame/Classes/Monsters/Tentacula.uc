// Class Name  : Tentacula
//
// Created on  : 05/30/2002
// Authored by : Michael Lankerovich
// 
// Description : Tentacula AI. Can follow and attack Harry.
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class Tentacula extends HChar;

const FORTY_FIVE_DEGREES = 8192;

struct LimitsParams
{
	var() float Max;
	var() float Min;
};

var(Tentacula)	LimitsParams	LimbTimeIdle;
var(Tentacula)	LimitsParams	LimbTimeStunned;
var(Tentacula)	int				BulbDamageToHarry;
var(Tentacula)	int				LimbDamageToHarry;
var(Tentacula)	int				AttachRaduis;
var(Tentacula)	int				NumLimbs;
var(Tentacula)	float			Scale;
var(Tentacula)	bool			bCouldMove;

var	int				TooClose;
var int				firstLimb;
var vector			vTargetDir;
var vector			vNewLoc;
var rotator			rNewRot;
var TentaculaLimb	limbs[4];
var TentaculaLimb	AttackLimb;
var	BaseCam			Camera;

var bool			bHasGoodLimbs;

// some extra stuff
var	name			bName[4];
var	float			rYaw[4];

function name GetBoneName(int index)
{
	local string sName;
	local name	 bName;
	
	sName = "Attach_0" $index;
	bName = name(sName);

	return bName;
}

function MyAttachActor(actor a, actor aOwner, name bone)
{
	a.SetOwner(aOwner);
	a.AttachToOwner(bone);
}

function MyDeattachActor(actor a)
{
	a.SetPhysics(PHYS_None);
	a.AnimBone		= 0;
	a.SetOwner( None );
}

function AttachLimb(int i)
{	
	if(	limbs[i] == none )
		return;

	if(	!limbs[i].bGood )
		return;

	SetCollision( false, false, false );


	limbs[i].DamageToHarry = LimbDamageToHarry;

	// attach limb to owner
	limbs[i].SetPhysics(PHYS_Falling);
	limbs[i].SetCollision( false, false, false );
	MyAttachActor(limbs[i], self, bName[i]);

	if(limbs[i].spellBox != none)
	{
		// attach spell box to limb
		limbs[i].spellBox.SetCollision( false, false, false );
		MyAttachActor(limbs[i].spellBox, limbs[i], 'bone_tent04');
	}

	// attach head box to limb
	if( limbs[i].headBox != none )
	{
		MyAttachActor(limbs[i].headBox, limbs[i], 'head');
	}

	// reset collision stuff back
	SetCollision( true, true, true );

	if(limbs[i].spellBox != none)
		limbs[i].spellBox.SetCollision( true, true, true );

	// make an appropriate collision cylinder
	if( limbs[i].headBox != none )
	{
		limbs[i].headBox.SetCollisionSize(18 * Scale, 8 * Scale);
		limbs[i].headBox.bCollideWorld = true;
	}
}

function AttachAllLimbs()
{
	local int i;
	for ( i = 0; i < NumLimbs; i++ )
		AttachLimb(i);

	playerHarry.clientMessage("Attach...............................");

	bHasGoodLimbs = true;
}

function DeAttachLimb(int i)
{
	if(	limbs[i] == none )
		return;

	if(	!limbs[i].bGood )
		return;

	// disconnect limb from the body
	MyDeattachActor(limbs[i]);

	// disconnect spell box from the limb
	if(limbs[i].spellBox != none)
	{
		MyDeattachActor(limbs[i].spellBox);
	}

	// disconnect head box from the limb
	if( limbs[i].headBox != none )
	{
		MyDeattachActor(limbs[i].headBox);
	}
}

function DeAttachAllLimbs()
{
	local int i;
	for ( i = 0; i < NumLimbs; i++ )
		DeAttachLimb(i);

	playerHarry.clientMessage("Deattach...............................");

	bHasGoodLimbs = false;
}

function PostBeginPlay()
{
	local int		i;
	local BaseCam	cam;

	Super.PostBeginPlay();

	// scale, if we need bigger tentacula
	SetCollisionSize(CollisionRadius * Scale, CollisionHeight * Scale);
	GroundSpeed *=Scale; 
	SightRadius *=Scale;
	DrawScale *=Scale;

	// Find the camera
	foreach AllActors( class'BaseCam', cam )
		Camera = cam;

	if(NumLimbs < 0)
		NumLimbs = 1;

	if(NumLimbs > 4)
		NumLimbs = 4;

	bName[0]= GetBoneName(0);
	rYaw[0] = 0;

	switch(NumLimbs)
	{
		case 1:
			break;
		case 2:
			bName[1]= GetBoneName(4);
			rYaw[1] = FORTY_FIVE_DEGREES * 4;
			break;
		case 3:
			bName[1]= GetBoneName(3);
			bName[2]= GetBoneName(5);
			rYaw[1] = FORTY_FIVE_DEGREES * 5;
			rYaw[2] = FORTY_FIVE_DEGREES * 3;
			break;
		case 4:
		default:
			bName[1]= GetBoneName(2);
			bName[2]= GetBoneName(4);
			bName[3]= GetBoneName(6);
			rYaw[1] = FORTY_FIVE_DEGREES * 6;
			rYaw[2] = FORTY_FIVE_DEGREES * 4;
			rYaw[3] = FORTY_FIVE_DEGREES * 2;
			break;
	}

	for ( i = 0; i < NumLimbs; i++ )
	{
		limbs[i] = Spawn(class'TentaculaLimb', , , , );

		// scale, if we need bigger tentacula
		limbs[i].SightRadius *=Scale;
		limbs[i].DrawScale *=Scale;
		limbs[i].TooClose = TooClose * Scale;

		limbs[i].relYaw = rYaw[i];

		// otherwise, it fall throw the floor
		limbs[i].SetPhysics(PHYS_None);

		// otherwise, we could not spawn limbs
		limbs[i].SetCollision( false, false, false );

		limbs[i].spellBox = Spawn(class'TentaculaSpell', , , , );

		if( limbs[i].spellBox != none )
		{
			limbs[i].MinIdleTime	= LimbTimeIdle.Min;
			limbs[i].MaxIdleTime	= LimbTimeIdle.Max;

			limbs[i].MinStunnedTime = LimbTimeStunned.Min;
			limbs[i].MaxStunnedTime = LimbTimeStunned.Max;

			// otherwise, we could not spawn head box
			limbs[i].spellBox.SetCollision( false, false, false );
		}

		limbs[i].headBox = Spawn(class'GenericColObj', self );
	}
}

function bool HasLimbs()
{
	local int i;
	for ( i = 0; i < NumLimbs; i++ )
	{
		if((limbs[i].GetStateName() != 'stateTwitch') && (limbs[i].GetStateName() != 'stateDie'))
			return true;
	}

	return false;
}

function int ClosestLimb()
{
	local int i, maxi;
	local float cos, maxcos;
	local vector v, v1;
	local rotator r;

	v1   = playerHarry.location - location;
	v1.Z = 0;

	maxcos = -2;
	maxi   = -1;
	for ( i = 0; i < NumLimbs; i++ )
	{
		if((limbs[i].GetStateName() != 'stateTwitch') && (limbs[i].GetStateName() != 'stateDie'))
		{

			r.Yaw   = rotation.Yaw + limbs[i].relYaw;
			r.Roll  = 0;
			r.Pitch = 0;
			v = vector(r);
			cos = (v dot v1) / vsize(v1);
			if(cos > maxcos)
			{
				maxcos = cos;
				maxi = i;
			}
		}
	}

	if(maxi >= 0)
		AttackLimb = limbs[maxi];
	else
		AttackLimb = none;

	return maxi;
}

function bool HandleSpellDiffindo( optional baseSpell spell, optional vector vHitLocation )
{
	local int i;

	PlaySoundOuch();

	for ( i = 0; i < NumLimbs; i++ )
	{
		if(limbs[i].GetStateName() == 'stateStun')
			continue;
		if(limbs[i].GetStateName() == 'stateStunned')
			continue;
		if(limbs[i].GetStateName() == 'stateWake')
			continue;
		if(limbs[i].GetStateName() == 'stateDie')
			continue;
		if(limbs[i].GetStateName() == 'stateTwitch')
			continue;

		limbs[i].gotoState('stateStun');
	}

	gotoState('stateStun');

	return true;
}

function PlaySoundAngry()
{
	// play it just one out of 5
	switch( Rand(5) )
	{
		case 0:	PlaySound(Sound'HPSounds.VT_big_idle_angry'); break;
		default: break;
	}
}

function PlaySoundOuch()
{
	switch( Rand(3) )
	{
		case 0:	PlaySound(Sound'HPSounds.VT_big_ouch1'); break;
		case 1:	PlaySound(Sound'HPSounds.VT_big_ouch2'); break;
		case 2:	PlaySound(Sound'HPSounds.VT_big_ouch3'); break;
	}
}

function Bump( actor other )
{
	//We only look for harry touches
	if( Harry(other) == none )
		return;

	// if stunned, do not damage Harry
	if(GetStateName() == 'stateStun')
		return;

	Harry(other).TakeDamage( BulbDamageToHarry, none, vect(0,0,0), vect(0,0,0), '');
}

auto state stateIdle
{
	begin:

	PlaySoundAngry();
	LoopAnim('IdleMad');
	Sleep(2);
	FinishAnim();
	
	// Stop moving
	Acceleration = vect(0,0,0);
	Velocity	 = vect(0,0,0);

	// start patroling
	gotostate('statePatrol');
}

state stateStun
{
	begin:

	// Stop moving
	Acceleration = vect(0,0,0);
	Velocity	 = vect(0,0,0);

	LoopAnim('idle');
	Sleep(2);
	FinishAnim();
	
	gotostate('stateIdle');
}

state statePatrol
{
	begin:

	vTargetDir = playerHarry.location - location;

	// if it is very far from Harry, deattach limbs, to speed everything up
	// otherwise attach everything back (but it will be much slower
	if( vsize(vTargetDir) >  AttachRaduis )
	{
		if(bHasGoodLimbs)
			DeAttachAllLimbs();

		Sleep(0.1);
		gotostate('statePatrol');
	}

	if( (vsize(vTargetDir) <=  AttachRaduis) && !bHasGoodLimbs )
	{
		AttachAllLimbs();
	}

	// if it is too close to Harry, turn to him with its good limb
	if( vsize(vTargetDir) <= TooClose * Scale )
	{
		rNewRot = rotator(vTargetDir);

		// Stop moving, if was moving
		Acceleration = vect(0,0,0);
		Velocity	 = vect(0,0,0);
	
		// turn one of his limbs to Harry
		firstLimb = ClosestLimb();
		if(firstLimb >= 0)
			desiredRotation.Yaw = rNewRot.Yaw - limbs[firstLimb].relYaw;

		gotostate('stateIdle');
	}

	// else if it could see Harry, go to him
	else if( vsize(vTargetDir) < SightRadius )
	{
		if(bCouldMove && LineOfSightTo(PlayerHarry))
		{
			// move a bit in Harry's direction
			vNewLoc = location + 2 * GroundSpeed * vTargetDir / vsize(vTargetDir);
			gotostate( 'stateGotoHarry' );
		}
		else
			gotostate('stateIdle');
	}
	else
		// We don't see harry, back to idle
		gotostate('stateIdle');
}

state stateGotoHarry
{
	begin:

	LoopAnim('walk');

	bRotateToDesired = false;
	MoveTo( vNewLoc );
	bRotateToDesired = true;

	gotostate('statePatrol');
}

defaultproperties
{
	Mesh=SkeletalMesh'HPModels.skvenomous1Mesh'
	AmbientGlow=65

	NumLimbs=2

	bHasGoodLimbs=false

	Scale=1.000000

	BulbDamageToHarry=1
	LimbDamageToHarry=5

	AttachRaduis=512

	LimbTimeIdle=(Min=1,Max=2)
	LimbTimeStunned=(Min=4,Max=6)

	CollisionRadius=15
	CollisionHeight=20

	GroundSpeed=20 
	SightRadius=250

	eVulnerableToSpell=SPELL_Diffindo

	bBlockActors=True
	bCollideWorld=True
	bCollideActors=false

	RotationRate=(Yaw=16384)

	bCouldMove=false

	TooClose=125

	DrawScale=1.0
}



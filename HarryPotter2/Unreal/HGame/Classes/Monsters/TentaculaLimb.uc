// Class Name  : TentaculaLimb
//
// Created on  : 05/30/2002
// Authored by : Michael Lankerovich
// 
// Description : Tentacula AI. Can follow and attack Harry.
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class TentaculaLimb extends HChar;

var vector  vTargetDir;

var float	relYaw;
var	float	TooClose;

var	float	HarryDamageTimer;
var	float	TestTime;

var float	idleTime;
var float	stunnedTime;

var	float	MinIdleTime;
var	float	MaxIdleTime;

var	float	MinStunnedTime;
var	float	MaxStunnedTime;

var int		DamageToHarry;

var bool	bGood;

var TentaculaSpell	spellBox;
var GenericColObj	headBox;

function ColObjTouch( actor other, GenericColObj ColObj )
{
	//We only look for harry touches
	if( Harry(other) == none )
		return;

	// do not allow hit Harry too often
	if( HarryDamageTimer > 0.2 )
	{
		HarryDamageTimer = 0;
		Harry(other).TakeDamage( DamageToHarry, self, ColObj.Location, vect(0,0,0), '');
	}

	// if it is attack, go back to idle ( do not want to see it go through Harry ).
	if( (GetStateName() != 'stateTwitch') && (GetStateName() != 'stateDie') && 
		(GetStateName() != 'stateStun') && (GetStateName() != 'stateStunned') && 
		(GetStateName() != 'stateBackToIdle') )
	{
		gotostate('stateBackToIdle');
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

function PlaySoundWilt()
{
	switch( Rand(8) )
	{
		case 0:	PlaySound(Sound'HPSounds.VT_big_wilt'); break;
		default: break;
	}
}

function PlaySoundAttack()
{
	switch( Rand(10) )
	{
		case 0:	PlaySound(Sound'HPSounds.VT_big_attack1'); break;
		case 1:	PlaySound(Sound'HPSounds.VT_big_attack2'); break;
		case 2:	PlaySound(Sound'HPSounds.VT_big_attack3'); break;
		default: break;
	}
}

function Tick(float dtime)
{
//	super.Tick(dtime);
	HarryDamageTimer += dtime;

	// try to disconnect head box from limb, if limb was disconnected from the bulb
	if(bGood)
	{
		TestTime = 0;
		return;
	}

	TestTime += dtime;

	// give limb time to hit the ground
	if( (TestTime > 1)  && (headBox.Owner != none) )
	{
		headBox.AnimBone = 0;
		headBox.SetOwner( none );
	}
}

auto state stateIdle
{
	begin:

	LoopAnim('idle');
	idleTime = RandRange(MinIdleTime, MaxIdleTime);

	// just in case
	if(idleTime <= 0)
		idleTime = 0.01;

	Sleep(idleTime);

	// stop animation
	AnimRate = 0;		//	FinishAnim();
	
	// start attacking
	gotostate('statePatrol');
}

state stateBackToIdle
{
	begin:

	PlayAnim( 'idle', [TweenTime]0.5);
	FinishAnim();

	gotostate('stateIdle');
}

state statePatrol
{
	begin:

	// Attack using just closest limb
	if( (Owner != none) && (Tentacula(Owner).AttackLimb != self) )
		gotostate('stateIdle');

	vTargetDir = playerHarry.location - location;
	if( vsize(vTargetDir) <= TooClose)
	{
		PlaySoundAttack();
		PlayAnim('Attack');
		FinishAnim();
	}

	// We don't see harry, back to idle
	gotostate('stateIdle');
}

state stateStun
{
	begin:

	PlaySoundOuch();

	PlayAnim('stun');
	FinishAnim();
	
	gotostate('stateStunned');
}

state stateStunned
{
	begin:

	PlaySoundWilt();

	LoopAnim('stunned');
	stunnedTime = RandRange(MinStunnedTime, MaxStunnedTime);

	// just in case
	if(stunnedTime <= 0)
		stunnedTime = 0.01;

	Sleep(stunnedTime);
	FinishAnim();
	
	gotostate('stateWake');
}

state stateWake
{
	begin:

	PlayAnim('wake');
	FinishAnim();
	
	gotostate('stateIdle');
}

state stateDie
{
	begin:

	PlayAnim('die');
	FinishAnim();
	
	gotostate('stateTwitch');
}

state stateTwitch
{
	function BeginState()
	{
		PlayAnim('twitch', 0);
		AnimFrame = RandRange( 0, 0.9 );
	}

	begin:

	Sleep(10);
	
	goto 'begin';
}

defaultproperties
{
	Mesh=SkeletalMesh'HPModels.skvenomous2Mesh'

	bGood=true

	bBlockActors=false
	bCollideWorld=false
	bCollideActors=false
	bCollideWorld=false

	SightRadius=100

	bRotateToDesired=false

	Physics=PHYS_Falling

	DrawScale=1.0
}


 
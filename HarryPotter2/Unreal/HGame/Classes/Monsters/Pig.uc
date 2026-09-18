// Class Name  : Pig
//
// Created on  : 06/14/2002
// Authored by : Michael Lankerovich
// 
// Description : Pig's AI.
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

//  Animations
//
//	idle
//	idle_1
//	fidget_1
//	eating
//	Squeal
//	React
//	walk

class Pig extends HChar;

var float  fCurrTime;

var		SleepingGoyle SlGoyle;

function PostBeginPlay()
{
	local SleepingGoyle sg;

	// Find the sleeping Goyle
	sg = none;
	foreach AllActors( class'SleepingGoyle', sg )
		SlGoyle = sg;
}

function Tick(float DeltaTime)
{
	Super.Tick(DeltaTime);

	if(	IsInState('stateSqueal') )
		return;

	if(	IsInState('stateStun') )
		return;

	// looking for Harry every 1 second
	fCurrTime += DeltaTime;
	if(fCurrTime < 1.0)
		return;

	fCurrTime = 0.0f;

	if( vsize(location - playerHarry.location) < SightRadius )
	{
		if(LineOfSightTo(PlayerHarry))
			gotostate('stateSqueal');
	}
}

function PlaySoundSnort()
{
	switch( Rand(10) )
	{
		case 0:	PlaySound(Sound'HPSounds.critters_sfx.Pig_snort01'); break;
		case 1:	PlaySound(Sound'HPSounds.critters_sfx.Pig_snort02'); break;
		case 2:	PlaySound(Sound'HPSounds.critters_sfx.Pig_snort03'); break;
		case 3:	PlaySound(Sound'HPSounds.critters_sfx.Pig_snort04'); break;
		case 4:	PlaySound(Sound'HPSounds.critters_sfx.Pig_snort05'); break;
		case 5:	PlaySound(Sound'HPSounds.critters_sfx.Pig_snort06'); break;
		case 6:	PlaySound(Sound'HPSounds.critters_sfx.Pig_snort07'); break;
		case 7:	PlaySound(Sound'HPSounds.critters_sfx.Pig_snort08'); break;
		case 8:	PlaySound(Sound'HPSounds.critters_sfx.Pig_snort09'); break;
		case 9:	PlaySound(Sound'HPSounds.critters_sfx.Pig_snort10'); break;
	}
}

function PlaySoundSqueal()
{
	PlaySound(Sound'HPSounds.critters_sfx.Pig_squeal1');
}

function bool HandleSpellRictusempra( optional baseSpell spell, optional vector vHitLocation )
{
	gotoState('stateStun');
	return true; // we have a valid hit
}

state stateSqueal
{
	begin:

	// Stop moving, if was moving
	Acceleration = vect(0,0,0);
	Velocity	 = vect(0,0,0);
	
	TurnTo(playerHarry.location);
	desiredRotation.Yaw = Rotation.Yaw;

	PlaySoundSqueal();

	PlayAnim('Squeal');
	FinishAnim();
	
	SlGoyle.PigWakeGoyle();

	gotostate('patrol');
}

state stateStun
{
	begin:

	// Stop moving
	Acceleration = vect(0,0,0);
	Velocity	 = vect(0,0,0);

	PlaySoundSqueal();

	PlayAnim('React');
	FinishAnim();
	
	SlGoyle.PigWakeGoyle();

	gotostate('patrol');
}

defaultproperties
{
	Mesh=SkeletalMesh'HPModels.skPigMesh'

	CollisionHeight=25
	CollisionRadius=40

	SightRadius=300

	GroundSpeed=125

	RotationRate=(Yaw=16384)

	eVulnerableToSpell=SPELL_Rictusempra
}



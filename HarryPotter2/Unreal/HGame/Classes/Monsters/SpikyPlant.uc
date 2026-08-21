
// Class Name  : SpikyPlant
//
// Created on  : 04/23/2002
// Authored by : Janet Weddle
// 
// Description : Spiky Plant AI. The entire Spiky Plant. Throws spikes at Harry.
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class SpikyPlant extends HChar;

// *** Variables

var SpikyHeadNoSpikes noSpikesBush;
var SpikyPlantSpike aSpike;

var() int SpikeDamage;
var() int SpikeLift;
var() int SpikeSpeed;
var() int NumberOfSpikes;
var() float durationNoSpikes;


// *** Constants

const		BOOL_DEBUG_AI	= false;	// if true this will spit out debug AI info


function PlayerCutCapture()
{
	gotoState('CutIdle');
}

state CutIdle
{

	begin:

	
}

function PlayerCutRelease()
{
	
	gotoState('ReadyAndWaiting');

}



auto state ReadyAndWaiting
{
	function ShootSpikes ()
	{
		local int i;
		local int NumSpikes;
		local rotator rotate_spike;
		local vector spike_locn, harrys_head;

		harrys_head = playerHarry.location;

		harrys_head.z += playerHarry.collisionHeight/2;

		NumSpikes = NumberOfSpikes;

		rotate_spike = rotator(harrys_head - location);
		log("Spike aim" @ rotate_spike);


		rotate_spike.roll = 0;
		rotate_spike.pitch += (65536*3) / 4;

/*
		switch( Rand(4) )
		{

			case 0: PlaySound(sound'HPSounds.Hub2_sfx.spiky_bush_shootlots1'); break;

			case 1: PlaySound(sound'HPSounds.Hub2_sfx.spiky_bush_shootlots2'); break;

			case 2: PlaySound(sound'HPSounds.Hub2_sfx.spiky_bush_shootlots3'); break;

			case 3: PlaySound(sound'HPSounds.Hub2_sfx.spiky_bush_shootlots4'); break;
		}
*/

		for (i=0; i<NumSpikes; ++i)
		{
			rotate_spike.yaw = (65536 / NumSpikes) * i;

			spike_locn = location;
			spike_locn.z += drawScale*3;

			aSpike = spawn(class'SpikyPlantSpike',self,,spike_locn, rotate_spike);
			aSpike.iDamage = SpikeDamage;
			aSpike.Lift = SpikeLift;
			aSpike.Speed = SpikeSpeed;
			aSpike.DrawScale = drawScale;
		}
	}


	function KillMaimDestroy()	// GAS
	{
		local SpikyBushNoThorns replaceBush;

		eVulnerableToSpell = SPELL_None;

		replaceBush = spawn(class'SpikyBushNoThorns',,,location+vect(0,0,-12),rotation);

		if (replaceBush == None)
		{
			log("Replace bush failed to spawn");
		}
		else
		{
			replaceBush.SetCollisionSize(collisionRadius, collisionHeight);
			replaceBush.drawScale = drawScale;
		}

		destroy ();
	}

	function bool HandleSpellDiffindo( optional baseSpell spell, optional vector vHitLocation )
	{
		ShootSpikes ();
		KillMaimDestroy(); // GAS
		return true;
	}

	function Bump (actor other)
	{
		if (other.IsA('Harry'))
		{
			other.acceleration = vect(0,0,0);
			other.velocity = vect(0,0,0);
			ShootSpikes ();
			KillMaimDestroy();	// GAS
		}
	}
begin:	// GAS
loop:
//	loopAnim ('idle', 1.0 - 0.5*FRand ());

	Sleep(Rand(2));

	//PlaySound ( sound'HPSounds.Hub2_sfx.spiky_bush_wilt', SLOT_None);

	playAnim ('idle');
	finishanim();

	if (baseHud(playerharry.myHud).bCutSceneMode == false)
	{
		if(abs(vsize(location-playerharry.location))<SightRadius)
		{
			ShootSpikes();
			gotoState('GrowBack');
			//KillMaimDestroy();
		}
	}

	goto 'loop';
}

state Wilted
{
	event AnimEnd()
	{
		// When wilted, players can walk over
		SetCollision(true,false,false);
	//   bBlockPlayers = false;

		// Don't know if the following line is completely necessary.
		// trying to fix problem where withered plants block spells being done on normal plants
		SetCollisionSize(0, 0);
	}

begin:
    bprojtarget=false;

	playAnim('wither');

	Sleep(1); // hold on for the first second of animation before playing sound effect

//	PlaySound ( sound'HPSounds.Hub2_sfx.spiky_bush_wilt', SLOT_None);
}

state GrowBack
{
	function ShowNoThorns()
	{
		SetCollision(false,false,false);
		bHidden = True;

		eVulnerableToSpell = SPELL_None;

		noSpikesBush = spawn(class'SpikyHeadNoSpikes',,,location+vect(0,0,-12),rotation);

		if (noSpikesBush == None)
		{
			log("Replace bush failed to spawn");
		}
		else
		{
			noSpikesBush.SetCollisionSize(collisionRadius, collisionHeight);
			noSpikesBush.drawScale = drawScale;
		}
	}

	function ShowThorns()
	{
		bHidden = False;

		noSpikesBush.Destroy();

		SetCollision(true,true,true);

		eVulnerableToSpell = SPELL_Diffindo;
	}

	begin:

	ShowNoThorns();

	sleep(durationNoSpikes);

	ShowThorns();

	sleep(0.05);

	gotoState('ReadyAndWaiting');

}



defaultproperties
{
	eVulnerableToSpell=SPELL_Diffindo
	SizeModifier=0.9
	CentreOffset=(Z=25)
	Mesh=SkeletalMesh'HPModels.skSpikyPlantMesh'
	AmbientGlow=65
	CollisionHeight=10
	CollisionRadius=35
	DrawScale=1.0
	bProjTarget=True
	bThrownObjectDamage=True
	SightRadius=200

	durationNoSpikes=0.2
	NumberOfSpikes=8
	SpikeDamage=1
	SpikeLift=400
	SpikeSpeed=300;
}

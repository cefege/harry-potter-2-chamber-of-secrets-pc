
// Class Name  : SpikyPlantHeadSpikes
//
// Created on  : 04/23/2002
// Authored by : Janet Weddle
// 
// Description : SpikyPlantHeadSpikes AI. The Top (Head) of the Spiky Plant. Can be picked up and thrown
//				 by Harry. 
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class SpikyPlantHeadSpikes extends HProp;

var	rotator vMoveDirRot;
var vector vMoveDir;
var float fRotVel;
var bool bFalling;
var float tempX, tempY;
var float scaleIncrement;

// *** Constants

const		BOOL_DEBUG_AI	= false;	// if true this will spit out debug AI info

function postBeginPlay()
{

	// The Plant Head will fly off in some random direction. Sometimes close, Sometimes far
	// Level designers were pretty excited about this feature.
	tempX = (FRand()*4.0);
	tempY = (FRand()*4.0);

	if ( rand(2) == 0 )
	{
		tempX = -tempX;
	}
	if ( rand(2) == 0 )
	{
		tempY = -tempY;
	}
}


// When the Plant Head hits the ground (landed) stop falling
function Landed(vector HitNormal)
{

//	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("The spiky bush has landed");

	bFalling = false;

}

function ThrownLanded(vector HitNormal)
{
//	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("I have been thrown and landed");
	ShootSpikes();
}



// When the Plant Head is thrown it will explode and throw off more spikes
function ShootSpikes ()
{
	local int i;
	local int NumSpikes;
	local rotator rotate_spike;
	local vector spike_locn, harrys_head;

	harrys_head = playerHarry.location;

	harrys_head.z += playerHarry.collisionHeight/2;

	NumSpikes = 8;

	rotate_spike = rotator(harrys_head - location);
	log("Spike aim" @ rotate_spike);


	rotate_spike.roll = 0;
	rotate_spike.pitch += (65536*3) / 4;
/*
	// @AE: Trigger audio events of shooting spikes.
	switch( Rand(4) )
	{

		case 0: PlaySound(sound'HPSounds.Hub2_sfx.spiky_bush_shootlots1'); break;

		case 1: PlaySound(sound'HPSounds.Hub2_sfx.spiky_bush_shootlots2'); break;

		case 2: PlaySound(sound'HPSounds.Hub2_sfx.spiky_bush_shootlots3'); break;

		case 3: PlaySound(sound'HPSounds.Hub2_sfx.spiky_bush_shootlots4'); break;
	}
*/
	// spawn a cloud of smoke where the spiky plant is
//	spawn(class'SmokeExplo_01',self,,location, rotation);
	if ( rand(2) == 0 )
	{
		spawn(class'Explosion_01',self,,location, rotation);
	}
	else
	{
		spawn(class'SmokeExplo_01',self,,location, rotation);
	}
 
	for (i=0; i<NumSpikes; ++i)
	{
		rotate_spike.yaw = (65536 / NumSpikes) * i;

		spike_locn = location;
		spike_locn.z += drawScale*3;

		spawn(class'ThrownSpike',self,,spike_locn, rotate_spike);
	}

	Destroy();

}

auto state fallOver
{
	// Move the Plant Head away from the stem until it hits the ground (landed) 
	function Tick(float DeltaTime)
	{

		Super.Tick(DeltaTime);

		if ( bFalling == true ) 
		{
			SetLocation( location+vec( tempX,tempY,0 ) );
		}

		if ( drawScale > 1.0 )
		{
			drawScale -= scaleIncrement;
		}
		else if ( drawScale < 1.0 )
		{
			drawScale = 1.0;
		}



	}

	// If the Plant Head hits a wall change the rotation so that it will bounce off
	function HitWall(vector HitNormal, actor Wall)
	{
		if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("Hit the wall");
		SetLocation( OldLocation );//+ (HitNormal * 50));
		//Find a new vMoveDir

		vMoveDirRot = rotator(HitNormal);
		vMoveDirRot.yaw += 65536.0*(8.0/20.0)/2.0*((FRand()*2.0)-1.0);
		vMoveDirRot.pitch = 0;
		vMoveDirRot.roll = 0;

		gotostate('TurnToNewDir');
	}

	begin:

	// Change the rotation of the Plant Head so it looks as if it's been violently cut off. 
	vMoveDir.x = 1;
	vMoveDir.y = 1;
	vMoveDir.z = 1;
	vMoveDirRot = rotation;
	vMoveDirRot.yaw += 16384*(8.0/20.0)/2.0*((FRand()*2.0)-1.0);;
	vMoveDirRot.pitch += 8191*(8.0/20.0)/2.0*((FRand()*2.0)-1.0);;
	vMoveDirRot.roll += 16384*(8.0/20.0)/2.0*((FRand()*2.0)-1.0);;
	vMoveDir = vMoveDir>>vMoveDirRot;

	fRotVel = 0;

	SetRotation(rotator(vMoveDir));
}


// Used when the Plant Head hits the wall. 
state TurnToNewDir
{

  begin:
	loopAnim('idle');

  wait:
	sleep( 0.5 );
		
	TurnTo( location + vMoveDir );
	gotostate('fallOver');

}

defaultproperties
{
     Mesh=SkeletalMesh'HPModels.skSpikyPlantHeadSpikesMesh'
     AmbientGlow=65
     CollisionRadius=25
     CollisionHeight=17
	 Physics=PHYS_Falling
	 bCollideWorld=True
	 bObjectCanBePickedUp=True
	 bFalling=True
	 scaleIncrement=0.1
}

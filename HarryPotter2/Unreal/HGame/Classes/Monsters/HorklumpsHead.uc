// Class Name  : HorklumpsHead
//
// Created on  : 04/23/2002
// Authored by : Janet Weddle
// 
// Description : HorklumpsHead AI. Can be picked up and thrown by Harry
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class HorklumpsHead extends HProp;

var	rotator vMoveDirRot;
var vector vMoveDir;
var float fRotVel;
var bool bFalling;
var float tempX, tempY;
var vector HeadLocation;
var rotator HeadRotation;

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

// Check tick. If in state stateBeingThrown make sure you can't pick up the head anymore
function Tick(float DeltaTime)
{
	Super.Tick(DeltaTime);

	if ( IsInState('stateBeingThrown') )
	{
		bObjectCanBePickedUp = false;
	}
}


// When the Plant Head hits the ground (landed) stop falling
function Landed(vector HitNormal)
{
	bFalling = false;
}


function ThrownLanded(vector HitNormal)
{
	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("Thrown and Landed");

	// Shoot poison
	ShootPoison();
}


// When the Plant Head is thrown it will explode and throw off more poison
function ShootPoison ()
{

	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("Shoot Poison");

	HeadLocation = location;
	HeadRotation = rotation;

	Destroy();

	PlaySound( sound'HPSounds.Critters_sfx.horklump_mushroom_head_explode', SLOT_Misc, [Volume]RandRange(0.6, 1.0), [Radius]95000, [Pitch]RandRange(0.8, 1.2),, false );

	// spawn a cloud of smoke where the horklump is as a 'blow-up' anim
//		spawn(class'SmokeExplo_01',self,,location, rotation);

	spawn(class'ThrownPoisonCloud',self,,HeadLocation, HeadRotation);

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
     Mesh=SkeletalMesh'HPModels.skhorklumpsHeadMesh'
     AmbientGlow=65
     CollisionRadius=10
     CollisionHeight=7
     bBlockActors=False
	 Physics=PHYS_Falling
	 bCollideWorld=True
	 bObjectCanBePickedUp=True
	 bFalling=True
}

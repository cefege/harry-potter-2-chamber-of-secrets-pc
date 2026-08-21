//===============================================================================
//  [ChocolateFrog] 
//===============================================================================

class ChocolateFrog extends HProp;

var   actor shower;
var   sound soundRibbit;
var   sound soundHop;
var   sound soundUse;

var() bool   bSmartFrog;
var   vector vStartLoc;
var() float  fFrogJumpRadius;
var   int    JumpOutOfWayCount;
var() float  fJumpTime;
var   vector vTemp;

//function PreBeginPlay()
//{
//	local vector newloc;
//
//	Super.PreBeginPlay();
//
//	newloc=location;
//	newloc.z=newloc.z+96;
//	shower=spawn(class'jellyglow',,,newloc);
//}

//*******************************************************************************************
function sound GetRibbitOrHopsound()
{
	// 70% of the time, chose the hop sound.  30% choose ribbit.
	if (fRand() <= 0.7)
		return(soundHop);
	else
		return(soundRibbit);
}

//*******************************************************************************************
function JumpToNewLoc( vector v, optional bool bJumpOutOfWay )
{
	local float JumpTime;

	JumpTime = fJumpTime;

	if( bJumpOutOfWay )
	{
		DesiredRotation.yaw = rotator(v - Location).yaw;
		PlayAnim( 'hop2', 1.0, 0.05 );
		JumpTime *= 0.5;

		JumpOutOfWayCount++;
	}

	SetPhysics( PHYS_Falling );
	Velocity = ComputeTrajectoryByTime( location, v, JumpTime );

	GotoState( 'stateJump' );
}

//*******************************************************************************************
auto state holdstill
{
	//function Tick(float dtime)
	//{
	//	JumpOutOfWayCount
	//}

	function Timer()
	{
		local vector vH, vToMe, vCross;
		local float  JumpDist;

		JumpDist = 100;

		vH = playerHarry.Velocity;
		if(   vH != vect(0,0,0)  // harry moving?
		   && vsize( playerHarry.Location - Location ) < 100  //harry in range?
		  )
		{
			vH = normal( vH * vect(1,1,0) );
			vToMe = (Location - playerHarry.Location) * vect(1,1,0);
			vCross = vH cross vToMe;
			if(   (vH dot vToMe) > 0  //coming towards the frog?
			   && Abs(vCross.z) < 40  //gonna touch it?
			  )
			{
				if( JumpOutOfWayCount >= 3 )
				{
					JumpDist = 25;
					//JumpOutOfWayCount = 0;
				}
				//Jump to the side.  Will automatically go the correct direction!  cross products are cool!
				JumpToNewLoc( location + normal(vCross cross vH) * JumpDist, true );
			}
		}
	}

	function EndState()
	{
		SetTimer(0,false);
	}

begin:
	SetTimer( 0.125, true );

	LoopAnim('CROAK', 1.0, 0.2);
	finishanim();

	// Pick the ribbit or hop sound.  If it's the ribbit, start sound before
	// animation starts.  If it's the hop sound, play it a little after the
	// animation starts.  Things seems to match up better that way.
	soundUse = GetRibbitOrHopSound();
	if (soundUse == soundRibbit)
		PlaySound(soundRibbit);

	LoopAnim('BREATH', 0.6, 0.2);
	sleep(frand()*3);
	finishanim();


	if( vStartLoc == vect(0,0,0) )
		vStartLoc = Location;
	vTemp = vStartLoc + normal(VRand()*vect(1,1,0)) * fFrogJumpRadius;
	//TurnTo( LocationSameZ( vTemp ) );
	DesiredRotation.yaw = rotator(vTemp - Location).yaw;
	PlayAnim( 'trans2hop2', ,0.2 );
	Sleep( 7.0/30.0 );
	SetTimer(0, false); //Can avoid harry up to this point.
	JumpOutOfWayCount = 0;
	PlayAnim( 'hop2', 1.0, 0.05 );
	JumpToNewLoc( vTemp );
}

/*
	soundUse = GetRibbitOrHopSound();
	if (soundUse == soundRibbit)
		PlaySound(soundRibbit);

	LoopAnim('hop', 1.0, 0.0);
	sleep(0.2);
	if (soundUse == soundHop)
		PlaySound(soundHop);
	finishanim();
*/

state stateJump
{
	function Landed(vector HitNormal)
	{
		GotoState( 'holdstill' );
	}

  Begin:
	Sleep( 0.2 );

	PlayAnim( 'breath', 1.0, 0.7 );
	FinishAnim();
}


defaultproperties
{
	Mesh=SkeletalMesh'HProps.skchocolateFrogMesh'
    DrawType=DT_Mesh
    bStatic=False
	bCollideActors=true
	bCollideWorld=true
	CollisionHeight=20
	Physics=PHYS_Falling
	DrawScale=0.5

	RotationRate=(yaw=150000)

	// Hop sounds
	soundHop=sound'HPSounds.critters_sfx.frog_hop'
	soundRibbit=sound'HPSounds.critters_sfx.frog_ribbit'

	// pickup related
	bBlockActors=false
	bBlockPlayers=false
	bBlockCamera=false
    bPickupOnTouch=true
	nPickupIncrement=40		// increase health by 40 on pickup
	PickupFlyTo=FT_HudPosition
	soundPickup=Sound'HPSounds.Critters_sfx.pickup_frog'

	// Status manager
	classStatusGroup=Class'HGame.StatusGroupHealth'
	classStatusItem=Class'HGame.StatusItemHealth'

	bSmartFrog=true
	fFrogJumpRadius=50
	fJumpTime=0.8

	ShadowClass=class'ActorShadow'
}



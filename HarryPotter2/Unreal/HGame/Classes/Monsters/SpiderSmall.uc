class SpiderSmall extends Spider; 

var vector vDir;
var vector attackLocation;
var rotator rot;
var int iteratorCheck;
var int randNum;
var name boneName;
var vector pivotAmount;
var sound spiderBiteSound;
var vector BonePosition;
 

function Timer()
{
	Destroy();

}

function bool harryIsStill()
{
	// check if Harry is pretty close to still
	if ( vSize(playerHarry.velocity) < 5 )
			return true;

	return false;
}


state preAttackCheck 
{ 
	begin:

//	playerHarry.clientMessage("preattackcheck");

	gotoState('Attack');

}


state Attack
{

	function HitWall( vector HitNormal, actor HitWall )
	{

		if ( HitWall != playerHarry )
		{
			if ( Floor == vec(0,0,1) )
			{
				acceleration *= -HitNormal*10;
			}
			else
			{
				acceleration *= HitNormal*10;
			}

			gotoState('Wander');
		} 
	}


	function bump( actor other)
	{
		touch(other);
	}

	function Touch(actor other)
	{

		Super.Touch(other);
 
		if ( other.IsA('Harry') )
		{  
//			playerHarry.clientMessage("I have touched Harry");
 
			// You have run into harry 
			if ( vSize(playerHarry.velocity) < 5 )
			{
				// stop where you are
				Velocity = vect(0,0,0);
				Acceleration = vect(0,0,0);

			} 
			else
			{
				playerHarry.clientMessage("Squish");
				playSquishSound();
				// if harry is moving he squishes the spiders. Leave them for a bit and then destroy

				gotoState('DeadSpider');
						
			}
		}
		
/*		
		if ( Other.IsA('SpiderStickyWeb') )
		{
			groundSpeed = webSpeed;
		}
*/
	}

	function unTouch(actor other)
	{
		Super.unTouch(other);

		if ( Other.IsA('SpiderStickyWeb' ) )
		{
			groundSpeed = normalSpeed;
		}
	}

begin:

	vDir = normal(playerHarry.location - location);
	SetRotation( rotator(vDir) );
	
	MoveToward(playerHarry);


	// while harry is not moving and the spider is far away keep moving toward harry
	while ( harryIsStill() == true && vSize2d(playerHarry.location - location) > 30)
	{
		sleep(0.2);
		vDir = normal(playerHarry.location - location);
		SetRotation( rotator(vDir) );
	
		MoveToward(playerHarry);
	} 

	// stop the spider
	Velocity = vect(0,0,0);
	Acceleration = vect(0,0,0);

	if ( harryIsStill() == false )
	{
		// harry is running away chase him for a bit
		gotoState('ChaseHarry');
	}
	else
	{
		gotoState('AttachToHarry');
	}
}


// This will get the spider moving toward Harry and then state 'wander' will test whether to attack or not
state ChaseHarry
{
	begin:

	vDir = normal(playerHarry.location - location);
	SetRotation( rotator(vDir) );
	
	MoveToward(playerHarry);

	sleep(0.2);

	gotoState('wander');
}


state AttachToHarry
{
	function BeginState() 
	{
//		SetCollision(false,false,false);
		iteratorCheck = 10;
	}

	function EndState()
	{
//		SetPhysics(PHYS_Spider);
//		SetCollision(true,true,true);
	}

	function Tick(float DeltaTime)
	{
		Super.Tick(DeltaTime);

		iteratorCheck--;
		if ( iteratorCheck < 0 )
		{ 
			iteratorCheck = 10;

			if ( harryIsStill()	== false )
			{
				// He's running so fall off his cloak
//				playerHarry.clientMessage("Falling off the cloak");
				StopSound(squishSound, SLOT_None);
				Detach(playerHarry);
				SetOwner(none);
				SetPhysics(PHYS_Falling);
				PrePivot = vect(0,0,0);
				gotoState('moveToMarker');

			}
			else
			{
				// play the spider bite / moving on Harry sound
				PlaySound( spiderBiteSound, SLOT_None, [Volume]RandRange(0.6, 1.0), [Radius]200 , [Pitch]RandRange(0.8, 1.2),, false );

				// they are standing there with spiders on them so keep biting
//				playerHarry.TakeDamage(fDamageAmount, Self, location, vec(0,0,0) , 'smallSpider' );
			} 
		}
 
	}

	begin:

	randNum = rand(3);

	switch (randNum)  
	{
	case 0:
		boneName = 'Cloak01 Tail1';
		pivotAmount = vec((frand()*12),(frand()*2),17);
		break;
	case 1:
		boneName = 'Cloak02 PonyTail2';
		pivotAmount = vec((frand()*16),2,19);
		break;
	case 2:
		boneName = 'Cloak02 PonyTail2';
		pivotAmount = vec((frand()*16),-2,19);
		break;
	default:
		boneName = 'Cloak01 Tail1';
		pivotAmount = vec((frand()*12),(frand()*2),17);
		break; 
	} 

	// stop the spider
	Velocity = vect(0,0,0);
	Acceleration = vect(0,0,0);

	BonePosition = BonePos(boneName);
	BonePosition.z = location.z;
	MoveTo(BonePosition); 
	sleep(0.05); 

	vDir = (vector(playerHarry.rotation));
	rot = rotator(vDir);
	rot.yaw += 32768;

	SetRotation(rot);

	SetOwner(playerHarry);

	AttachToOwner(boneName);
	PrePivot = pivotAmount;



}

state DeadSpider
{

	ignores Touch;

	begin:

	Velocity = vect(0,0,0);
	Acceleration = vect(0,0,0);

	loopAnim('idle');
	setTimer(leaveDeadSpider, false);	

} 

defaultproperties
{
     Mesh=SkeletalMesh'HPModels.skSpiderSmallMesh'
	 Menuname="SpiderSmall"
	 Physics=PHYS_Walking
     AmbientGlow=65
     CollisionRadius=10 
     CollisionHeight=7
	 IdleAnimName="walk"
	 bCollideWorld=true  
	 bcollideactors=true 
	 bBlockActors=false
	 bBlockPlayers=false
	 sightRadius=300 
//	 Mass=40; 
	 RotationRate=(Pitch=50000,Yaw=50000,Roll=50000)

	 MaxStepHeight=+005.000000
 
	 normalSpeed=75 
	 attackSpeed=105
//	 webSpeed=115    
	 fDamageAmount=1
	 leaveDeadSpider=0.5
	 bDespawnable=true 

	 // sounds
	spiderBiteSound=Sound'HPSounds.Critters_sfx.spider_small_movement'
}

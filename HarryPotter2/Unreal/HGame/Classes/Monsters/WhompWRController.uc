
class WhompWRController expands AnimChannel;

var WhompingWillow   tree;
var int              WhichRoot;  //1,2, or 3
var int              TimingStage;
var name             RootAnimNameUp;
var name             RootAnimNameLoop;
var name             RootAnimNameDown;

var GenericColObj    ColObj[3];
var float            ColTime1[3];
var float            ColTime2[3];

var harry            playerHarry;

var bool             bGoDisabled;

var int              Damage;

//***********************************************************************************************************************
function PostBeginPlay()
{
	tree = WhompingWillow(owner);
	playerHarry = tree.playerHarry;
}

//***********************************************************************************************************************
auto state stateIdle
{
  Begin:

	do { sleep(0.5); } until( Vsize2d( playerHarry.Location - ColObj[1].Location ) < 500 )

	if( !bGoDisabled )
		GotoState( 'ThrashingAndSmashing' );
}

//***********************************************************************************************************************
state  ThrashingAndSmashing
{
	function Tick(float dtime)
	{
		local int    i;
		local vector vX, vY, vZ;
		local vector vHLoc;
		local vector v;
		local float  d;

		//Update the collision objects
		for( i = 0; i < 3; i++ )
		{
			if(   AnimSequence == RootAnimNameUp    &&  AnimFrame < ColTime1[i]
			   || AnimSequence == RootAnimNameDown  &&  AnimFrame > ColTime2[i] )
			{
				if( !ColObj[i].bBlockPlayers )
					ColObj[i].SetCollision(true, true, true);
			}
			else
			{
				if( ColObj[i].bBlockPlayers )
					ColObj[i].SetCollision(true, true, false);
			}
		}

		//Now look for harry colliding
		for( i = 0; i < 3; i++ )
		{
			//Dont look for harry if he's falling/being hurt
			if( playerHarry.Physics == PHYS_Falling )
				continue;

			//You're free to run if bBlockPlayers is false
			if( !ColObj[i].bBlockPlayers )
				continue;

			//See if harry is "colliding" with the colobj.  If so, we need to toss him back.

			//Quick cull
			vHLoc = playerHarry.Location;
			if( VSize(vHLoc - ColObj[i].Location) > 200 )
				continue;

			GetAxes( ColObj[i].Rotation, vX, vY, vZ );
			v = vX * (ColObj[i].CollisionRadius / 2);
			d = ColObj[i].CollisionRadius/2 + playerHarry.CollisionRadius;
			//if( i == 0 )
			//	playerHarry.ClientMessage("d="$d$" dot="$(vHLoc - (ColObj[i].Location + v)) dot  vX);
			if( ((vHLoc - (ColObj[i].Location + v)) dot  vX)  >  d )
				continue;
			if( ((vHLoc - (ColObj[i].Location - v)) dot -vX)  >  d )
				continue;
			v = vY * (ColObj[i].CollisionWidth / 2);
			d = ColObj[i].CollisionWidth/2 + playerHarry.CollisionRadius;
			if( ((vHLoc - (ColObj[i].Location + v)) dot  vY)   >  d )
				continue;
			if( ((vHLoc - (ColObj[i].Location - v)) dot -vY)   >  d )
				continue;

			//We collide, so throw harry back
			playerHarry.DoJump();
			if( ((vHLoc - ColObj[i].Location) dot vX)  >  0 )
				playerHarry.velocity = ( vX + vect(0,0,1)) * 200;
			else
				playerHarry.velocity = (-vX + vect(0,0,1)) * 200;

			playerHarry.TakeDamage( Damage, tree, vect(0,0,0), Vect(0,0,0), '');

			break;
		}
	}

  Begin:

	do
	{
		PlayAnim( RootAnimNameUp, tree.GetUpAnimRate(WhichRoot, TimingStage), 0.1 );
		FinishAnim();//do { sleep(0.0001); } until( AnimFrame > 22/38 );

		PlayAnim( RootAnimNameLoop, 1.0, 0.1 );
		Sleep( tree.GetUpTime(WhichRoot, TimingStage) );

		PlayAnim( RootAnimNameDown, tree.GetDownAnimRate(WhichRoot, TimingStage), 0.1 );
		//do { sleep(0.0001); } until( AnimFrame > 1.0/9.0 );
		PlaySound( sound'HPSounds.Adv1Willow.whomp06', SLOT_NONE, [Volume]RandRange(0.8,1.0), [Pitch]RandRange(0.7,1.0) );
		do { sleep(0.0001); } until( AnimFrame > 3.0/9.0 );
		DoHitGroundEffects();
		AnimRate = 1.0;
		FinishAnim();

		Sleep( tree.GetOnGroundTime(WhichRoot, TimingStage) );

		TimingStage++;
		if( TimingStage >= 10  ||  tree.GetUpAnimRate(WhichRoot, TimingStage) == 0 )
			TimingStage = 0;

	}until(   Vsize2d( playerHarry.Location - ColObj[1].Location ) > 500
	       || bGoDisabled
	      );

	GotoState( 'stateIdle' );
}

//***********************************************************************************************************************
function DoHitGroundEffects()
{
	local vector  vX,vY,vZ;
	local int     i;
	local int     NumDustParts;

	GetAxes( ColObj[0].Rotation, vX, vY, vZ );
	if( ((playerHarry.Location - ColObj[0].Location) dot vX)  <  0 )
		vX = -vX;

	vX *=  ColObj[0].CollisionRadius/2;
	vY *=  ColObj[0].CollisionWidth;///2;
	vZ *= -ColObj[0].CollisionHeight;///2;

	//Dust off tip end
	NumDustParts = RandRange(3,4);
	for( i = 0; i < NumDustParts; i++ )
		spawn( PickRandomDustParticle(), none, [SpawnLocation]ColObj[0].Location + vX + vY*RandRange(-1,1) + vZ /* *RandRange(0,1)*/ );

	//Dust off middle piece
	NumDustParts = RandRange(2,4);
	for( i = 0; i < NumDustParts; i++ )
		spawn( PickRandomDustParticle(), none, [SpawnLocation]ColObj[1].Location + vX + vY*RandRange(-1,1) + vZ /* *RandRange(0,10)*/ );

	//Dust off base piece
	NumDustParts = RandRange(1,2);
	for( i = 0; i < NumDustParts; i++ )
		spawn( PickRandomDustParticle(), none, [SpawnLocation]ColObj[2].Location + vX + vY*RandRange(-1,1) + vZ /* *RandRange(0,10)*/ );

	//Play sound for Willow root
	ColObj[1].PlaySound(sound'HPSounds.Adv1Willow.Big_Whomp3', SLOT_none, RandRange(0.8, 1.0), [Pitch]RandRange(0.6, 0.9) );

	//Camera shake...
	playerHarry.ShakeView( 0.5, 200, 200 );
}

//***********************************************************************************************************************
function class<DustClouds> PickRandomDustParticle()
{
	local int  i;

	i = Rand(100);

	if( i > 60 )
		return class'DustCloud03_med';
	if( i > 45 )
		return class'DustCloud01_tiny';
	if( i > 30 )
		return class'DustCloud02_small';
	if( i > 15 )
		return class'DustCloud05_lrg';

		return class'DustCloud04_med';
}

//***********************************************************************************************************************
defaultproperties
{
	ColTime1(0)=0.1;
	ColTime2(0)=0.35;
	ColTime1(1)=0.09;
	ColTime2(1)=0.3;
	ColTime1(2)=0.15;
	ColTime2(2)=0.1;

}
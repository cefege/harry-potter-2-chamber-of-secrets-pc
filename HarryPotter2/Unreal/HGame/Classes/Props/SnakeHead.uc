//===============================================================================
//  [skSnakeHead] 
//===============================================================================

class SnakeHead extends HDecoration;

var    int    TimingStage;
var()  float  InitialDelayTime;
var()  float  TimeToNextShot[20];
var    vector vShootLocation;

var()  float  SpeedScalar;

var()  bool   bPlayFlameSound;

//******************************************************************************************************************
function PostBeginPlay()
{
	local TorchFire03  a;
	local float        fClosest;
	local actor        aClosest;
	local float        f;
	local int          i, n;

	fClosest = 1000000;

	foreach AllActors(class'TorchFire03', a)
	{
		f = VSize(a.Location - Location);
		if( f < fClosest)
		{
			fClosest = f;
			aClosest = a;
		}
	}

	vShootLocation = aClosest.Location;
	aClosest.Destroy();

	if( TimeToNextShot[0] == 0 )
	{
		//n = 4 + Rand(8);
		//for( i = 0; i < n; i++ )
		//	TimeToNextShot[i] = RandRange(0.25,3);
		TimeToNextShot[0] = RandRange(1.2,1.5);
		//TimeToNextShot[1] = RandRange(1,   2);
	}
}

//***********************************************************************************************************************
function Trigger( Actor Other, Pawn EventInstigator )
{
	if( IsInState( 'stateShooting' ) )
		GotoState( 'stateIdle' );
	else
		GotoState( 'stateShooting' );
}

//***********************************************************************************************************************
state stateIdle
{
}

//******************************************************************************************************************
auto state stateShooting
{
  Begin:

	TimingStage = 0;

	Sleep( InitialDelayTime / SpeedScalar );

	do
	{
		Sleep( TimeToNextShot[TimingStage] / SpeedScalar );
		SpawnFireBall();

		TimingStage++;
		if( TimingStage >= 20  ||  TimeToNextShot[TimingStage] == 0 )
			TimingStage = 0;
	}until( false )
}

//******************************************************************************************************************
function SpawnFireBall()
{
	local spellSnakeHeadFire   a;

	a = spawn(class'spellSnakeHeadFire', [SpawnLocation]vShootLocation, [SpawnRotation]Rotation );
	if( bPlayFlameSound )
	{
		switch( Rand(3) )
		{
			case 0: a.PlaySound(sound'HPSounds.Adv11_COS.Flame_shoot1');  break;
			case 1: a.PlaySound(sound'HPSounds.Adv11_COS.Flame_shoot2');  break;
			case 2: a.PlaySound(sound'HPSounds.Adv11_COS.Flame_shoot3');  break;
		}
	}
}

//******************************************************************************************************************
defaultproperties
{
    Mesh=skSnakeHeadMesh
    DrawType=DT_Mesh
    bStatic=False
	SpeedScalar=2
	bPlayFlameSound=true
}


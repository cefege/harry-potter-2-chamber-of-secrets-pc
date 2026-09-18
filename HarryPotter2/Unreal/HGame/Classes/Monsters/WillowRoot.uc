//===============================================================================
//  [skWillowRoot] 
//===============================================================================

class WillowRoot extends HChar;

var()  float  fGetAngryDistance;
var()  float  fAvgSoundPeriod;
var()  float  WillowSoundRadius;

var    int    LastAnimFrame;
var    int    NumWillowRootFrames;

var()  int    Damage;

//****************************************************************************************************
function PreBeginPlay()
{
	local GenericColObj  a;

	super.PreBeginPlay();

	a = spawn( class'GenericColObj', self );
	a.AttachToOwner( 'bone03' );

	a = spawn( class'GenericColObj', self );
	a.AttachToOwner( 'bone05' );

	a = spawn( class'GenericColObj', self );
	a.AttachToOwner( 'bone07' );

//		a.attachedParticleFX[0] = ParticleFX(FancySpawn(class'avifors_fly',a,,location));
//		a.attachedParticleFX[0].setPhysics(PHYS_Trailer);

	SetTimer( RandRange(0.1, 0.9), true );
}

//****************************************************************************************************
function ColObjTouch( actor other, GenericColObj ColObj )
{
	Harry(other).TakeDamage( Damage, self, ColObj.Location, vect(0,0,0), '');
}

//****************************************************************************************************
auto state stateIdle
{
	function Timer()
	{
		SetTimer( RandRange(0.6, 0.9), true );

		if( VSize2d(playerHarry.Location - Location) < fGetAngryDistance )
			GotoState( 'stateAngry' );
	}

  Begin:
	LoopAnim( 'idle', RandRange( 0.4, 1.1 ), 1 );
	AnimFrame = RandRange(0, 0.7);

	do
	{
		Sleep( fAvgSoundPeriod + RandRange( -fAvgSoundPeriod*0.5, fAvgSoundPeriod*0.5 ) );
		AnimRate = RandRange( 0.4, 1.1 );
	}until(false);
}

//****************************************************************************************************
state stateAngry
{
	function Timer()
	{
		SetTimer( RandRange(0.6, 0.9), true );

		if( VSize2d(playerHarry.Location - Location) > fGetAngryDistance + 50 )
			GotoState( 'stateIdle' );
	}

	function Tick(float dtime)
	{
		local int     Frame;
		local sound   snd;
		local bool    bPlaySound;
		local float   scale;

		Frame = AnimFrame * NumWillowRootFrames;

		//if( Frame >= 82 && LastAnimFrame < 82 )
		//	snd = sound'HPSounds.Adv1Willow.whomp01';
		//else
		if(   Frame >= 23  && LastAnimFrame < 23
		   || Frame >= 68  && LastAnimFrame < 68
		   || Frame >= 124 && LastAnimFrame < 124
		   || Frame >= 165 && LastAnimFrame < 165
		  )
			bPlaySound = true;

		if( bPlaySound )
		{
			switch( Rand(6) )
			{
				case 0:  snd = sound'HPSounds.Adv1Willow.whomp01';   break;
				case 1:  snd = sound'HPSounds.Adv1Willow.whomp02';   break;
				case 2:  snd = sound'HPSounds.Adv1Willow.whomp03';   break;
				case 3:  snd = sound'HPSounds.Adv1Willow.whomp04';   break;
				case 4:  snd = sound'HPSounds.Adv1Willow.whomp05';   break;
				case 5:  snd = sound'HPSounds.Adv1Willow.whomp06';   break;
			}
		}

		if( snd != none )
			PlaySound( snd, SLOT_None, RandRange(0.7,1), false, WillowSoundRadius, RandRange(0.8,1.2) );




		if(   Frame >= 28  && LastAnimFrame < 28
		   || Frame >= 73  && LastAnimFrame < 73
		   || Frame >= 127 && LastAnimFrame < 127
		   || Frame >= 169 && LastAnimFrame < 169
		  )
		{
			snd = sound'HPSounds.Adv1Willow.Big_whomp2';
			//scale = (fGetAngryDistance - vsize2d(playerHarry.Location - Location)) / fGetAngryDistance
			scale = -2.0/fGetAngryDistance * vsize2d(playerHarry.Location - Location)  +  2.5;
			scale = Clamp(scale,0,1);
			PlaySound( snd, SLOT_None, RandRange(0.4,1.0), false, WillowSoundRadius, RandRange(0.2,0.9) );
			playerHarry.ShakeView( 0.5, 50*scale, 50*scale );
		}

		LastAnimFrame = Frame;
	}

  Begin:
	LastAnimFrame = 0;
	LoopAnim( 'attack', RandRange( 0.1, 0.3 ), 1.0 );

	do
	{
		Sleep( fAvgSoundPeriod + RandRange( -fAvgSoundPeriod*0.5, fAvgSoundPeriod*0.5 ) );
		AnimRate = RandRange( 0.1, 0.3 );
	}until(false);
}

//****************************************************************************************************
defaultproperties
{
	Mesh=SkeletalMesh'HPmodels.skWillowRootMesh'
	DrawScale=1
	AmbientGlow=0

	fAvgSoundPeriod=2
	fGetAngryDistance=300
	ShadowScale=0.33
	NumWillowRootFrames=191
	WillowSoundRadius=400

	Damage=5
}


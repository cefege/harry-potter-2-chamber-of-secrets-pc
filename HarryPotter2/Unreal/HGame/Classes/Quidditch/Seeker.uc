//=============================================================================
// Seeker	-- A Quidditch player that tries to catch the Snitch
//=============================================================================
class Seeker extends QuidditchPlayer;

//-------------------------------------------------------------------------------------------
// States
//
// Fly		- Flying on a path; act like a seeker, waiting for target (snitch) to show up
// Pursue	- Chasing target while free-flying
//-------------------------------------------------------------------------------------------

state Fly
{
	function BeginState()
	{
		PlayerHarry.ClientMessage( Name$' Begin Seeking' );
		Log( Name$' Begin Seeking' );

		LoopAnim( 'Fly_Forward' );
		FlyOnPath( PathToFly, iReturnPoint );
	}

	function EndState()
	{
		PlayerHarry.ClientMessage( Name$' End Seeking' );
		Log( Name$' End Seeking' );

		StopFlyingOnPath();
	}

begin:
loop:
	FinishAnim();
	if ( bCapturedByCutScene )
	{
		if ( VSize(Velocity) < 50.0 )
			LoopAnim( 'Hover', , 0.5 );
		else
			LoopAnim( 'Fly_Forward', , 0.5 );
	}
	else	// Normal interactive animations...
	{
		if (    LookForTarget != None
			 && !bCaughtTarget
			 && !LookForTarget.bHidden
			 && PathToFly != Path_Intro
			 && !bStunned )
		{
			Log( Name$" Sees Target, will pursue" );
			GotoState( 'Pursue' );
		}

		if ( bCaughtTarget )
			LoopAnim( 'Hold', , 0.1 );
		else if ( bStunned )
			LoopAnim( 'Stunned', , 0.5 );
		else if ( frand() < 0.4 )
			LoopAnim( 'Fly_Forward', , 0.5 );
		else if ( frand() < 0.8 )
			LoopAnim( 'Look', , 0.5 );
		else
			LoopAnim( 'Hover', , 0.5 );
	}

	goto 'loop';
}


defaultproperties
{
	// Gryffindor
	HouseDisplayInfo(0)=(Sex=SX_Female,Mesh=SkeletalMesh'HPModels.skQuidPlayerFMesh')
	HouseDisplayInfo(0)=(MultiSkins[0]=Texture'HPModels.Skins.skQuidPlayerF_GTex0')
	HouseDisplayInfo(0)=(MultiSkins[1]=Texture'HPModels.Skins.skQuidPlayerF_Tex1')
	HouseDisplayInfo(0)=(MultiSkins[2]=None)

	// Ravenclaw
	HouseDisplayInfo(1)=(Sex=SX_Female,Mesh=SkeletalMesh'HPModels.skQuidPlayerFMesh')
	HouseDisplayInfo(1)=(MultiSkins[0]=Texture'HPModels.Skins.skQuidPlayerF_RTex0')
	HouseDisplayInfo(1)=(MultiSkins[1]=Texture'HPModels.Skins.skQuidPlayerF_Tex1')
	HouseDisplayInfo(1)=(MultiSkins[2]=None)

	// Hufflepuff
	HouseDisplayInfo(2)=(Sex=SX_Male,Mesh=SkeletalMesh'HPModels.skQuidPlayerMMesh')
	HouseDisplayInfo(2)=(MultiSkins[0]=Texture'HPModels.Skins.skQuidPlayerM_HTex0')
	HouseDisplayInfo(2)=(MultiSkins[1]=Texture'HPModels.Skins.skQuidPlayerM_Tex1')
	HouseDisplayInfo(2)=(MultiSkins[2]=None)

	// Slytherin
	HouseDisplayInfo(3)=(Sex=SX_Male,Mesh=SkeletalMesh'HPModels.skDracoQuidMesh')
	HouseDisplayInfo(3)=(MultiSkins[0]=Texture'HPModels.Skins.skDracoQuidTex0')
	HouseDisplayInfo(3)=(MultiSkins[1]=Texture'HPModels.Skins.skDracoQuidTex1')
	HouseDisplayInfo(3)=(MultiSkins[2]=Texture'HPModels.Skins.skDracoQuidTex2')

	EnemyHealthBar=EnemyBar_Seeker
}

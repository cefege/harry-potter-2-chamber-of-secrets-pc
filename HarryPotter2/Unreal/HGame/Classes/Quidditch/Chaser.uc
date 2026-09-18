//=============================================================================
// Chaser	-- A Quidditch player that handles the Quaffle
//=============================================================================
class Chaser extends QuidditchPlayer;

//-------------------------------------------------------------------------------------------
// States
//
// Fly	- Flying on a path; act like a chaser
//-------------------------------------------------------------------------------------------

state Fly
{
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
		if ( frand() < 0.9 )
			LoopAnim( 'Fly_Forward', , 0.5 );
		else if ( frand() < 0.92 )
		{
			LoopAnim( 'Catch_right', , 0.5 );
			goto 'catch';
		}
		else
			LoopAnim( 'Look', , 0.5 );
	}

	goto 'loop';

catch:
	FinishAnim();
	LoopAnim( 'Hold', , 0.5 );
	FinishAnim();
	Sleep( 3.0 );
	LoopAnim( 'throw_right', , 0.5 );
	goto 'loop';
}


defaultproperties
{
	HouseDisplayInfo(0)=(Sex=SX_Female,Mesh=SkeletalMesh'HPModels.skQuidPlayerFMesh',MultiSkins[0]=Texture'HPModels.Skins.skQuidPlayerF_GTex0')
	HouseDisplayInfo(1)=(Sex=SX_Male,Mesh=SkeletalMesh'HPModels.skQuidPlayerMMesh',MultiSkins[0]=Texture'HPModels.Skins.skQuidPlayerM_RTex0')
	HouseDisplayInfo(2)=(Sex=SX_Female,Mesh=SkeletalMesh'HPModels.skQuidPlayerFMesh',MultiSkins[0]=Texture'HPModels.Skins.skQuidPlayerF_HTex0')
	HouseDisplayInfo(3)=(Sex=SX_Male,Mesh=SkeletalMesh'HPModels.skQuidPlayerMMesh',MultiSkins[0]=Texture'HPModels.Skins.skQuidPlayerM_STex0')
}

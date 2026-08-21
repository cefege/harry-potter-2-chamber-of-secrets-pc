//=============================================================================
// Keeper	-- A Quidditch player that guards the Quaffle goal hoops
//=============================================================================
class Keeper extends QuidditchPlayer;

//-------------------------------------------------------------------------------------------
// States
//
// Fly	- Flying on a path; act like a keeper
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
		if ( frand() < 0.3 )
			LoopAnim( 'Fly_Forward', , 0.5 );
		else if ( frand() < 0.6 )
			LoopAnim( 'Look', , 0.5 );
		else
			LoopAnim( 'Starfish', , 0.5 );
	}

	goto 'loop';
}


defaultproperties
{
	HouseDisplayInfo(0)=(Sex=SX_Male,Mesh=SkeletalMesh'HPModels.skQuidPlayerMMesh',MultiSkins[0]=Texture'HPModels.Skins.skQuidPlayerM_GTex0')
	HouseDisplayInfo(1)=(Sex=SX_Female,Mesh=SkeletalMesh'HPModels.skQuidPlayerFMesh',MultiSkins[0]=Texture'HPModels.Skins.skQuidPlayerF_RTex0')
	HouseDisplayInfo(2)=(Sex=SX_Male,Mesh=SkeletalMesh'HPModels.skQuidPlayerMMesh',MultiSkins[0]=Texture'HPModels.Skins.skQuidPlayerM_HTex0')
	HouseDisplayInfo(3)=(Sex=SX_Female,Mesh=SkeletalMesh'HPModels.skQuidPlayerFMesh',MultiSkins[0]=Texture'HPModels.Skins.skQuidPlayerF_STex0')
}

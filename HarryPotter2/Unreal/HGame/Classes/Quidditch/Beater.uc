//=============================================================================
// Beater	-- A Quidditch player that beats the Bludgers
//=============================================================================
class Beater extends QuidditchPlayer;

//-------------------------------------------------------------------------------------------
// States
//
// Fly	- Flying on a path; act like a beater
//-------------------------------------------------------------------------------------------

state() Fly
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
		if ( bCaughtTarget )
			LoopAnim( 'Hold', , 0.1 );
		else if ( frand() < 0.4 )
			LoopAnim( 'Fly_Forward', , 0.5 );
		else if ( frand() < 0.6 )
			LoopAnim( 'Look', , 0.5 );
		else if ( frand() < 0.7 )
			LoopAnim( 'Hover', , 0.5 );
		else if ( frand() < 0.8 )
			LoopAnim( 'Hit_Bludger_Left', , 0.5 );
		else
			LoopAnim( 'Hit_Bludger_Right', , 0.5 );
	}

	goto 'loop';
}


defaultproperties
{
	Mesh=SkeletalMesh'HPModels.skBeaterMesh'
	HouseDisplayInfo(0)=(Sex=SX_Male,MultiSkins[0]=Texture'HPModels.Skins.skBeater_GTex0')
	HouseDisplayInfo(1)=(Sex=SX_Male,MultiSkins[0]=Texture'HPModels.Skins.skBeater_RTex0')
	HouseDisplayInfo(2)=(Sex=SX_Male,MultiSkins[0]=Texture'HPModels.Skins.skBeater_HTex0')
	HouseDisplayInfo(3)=(Sex=SX_Male,MultiSkins[0]=Texture'HPModels.Skins.skBeater_STex0')
}

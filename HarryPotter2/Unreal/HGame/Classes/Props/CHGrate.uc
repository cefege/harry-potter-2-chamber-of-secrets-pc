
//===============================================================================
//  [Armoire] 
//===============================================================================

class CHGrate extends HProp;

//******************************************************************************************
function Trigger( Actor Other, Pawn EventInstigator )
{
	GotoState('OpenUp');
}

//************************************************************************
state OpenUp
{
  Begin:
	SetCollision(false,false,false);
	PlayAnim('PopOut');
	Sleep( 25.0/30.0 );
	PlaySound( sound'HPSounds.Adv11_COS.floor_grate_landing', SLOT_None, , false, 1000000, RandRange(0.8,1.2) );
}

state idle
{
}

state rattle
{
  Begin:
	PlayAnim('rattle');
	FinishAnim();
	PlayAnim('idle', , 0.4);
	GotoState('idle');
}

//************************************************************************
function OnEvent(name EventName)
{
	super.OnEvent(EventName);

	if( EventName == 'rattle' )
		GotoState('rattle');
}

//******************************************************************************************
defaultproperties
{
     DrawType=DT_Mesh
     Mesh=SkeletalMesh'skCHGrate'
	 bAlignBottom=false
	 bCollideActors=true
	 bBlockActors=true
	 bBlockPlayers=true
	 bCollideworld=false
     CollisionRadius=30
     CollisionHeight=9 //7
	 physics=phys_none
}

//===============================================================================

class Boulder extends HProp;

function bool HandleSpellFlipendo( optional baseSpell spell, optional vector vHitLocation )
{ 
	GotoState( 'patrol' );
	return true;
}

function Trigger( Actor Other, Pawn EventInstigator )
{
	HandleSpellFlipendo( );
}

function _PostPawnAtPatrolPoint(PatrolPoint CurrentP, PatrolPoint NextP)
{
	if( CurrentP.pauseTime > 0 )
	{
		LoopAnim( IdleAnimName );
		GotoState('stateIdle');
	}
	else
	{
		super._PostPawnAtPatrolPoint( CurrentP, NextP );
	}
}

defaultproperties
{
     DrawType=DT_Mesh
     Mesh=SkeletalMesh'HPModels.skboulderMesh'
     //DrawScale=1.0
     CollisionHeight=44
	 CollisionRadius=44
	 bCollideWorld=true
	 Physics=PHYS_Walking

	 walkAnimName="roll"
	 RunAnimName="roll"
	 idleAnimName="stop"

}

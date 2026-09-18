//===============================================================================
//  [Candleflame that moves with a mover] 
//===============================================================================

class CandleF extends HProp;

defaultproperties
{
     DrawType=DT_Sprite
     Style=STY_Translucent
     Texture=Texture'Engine.Can_F'
     Mesh=SkeletalMesh'HProps.skDragonSkullMesh'
     bCollideActors=False
     bCollideWorld=False
     bBlockActors=False
	 bBlockCamera=false
     bBlockPlayers=False
     bAlignBottom=False
}

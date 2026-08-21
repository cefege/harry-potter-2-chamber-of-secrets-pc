//===============================================================================
//  Decanter that spawns beans
//===============================================================================

class DecanterSpawn extends GenericSpawner;

defaultproperties
{
     GoodieToSpawn(0)=Class'HGame.Jellybean'
     Snds=(Opening=Sound'HPSounds.General.spawner_decanter')
     Limits=(Max=2)
     StartBone=StartBone
     StartPos=(Z=0)
     Mesh=SkeletalMesh'HPModels.skdecanterMesh'
     DrawScale=2.5
     AmbientGlow=75
     CollisionRadius=16
     CollideType=CT_AlignedCylinder
}

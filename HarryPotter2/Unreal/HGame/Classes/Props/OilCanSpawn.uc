//===============================================================================
//Oil can that spawns beans
//===============================================================================

class OilCanSpawn extends GenericSpawner;

defaultproperties
{
     GoodieToSpawn(0)=Class'HGame.Jellybean'
     Snds=(Opening=Sound'HPSounds.General.spawner_oil_can')
     Limits=(Max=4,Min=3)
     StartBone=StartBone
     GoodieDelay=0.1
     Lives=2
     Mesh=SkeletalMesh'HPModels.skoilcanMesh'
     DrawScale=3
     AmbientGlow=75
     CollisionRadius=16
     CollisionHeight=20
     CollideType=CT_AlignedCylinder
}

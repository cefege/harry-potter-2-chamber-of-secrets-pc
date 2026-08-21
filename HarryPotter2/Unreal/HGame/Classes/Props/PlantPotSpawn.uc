//===============================================================================
//  PlantPot that spawns beans
//===============================================================================

class PlantPotSpawn extends GenericSpawner;

defaultproperties
{
     GoodieToSpawn(0)=Class'HGame.Jellybean'
     Snds=(Opening=Sound'HPSounds.General.spawner_plant_pot')
     Limits=(Max=3)
     StartBone=StartBone
     GoodieDelay=0.2
     Mesh=SkeletalMesh'HPModels.skemptyplantpotMesh'
     DrawScale=3.5
     AmbientGlow=75
     CollisionRadius=20
     CollisionHeight=16
     CollideType=CT_AlignedCylinder
}

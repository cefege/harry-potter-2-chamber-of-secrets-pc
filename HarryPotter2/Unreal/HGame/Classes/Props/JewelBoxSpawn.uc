//===============================================================================
//  JewelBox that spawns beans
//===============================================================================

class JewelBoxSpawn extends GenericSpawner;

defaultproperties
{
     GoodieToSpawn(0)=Class'HGame.Jellybean'
     Snds=(Opening=Sound'HPSounds.General.spawner_jewel_box')
     Limits=(Min=4)
     Lives=3
     eVulnerableToSpell=SPELL_Alohomora
     Mesh=SkeletalMesh'HPModels.skjewelboxMesh'
     DrawScale=3
     AmbientGlow=75
     CollisionRadius=20
     CollisionWidth=26
     CollideType=CT_Box
}

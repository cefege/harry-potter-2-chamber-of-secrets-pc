//===============================================================================
//Music Box that spawns beans
//===============================================================================

class MusicBoxSpawn extends GenericSpawner;

defaultproperties
{
     GoodieToSpawn(0)=Class'HGame.Jellybean'
     Snds=(Opening=Sound'HPSounds.General.spawner_music_box')
     Limits=(Min=4)
     StartPos=(Z=20)
     Lives=3
     eVulnerableToSpell=SPELL_Alohomora
     Mesh=SkeletalMesh'HPModels.skmusicboxMesh'
     DrawScale=2
     AmbientGlow=75
     CollisionRadius=20
     CollisionWidth=26
     CollisionHeight=16
     CollideType=CT_Box
}

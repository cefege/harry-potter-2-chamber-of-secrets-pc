//===============================================================================
//  Cigar box that spawns beans
//===============================================================================

class CigarBoxSpawn extends GenericSpawner;

defaultproperties
{
     GoodieToSpawn(0)=Class'HGame.Jellybean'
     Snds=(Opening=Sound'HPSounds.General.spawner_cigar_box')
     Limits=(Max=3)
     StartPos=(Z=20)
     GoodieDelay=0.1
     Lives=2
     eVulnerableToSpell=SPELL_Alohomora
     DrawScale=2.5
     AmbientGlow=75
     CollisionRadius=17
     CollisionWidth=22
     CollisionHeight=18
     CollideType=CT_Box
}

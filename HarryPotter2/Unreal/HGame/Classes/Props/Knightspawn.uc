//===============================================================================
//  Suit of armor that spawns beans 
//===============================================================================

class Knightspawn extends GenericSpawner;

defaultproperties
{
     GoodieToSpawn(0)=Class'HGame.Jellybean'
     Snds=(Opening=Sound'HPSounds.General.knight_hit')
     StartPos=(Z=65)
     GoodieDelay=0.25
     Lives=3
     CentreOffset=(Z=40)
     Mesh=SkeletalMesh'HProps.skknightMesh'
     DrawScale=1.5
     AmbientGlow=60
     CollisionRadius=11
     CollisionWidth=28
     CollisionHeight=60
     CollideType=CT_Box
}

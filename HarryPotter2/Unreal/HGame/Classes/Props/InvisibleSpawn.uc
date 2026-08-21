//===============================================================================
//  InvisibleSpawn that spawns beans 
//===============================================================================

class InvisibleSpawn extends GenericSpawner;

defaultproperties
{
     GoodieToSpawn(0)=Class'HGame.Jellybean'
     Snds=(Opening=Sound'HPSounds.General.knight_hit')
     StartPos=(Z=65)
     GoodieDelay=0.25
     Lives=3
     bHidden=True
     Physics=PHYS_None
     CentreOffset=(Z=40)
     DrawType=DT_Sprite
     Mesh=None
     DrawScale=1.5
     AmbientGlow=60
     CollisionRadius=32
     CollisionHeight=32
     CollideType=CT_OrientedCylinder
     bBlockPlayers=False
}

//===============================================================================
//  SpellBallTrail 
//
//===============================================================================

class SpellBallTrail extends HProp;

defaultproperties
{
	LifeSpan=4.0
    //Rotation=(Roll=-16320)
    Style=STY_Translucent
    Mesh=SkeletalMesh'HProps.skSheetTestMesh'
    DrawScale=0.1
    bUnlit=True
    MultiSkins(0)=FireTexture'HPParticle.hp_fx.Particles.LessonTrail'
    CollisionRadius=10
    CollisionHeight=5
    bCollideWorld=False
    bBlockActors=False
    bBlockPlayers=False
	bBlockCamera=false
}

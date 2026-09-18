//=============================================================================
// SpellLessonShape
//=============================================================================

class SpellLessonShape extends HProp;

defaultproperties
{
    Style=STY_Translucent
	DrawType=DT_MESH
    Mesh=SkeletalMesh'HProps.skSheetTestMesh'
    bUnlit=True
	MultiSkins(0)=Texture'SpellShapes.Shapes.SPSrictusempra'
    //MultiSkins(0)=FireTexture'HPParticle.hp_fx.Particles.Silverslime'
    CollisionRadius=10
    CollisionHeight=5
    bCollideWorld=False
    bBlockActors=False
    bBlockPlayers=False
	bBlockCamera=false
}

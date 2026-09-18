//===============================================================================
//  lesson background texture
//===============================================================================

class lessonbackground extends HProp;

defaultproperties
{
     Rotation=(Roll=-16320)
     Style=STY_Masked
     Mesh=SkeletalMesh'HProps.skSheetTestMesh'
     AmbientGlow=0
     Opacity=0.5
     MultiSkins(0)=WetTexture'HPParticle.hp_fx.General.LessonBackWet'
     bCollideWorld=False
     bBlockActors=False
     bBlockPlayers=False
}

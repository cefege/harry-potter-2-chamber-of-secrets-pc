//===============================================================================
//  Snake ray mesh for rays coming out of Basilisk eyes
//===============================================================================

class SnakeRay extends HProp;

defaultproperties
{
     Style=STY_Translucent
     Mesh=SkeletalMesh'HProps.skSnakeRayMesh'
     AmbientGlow=200
     MultiSkins(0)=WetTexture'HPParticle.hp_fx.General.SnakeEyesWet'
}

//===============================================================================
//  [HogFrontTorch] with FireHP2_bigtorch1 particle fx attached
//===============================================================================

class HogFrontTorch_WithFlame extends HogFrontTorch;

defaultproperties
{
     attachedParticleClass(0)=Class'HPParticle.FireHP2_bigtorch1'
     attachedParticleOffset(0)=(Y=15,Z=7)
     Style=STY_Masked
     Mesh=SkeletalMesh'HProps.skHogFrontTorchMesh'
     DrawScale=1.8
     CollisionRadius=50
     CollisionHeight=50
}

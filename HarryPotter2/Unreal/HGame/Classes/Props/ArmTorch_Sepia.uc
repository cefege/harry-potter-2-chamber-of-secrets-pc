//===============================================================================
//  [skArmTorch] 
//===============================================================================

class ArmTorch_Sepia extends ArmTorch;


defaultproperties
{
    DrawType=DT_Mesh
    Mesh=SkeletalMesh'HProps.skArmTorchSepiaMesh'
    DrawType=DT_Mesh
    bStatic=False
    attachedParticleClass(0)=Class'HPParticle.FireHP2Sepia'
    attachedParticleOffset(0)=(X=-18,Y=-4,Z=14)
}


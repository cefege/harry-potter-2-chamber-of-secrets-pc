//===============================================================================
//  [WizzardCardIcon] 
//===============================================================================

class skWizardCardIcon extends HPMeshActor;

#exec MESH  MODELIMPORT MESH=skWizardCardIconMesh MODELFILE=models\skWizardCardIcon.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skWizardCardIconMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skWizardCardIconAnims ANIMFILE=models\skWizardCardIcon.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skWizardCardIconMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skWizardCardIconMesh ANIM=skWizardCardIconAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skWizardCardIconAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=WizardCardIconTex0  FILE=TEXTURES\WizardCardIcon.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skWizardCardIconMesh NUM=0 TEXTURE=WizardCardIconTex0

defaultproperties
{
     bStatic=False
     DrawType=DT_Mesh
     Mesh=SkeletalMesh'HProps.skWizardCardIconMesh'
}

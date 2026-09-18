//===============================================================================
//  [skBlackboardWizDuel] 
//===============================================================================

class skBlackboardWizDuel extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBlackboardWizDuelMesh MODELFILE=models\skBlackboardWizDuel.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBlackboardWizDuelMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBlackboardWizDuelAnims ANIMFILE=models\skBlackboardWizDuel.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBlackboardWizDuelMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBlackboardWizDuelMesh ANIM=skBlackboardWizDuelAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBlackboardWizDuelAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBlackboardWizDuelTex0  FILE=TEXTURES\WizardDuelblackboard.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBlackboardWizDuelMesh NUM=0 TEXTURE=skBlackboardWizDuelTex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: WizardDuelblackboard.bmp  Path: C:\Harry Potter 2\ART\Objects\Blackboards\Standup 


defaultproperties
{
    Mesh=skBlackboardWizDuelMesh
    DrawType=DT_Mesh
    bStatic=False
}


//===============================================================================
//  [skSculptureClawFoot] 
//===============================================================================

class skSculptureClawFoot extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skSculptureClawFootMesh MODELFILE=models\skSculptureClawFoot.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skSculptureClawFootMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skSculptureClawFootAnims ANIMFILE=models\skSculptureClawFoot.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skSculptureClawFootMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skSculptureClawFootMesh ANIM=skSculptureClawFootAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skSculptureClawFootAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skSculptureClawFootTex0  FILE=TEXTURES\clawfoot_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skSculptureClawFootMesh NUM=0 TEXTURE=skSculptureClawFootTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: clawfoot_128.bmp  Path: C:\Harry Potter\ART\Objects\Statues_Sculptures\Greenhouse Clawfoot 


defaultproperties
{
    Mesh=skSculptureClawFootMesh
    DrawType=DT_Mesh
    bStatic=False
}


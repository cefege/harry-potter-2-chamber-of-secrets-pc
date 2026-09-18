//===============================================================================
//  [skSundial] 
//===============================================================================

class skSundial extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skSundialMesh MODELFILE=models\skSundial.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skSundialMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skSundialAnims ANIMFILE=models\skSundial.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skSundialMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skSundialMesh ANIM=skSundialAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skSundialAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skSundialTex0  FILE=TEXTURES\sundial_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skSundialMesh NUM=0 TEXTURE=skSundialTex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: sundial_128.bmp  Path: C:\Harry Potter\ART\Objects\skSundial 


defaultproperties
{
    Mesh=skSundialMesh
    DrawType=DT_Mesh
    bStatic=False
}


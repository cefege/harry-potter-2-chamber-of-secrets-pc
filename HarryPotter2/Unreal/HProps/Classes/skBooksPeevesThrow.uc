//===============================================================================
//  [skBooksPeevesThrow] 
//===============================================================================

class skBooksPeevesThrow extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBooksPeevesThrowMesh MODELFILE=models\skBooksPeevesThrow.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBooksPeevesThrowMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBooksPeevesThrowAnims ANIMFILE=models\skBooksPeevesThrow.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBooksPeevesThrowMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBooksPeevesThrowMesh ANIM=skBooksPeevesThrowAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBooksPeevesThrowAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBooksPeevesThrowTex0  FILE=TEXTURES\pevebook_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBooksPeevesThrowMesh NUM=0 TEXTURE=skBooksPeevesThrowTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: pevebook_128.bmp  Path: C:\Harry Potter\ART\Objects\Books\Peeves Book 


defaultproperties
{
    Mesh=skBooksPeevesThrowMesh
    DrawType=DT_Mesh
    bStatic=False
}


//===============================================================================
//  [skFramesEmpty] 
//===============================================================================

class skFramesEmpty extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skFramesEmptyMesh MODELFILE=models\skFramesEmpty.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skFramesEmptyMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skFramesEmptyAnims ANIMFILE=models\skFramesEmpty.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skFramesEmptyMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skFramesEmptyMesh ANIM=skFramesEmptyAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skFramesEmptyAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skFramesEmptyTex0  FILE=TEXTURES\forbidfr_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skFramesEmptyMesh NUM=0 TEXTURE=skFramesEmptyTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: forbidfr_128.bmp  Path: C:\Harry Potter\ART\Objects\Pictures_Frames\Empty Frames 


defaultproperties
{
    Mesh=skFramesEmptyMesh
    DrawType=DT_Mesh
    bStatic=False
}


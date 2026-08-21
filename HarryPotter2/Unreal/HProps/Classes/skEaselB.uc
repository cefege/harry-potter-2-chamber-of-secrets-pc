//===============================================================================
//  [skEaselB] 
//===============================================================================

class skEaselB extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skEaselBMesh MODELFILE=models\skEaselB.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skEaselBMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skEaselBAnims ANIMFILE=models\skEaselB.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skEaselBMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skEaselBMesh ANIM=skEaselBAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skEaselBAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skEaselBTex0  FILE=TEXTURES\EisleMap2_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skEaselBMesh NUM=0 TEXTURE=skEaselBTex0

// Original material [0] is [SKIN00.TWOSIDDED] SkinIndex: 0 Bitmap: EisleMap2_128.bmp  Path: C:\HP2_master\Dada\Objects 


defaultproperties
{
    Mesh=skEaselBMesh
    DrawType=DT_Mesh
    bStatic=False
}


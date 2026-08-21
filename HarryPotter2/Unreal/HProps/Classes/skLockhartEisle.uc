//===============================================================================
//  [skLockhartEisle] 
//===============================================================================

class skLockhartEisle extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skLockhartEisleMesh MODELFILE=models\skLockhartEisle.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skLockhartEisleMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skLockhartEisleAnims ANIMFILE=models\skLockhartEisle.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skLockhartEisleMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skLockhartEisleMesh ANIM=skLockhartEisleAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skLockhartEisleAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skLockhartEisleTex0  FILE=TEXTURES\lockhart_EisleMap.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skLockhartEisleMesh NUM=0 TEXTURE=skLockhartEisleTex0

// Original material [0] is [SKIN00.TWOSIDDED] SkinIndex: 0 Bitmap: lockhart_EisleMap.bmp  Path: C:\HP2_master\Dada\Objects 


defaultproperties
{
    Mesh=skLockhartEisleMesh
    DrawType=DT_Mesh
    bStatic=False
}


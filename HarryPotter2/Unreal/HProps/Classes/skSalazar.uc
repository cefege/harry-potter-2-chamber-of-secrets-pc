//===============================================================================
//  [skSalazar] 
//===============================================================================

class skSalazar extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skSalazarMesh MODELFILE=models\skSalazar.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skSalazarMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skSalazarAnims ANIMFILE=models\skSalazar.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skSalazarMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skSalazarMesh ANIM=skSalazarAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skSalazarAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skSalazarTex0  FILE=TEXTURES\HP2SAL_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skSalazarTex1  FILE=TEXTURES\HP2SAL_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skSalazarTex2  FILE=TEXTURES\HP2SAL_SKIN02.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skSalazarTex3  FILE=TEXTURES\HP2SAL_SKIN03.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skSalazarMesh NUM=0 TEXTURE=skSalazarTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skSalazarMesh NUM=1 TEXTURE=skSalazarTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skSalazarMesh NUM=2 TEXTURE=skSalazarTex2
#EXEC MESHMAP SETTEXTURE MESHMAP=skSalazarMesh NUM=3 TEXTURE=skSalazarTex3

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2SAL_SKIN00.bmp  Path: C:\HP_2\HP2 Characters\HP2_Salazar 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2SAL_SKIN01.bmp  Path: C:\HP_2\HP2 Characters\HP2_Salazar 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: HP2SAL_SKIN02.bmp  Path: C:\HP_2\HP2 Characters\HP2_Salazar 
// Original material [3] is [SKIN03] SkinIndex: 3 Bitmap: HP2SAL_SKIN03.bmp  Path: C:\HP_2\HP2 Characters\HP2_Salazar 


defaultproperties
{
    Mesh=skSalazarMesh
    DrawType=DT_Mesh
    bStatic=False
}


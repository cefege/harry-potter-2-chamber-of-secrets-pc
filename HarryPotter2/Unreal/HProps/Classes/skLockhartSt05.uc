//===============================================================================
//  [skLockhartSt05] 
//===============================================================================

class skLockhartSt05 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skLockhartSt05Mesh MODELFILE=models\skLockhartSt05.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skLockhartSt05Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skLockhartSt05Anims ANIMFILE=models\skLockhartSt05.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skLockhartSt05Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skLockhartSt05Mesh ANIM=skLockhartSt05Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skLockhartSt05Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skLockhartSt05Tex0  FILE=TEXTURES\HP2LOCKST_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skLockhartSt05Tex1  FILE=TEXTURES\HP2LOCKST_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skLockhartSt05Mesh NUM=0 TEXTURE=skLockhartSt05Tex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skLockhartSt05Mesh NUM=1 TEXTURE=skLockhartSt05Tex1

// Original material [0] is [skin00] SkinIndex: 0 Bitmap: HP2LOCKST_SKIN00.bmp  Path: C:\HP_2\HP2 Characters\HP2_GilderoyLockhartStatues 
// Original material [1] is [skin01] SkinIndex: 1 Bitmap: HP2LOCKST_SKIN01.bmp  Path: C:\HP_2\HP2 Characters\HP2_GilderoyLockhartStatues 


defaultproperties
{
    Mesh=skLockhartSt05Mesh
    DrawType=DT_Mesh
    bStatic=False
}


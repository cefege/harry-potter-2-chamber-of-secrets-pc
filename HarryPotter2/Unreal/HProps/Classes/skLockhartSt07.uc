//===============================================================================
//  [skLockhartSt07] 
//===============================================================================

class skLockhartSt07 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skLockhartSt07Mesh MODELFILE=models\skLockhartSt07.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skLockhartSt07Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skLockhartSt07Anims ANIMFILE=models\skLockhartSt07.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skLockhartSt07Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skLockhartSt07Mesh ANIM=skLockhartSt07Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skLockhartSt07Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skLockhartSt07Tex0  FILE=TEXTURES\HP2LOCKST_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skLockhartSt07Tex1  FILE=TEXTURES\HP2LOCKST_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skLockhartSt07Mesh NUM=0 TEXTURE=skLockhartSt07Tex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skLockhartSt07Mesh NUM=1 TEXTURE=skLockhartSt07Tex1

// Original material [0] is [skin00] SkinIndex: 0 Bitmap: HP2LOCKST_SKIN00.bmp  Path: C:\HP_2\HP2 Characters\HP2_GilderoyLockhartStatues 
// Original material [1] is [skin01] SkinIndex: 1 Bitmap: HP2LOCKST_SKIN01.bmp  Path: C:\HP_2\HP2 Characters\HP2_GilderoyLockhartStatues 


defaultproperties
{
    Mesh=skLockhartSt07Mesh
    DrawType=DT_Mesh
    bStatic=False
}


//===============================================================================
//  [skBlueJellyBean] 
//===============================================================================

class skBlueJellyBean extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBlueJellyBeanMesh MODELFILE=models\skBlueJellyBean.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBlueJellyBeanMesh X=0 Y=0 Z=16 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBlueJellyBeanAnims ANIMFILE=models\skBlueJellyBean.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBlueJellyBeanMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBlueJellyBeanMesh ANIM=skBlueJellyBeanAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBlueJellyBeanAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBlueJellyBeanTex0  FILE=TEXTURES\bluebean_64.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBlueJellyBeanMesh NUM=0 TEXTURE=skBlueJellyBeanTex0

// Original material [0] is [Material #25] SkinIndex: 0 Bitmap: bluebean_64.bmp  Path: D:\Harry Potter\A Lorian's Stuff\Hogwarts\General Objects 


defaultproperties
{
    Mesh=skBlueJellyBeanMesh
    DrawType=DT_Mesh
    bStatic=False
}


//===============================================================================
//  [skSpottedJellyBean] 
//===============================================================================

class skSpottedJellyBean extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skSpottedJellyBeanMesh MODELFILE=models\skSpottedJellyBean.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skSpottedJellyBeanMesh X=0 Y=0 Z=16 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skSpottedJellyBeanAnims ANIMFILE=models\skSpottedJellyBean.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skSpottedJellyBeanMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skSpottedJellyBeanMesh ANIM=skSpottedJellyBeanAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skSpottedJellyBeanAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skSpottedJellyBeanTex0  FILE=TEXTURES\spotbean_64.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skSpottedJellyBeanMesh NUM=0 TEXTURE=skSpottedJellyBeanTex0

// Original material [0] is [Material #25] SkinIndex: 0 Bitmap: spotbean_64.bmp  Path: D:\Harry Potter\A Lorian's Stuff\Hogwarts\General Objects 


defaultproperties
{
    Mesh=skSpottedJellyBeanMesh
    DrawType=DT_Mesh
    bStatic=False
}


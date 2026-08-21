//===============================================================================
//  [skGreenJellyBean] 
//===============================================================================

class skGreenJellyBean extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skGreenJellyBeanMesh MODELFILE=models\skGreenJellyBean.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skGreenJellyBeanMesh X=0 Y=0 Z=16 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skGreenJellyBeanAnims ANIMFILE=models\skGreenJellyBean.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skGreenJellyBeanMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skGreenJellyBeanMesh ANIM=skGreenJellyBeanAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skGreenJellyBeanAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skGreenJellyBeanTex0  FILE=TEXTURES\grenbean_64.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skGreenJellyBeanMesh NUM=0 TEXTURE=skGreenJellyBeanTex0

// Original material [0] is [Material #25] SkinIndex: 0 Bitmap: grenbean_64.bmp  Path: D:\Harry Potter\A Lorian's Stuff\Hogwarts\General Objects 


defaultproperties
{
    Mesh=skGreenJellyBeanMesh
    DrawType=DT_Mesh
    bStatic=False
}


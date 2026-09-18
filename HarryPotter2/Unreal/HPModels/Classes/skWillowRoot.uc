//===============================================================================
//  [skWillowRoot] 
//===============================================================================

class skWillowRoot extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skWillowRootMesh MODELFILE=models\skWillowRoot.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skWillowRootMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skWillowRootAnims ANIMFILE=models\skWillowroot.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skWillowRootMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skWillowRootMesh ANIM=skWillowRootAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skWillowRootAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skWillowRootTex0  FILE=TEXTURES\HP2_WWDooDoo.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skWillowRootTex1  FILE=TEXTURES\HP2_WWWhipperTrans.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skWillowRootMesh NUM=0 TEXTURE=skWillowRootTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skWillowRootMesh NUM=1 TEXTURE=skWillowRootTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2_WWDooDoo.bmp  Path: C:\potter\Characters\HP2\WhompingWillow 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2_WWWhipperTrans.bmp  Path: C:\potter\Characters\HP2\WhompingWillow 

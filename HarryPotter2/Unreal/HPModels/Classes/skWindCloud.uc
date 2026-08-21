//===============================================================================
//  [skWindCloud] 
//===============================================================================

class skWindCloud extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skWindCloudMesh MODELFILE=models\skWindCloud.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skWindCloudMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skWindCloudAnims ANIMFILE=models\skWindCloud.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skWindCloudMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skWindCloudMesh ANIM=skWindCloudAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST ANIM=skWindCloudAnims USERAWINFO VERBOSE

#EXEC TEXTURE IMPORT NAME=skWindCloudTex0  FILE=TEXTURES\WindCloud2.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skWindCloudMesh NUM=0 TEXTURE=skWindCloudTex0

// Original material [0] is [SKIN00.TRANS] SkinIndex: 0 Bitmap: WindCloud2.bmp  Path: C:\HP2 Art\Textures\Flying Ford\Lightning Cloud 

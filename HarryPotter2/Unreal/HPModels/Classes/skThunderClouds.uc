//===============================================================================
//  [skThunderClouds] 
//===============================================================================

class skThunderClouds extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skThunderCloudsMesh MODELFILE=models\skThunderClouds.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skThunderCloudsMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skThunderCloudsAnims ANIMFILE=models\skThunderClouds.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skThunderCloudsMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skThunderCloudsMesh ANIM=skThunderCloudsAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST ANIM=skThunderCloudsAnims USERAWINFO VERBOSE

#EXEC TEXTURE IMPORT NAME=skThunderCloudsTex0  FILE=TEXTURES\WindCloud2.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skThunderCloudsTex1  FILE=TEXTURES\WindCloud.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skThunderCloudsMesh NUM=0 TEXTURE=skThunderCloudsTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skThunderCloudsMesh NUM=1 TEXTURE=skThunderCloudsTex1

// Original material [0] is [SKIN00.TRANS] SkinIndex: 0 Bitmap: WindCloud2.bmp  Path: C:\HP2 Art\Textures\Flying Ford\Lightning Cloud 
// Original material [1] is [SKIN01.TRANS] SkinIndex: 1 Bitmap: WindCloud.bmp  Path: C:\HP2 Art\Textures\Flying Ford\Lightning Cloud 

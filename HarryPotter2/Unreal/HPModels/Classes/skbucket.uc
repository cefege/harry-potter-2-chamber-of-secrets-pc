//===============================================================================
//  [skbucket] 
//===============================================================================

class skbucket extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skbucketMesh MODELFILE=models\skbucket.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skbucketMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skbucketAnims ANIMFILE=models\skbucket.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skbucketMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skbucketMesh ANIM=skbucketAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skbucketAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skbucketTex0  FILE=TEXTURES\fpbucket_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skbucketMesh NUM=0 TEXTURE=skbucketTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: fpbucket_128.bmp  Path: C:\Nathan 


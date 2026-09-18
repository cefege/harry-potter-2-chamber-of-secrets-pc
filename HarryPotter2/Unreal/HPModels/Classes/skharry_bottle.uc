//===============================================================================
//  [skharry_bottle] 
//===============================================================================

class skharry_bottle extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skharry_bottleMesh MODELFILE=models\skharry_bottle.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skharry_bottleMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skharry_bottleAnims ANIMFILE=models\skharry_bottle.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skharry_bottleMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skharry_bottleMesh ANIM=skharry_bottleAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skharry_bottleAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skharry_bottleTex0  FILE=TEXTURES\H_greenbot_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skharry_bottleMesh NUM=0 TEXTURE=skharry_bottleTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: H_greenbot_128.bmp  Path: C:\HP2_Objects\Bottles 

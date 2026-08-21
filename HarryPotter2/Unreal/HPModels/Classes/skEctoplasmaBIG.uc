//===============================================================================
//  [skEctoplasmaBIG] 
//===============================================================================

class skEctoplasmaBIG extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skEctoplasmaBIGMesh MODELFILE=models\skEctoplasmaBIG.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skEctoplasmaBIGMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skEctoplasmaBIGAnims ANIMFILE=models\skEctoplasmaBIG.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skEctoplasmaBIGMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skEctoplasmaBIGMesh ANIM=skEctoplasmaBIGAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skEctoplasmaBIGAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skEctoplasmaBIGTex0  FILE=TEXTURES\ectoplasm.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skEctoplasmaBIGMesh NUM=0 TEXTURE=skEctoplasmaBIGTex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: ectoplasm.bmp  Path: C:\potter\Characters\HP2\EktoPlasm 

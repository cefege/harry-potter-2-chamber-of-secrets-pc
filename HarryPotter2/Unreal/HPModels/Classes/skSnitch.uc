//===============================================================================
//  [skSnitch] 
//===============================================================================

class skSnitch extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skSnitchMesh MODELFILE=models\skSnitch.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skSnitchMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skSnitchAnims ANIMFILE=models\skSnitch.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skSnitchMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skSnitchMesh ANIM=skSnitchAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skSnitchAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skSnitchTex0  FILE=TEXTURES\glsnitch_64.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skSnitchMesh NUM=0 TEXTURE=skSnitchTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: glsnitch_64.bmp  Path: D:\Harry Potter\Art\Objects\General Objects\Golden Snitch 

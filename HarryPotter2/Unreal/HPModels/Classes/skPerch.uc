//===============================================================================
//  [skPerch] 
//===============================================================================

class skPerch extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skPerchMesh MODELFILE=models\skPerch.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skPerchMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skPerchAnims ANIMFILE=models\skperch.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skPerchMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skPerchMesh ANIM=skPerchAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skPerchAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skPerchTex0  FILE=TEXTURES\perch_shiny.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skPerchMesh NUM=0 TEXTURE=skPerchTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: perch_shiny.bmp  Path: C:\hp2_characters\hp2_fawkes 

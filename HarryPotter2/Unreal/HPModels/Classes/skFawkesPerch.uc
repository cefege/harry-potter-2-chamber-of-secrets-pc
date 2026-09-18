//===============================================================================
//  [skFawkesPerch] 
//===============================================================================

class skFawkesPerch extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skFawkesPerchMesh MODELFILE=models\skFawkesPerch.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skFawkesPerchMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skFawkesPerchAnims ANIMFILE=models\skFawkesPerch.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skFawkesPerchMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skFawkesPerchMesh ANIM=skFawkesPerchAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skFawkesPerchAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skFawkesPerchTex0  FILE=TEXTURES\HP2FAWKES_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skFawkesPerchTex1  FILE=TEXTURES\perch_shiny.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skFawkesPerchMesh NUM=0 TEXTURE=skFawkesPerchTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skFawkesPerchMesh NUM=1 TEXTURE=skFawkesPerchTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2FAWKES_SKIN00.bmp  Path: C:\hp2_characters\hp2_fawkes 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: perch_shiny.bmp  Path: C:\hp2_characters\hp2_fawkes 

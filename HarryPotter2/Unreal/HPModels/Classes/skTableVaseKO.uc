//===============================================================================
//  [skTableVaseKO] 
//===============================================================================

class skTableVaseKO extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skTableVaseKOMesh MODELFILE=models\skTableVaseKO.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skTableVaseKOMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skTableVaseKOAnims ANIMFILE=models\skTableVaseKO.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skTableVaseKOMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skTableVaseKOMesh ANIM=skTableVaseKOAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skTableVaseKOAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skTableVaseKOTex0  FILE=TEXTURES\hogroundtable_128.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skTableVaseKOTex1  FILE=TEXTURES\HWvaseTW_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skTableVaseKOMesh NUM=0 TEXTURE=skTableVaseKOTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skTableVaseKOMesh NUM=1 TEXTURE=skTableVaseKOTex1

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: hogroundtable_128.bmp  Path: H:\Art\AnimDump\KnockOverVase 
// Original material [1] is [SKIN01.MASKED] SkinIndex: 1 Bitmap: HWvaseTW_128.bmp  Path: H:\Art\AnimDump\KnockOverVase 

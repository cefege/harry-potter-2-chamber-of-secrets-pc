//===============================================================================
//  [skLuciousMalfoy] 
//===============================================================================

class skLuciousMalfoy extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skLuciousMalfoyMesh MODELFILE=models\skLuciousMalfoy.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skLuciousMalfoyMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skLuciousMalfoyAnims ANIMFILE=models\skLuciousMalfoy.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skLuciousMalfoyMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skLuciousMalfoyMesh ANIM=skLuciousMalfoyAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skLuciousMalfoyAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skLuciousMalfoyTex0  FILE=TEXTURES\HP2LUC_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skLuciousMalfoyTex1  FILE=TEXTURES\HP2LUC_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skLuciousMalfoyMesh NUM=0 TEXTURE=skLuciousMalfoyTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skLuciousMalfoyMesh NUM=1 TEXTURE=skLuciousMalfoyTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2LUC_SKIN00.bmp  Path: C:\potter\Characters\HP2\LuciousMalfoy 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2LUC_SKIN01.bmp  Path: C:\potter\Characters\HP2\LuciousMalfoy 

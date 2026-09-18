//===============================================================================
//  [skdevilplant] 
//===============================================================================

class skdevilplant extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skdevilplantMesh MODELFILE=models\skdevilplant.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skdevilplantMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skdevilplantAnims ANIMFILE=models\skdevilplant.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skdevilplantMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skdevilplantMesh ANIM=skdevilplantAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skdevilplantAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skdevilplantTex0  FILE=TEXTURES\DEVILPLANT_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skdevilplantTex1  FILE=TEXTURES\DEVILPLANT_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skdevilplantMesh NUM=0 TEXTURE=skdevilplantTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skdevilplantMesh NUM=1 TEXTURE=skdevilplantTex1

// Original material [0] is [DEVILPLANT_SKIN00] SkinIndex: 0 Bitmap: DEVILPLANT_SKIN00.bmp  Path: H:\Art\Design\Creatures\Devil's Snare 
// Original material [1] is [DEVILPLANT_SKIN01] SkinIndex: 1 Bitmap: DEVILPLANT_SKIN01.bmp  Path: H:\Art\Design\Creatures\Devil's Snare 


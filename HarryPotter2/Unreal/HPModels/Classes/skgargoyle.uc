//===============================================================================
//  [skgargoyle] 
//===============================================================================

class skgargoyle extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skgargoyleMesh MODELFILE=models\skgargoyle.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skgargoyleMesh X=0 Y=0 Z=30 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skgargoyleAnims ANIMFILE=models\skgargoyle.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skgargoyleMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skgargoyleMesh ANIM=skgargoyleAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skgargoyleAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skgargoyleTex0  FILE=TEXTURES\gargoyle_256.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skgargoyleMesh NUM=0 TEXTURE=skgargoyleTex0

// Original material [0] is [GARGOYLE_SKIN00] SkinIndex: 0 Bitmap: gargoyle_256.bmp  Path: H:\Art\Design\Creatures\Gargoyle 


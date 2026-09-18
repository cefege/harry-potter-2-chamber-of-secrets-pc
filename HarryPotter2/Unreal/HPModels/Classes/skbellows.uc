//===============================================================================
//  [skbellows] 
//===============================================================================

class skbellows extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skbellowsMesh MODELFILE=models\skbellows.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skbellowsMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skbellowsAnims ANIMFILE=models\skbellows.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skbellowsMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skbellowsMesh ANIM=skbellowsAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skbellowsAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skbellowsTex0  FILE=TEXTURES\HP2BELLOW_SKIN00.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skbellowsMesh NUM=0 TEXTURE=skbellowsTex0

// Original material [0] is [BELLOWS] SkinIndex: 0 Bitmap: HP2BELLOW_SKIN00.bmp  Path: C:\potter\Objects\FlipendoObjects\Bellows 

//===============================================================================
//  [skMrsNorris] 
//===============================================================================

class skMrsNorris extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skMrsNorrisMesh MODELFILE=models\skMrsNorris.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skMrsNorrisMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skMrsNorrisAnims ANIMFILE=models\skMrsNorris.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skMrsNorrisMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skMrsNorrisMesh ANIM=skMrsNorrisAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skMrsNorrisAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skMrsNorrisTex0  FILE=TEXTURES\Norris_Skin00.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skMrsNorrisMesh NUM=0 TEXTURE=skMrsNorrisTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: Norris_Skin00.bmp  Path: C:\potter\Characters\HP2\MrsNorris 

//===============================================================================
//  [skSortingHat] 
//===============================================================================

class skSortingHat extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skSortingHatMesh MODELFILE=models\skSortingHat.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skSortingHatMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skSortingHatAnims ANIMFILE=models\skSortingHat.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skSortingHatMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skSortingHatMesh ANIM=skSortingHatAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skSortingHatAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skSortingHatTex0  FILE=TEXTURES\HP2SORTHAT_SKIN00.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skSortingHatMesh NUM=0 TEXTURE=skSortingHatTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2SORTHAT_SKIN00.bmp  Path: C:\potter\Characters\HP2\SortingHat 

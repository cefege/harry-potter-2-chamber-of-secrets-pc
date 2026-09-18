//===============================================================================
//  [skFawkes] 
//===============================================================================

class skFawkes extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skFawkesMesh MODELFILE=models\skFawkes.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skFawkesMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skFawkesAnims ANIMFILE=models\skFawkes.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skFawkesMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skFawkesMesh ANIM=skFawkesAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skFawkesAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skFawkesTex0  FILE=TEXTURES\HP2FAWKES_SKIN00.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skFawkesMesh NUM=0 TEXTURE=skFawkesTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2FAWKES_SKIN00.bmp  Path: C:\potter\Characters\HP2\Fawkes 

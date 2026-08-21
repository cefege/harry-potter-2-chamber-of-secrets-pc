//===============================================================================
//  [skSpikyPlant] 
//===============================================================================

class skSpikyPlant extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skSpikyPlantMesh MODELFILE=models\skSpikyPlant.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skSpikyPlantMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skSpikyPlantAnims ANIMFILE=models\skSpikyPlant.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skSpikyPlantMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skSpikyPlantMesh ANIM=skSpikyPlantAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skSpikyPlantAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skSpikyPlantTex0  FILE=TEXTURES\SPIKYBUSH_SKIN00.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skSpikyPlantMesh NUM=0 TEXTURE=skSpikyPlantTex0

// Original material [0] is [SPIKYBUSH_SKIN00] SkinIndex: 0 Bitmap: SPIKYBUSH_SKIN00.bmp  Path: C:\potter\Characters\HP2\SpikyPlant 

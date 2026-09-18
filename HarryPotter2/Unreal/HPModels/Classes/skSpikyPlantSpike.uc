//===============================================================================
//  [skSpikyPlantSpike] 
//===============================================================================

class skSpikyPlantSpike extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skSpikyPlantSpikeMesh MODELFILE=models\skSpikyPlantSpike.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skSpikyPlantSpikeMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skSpikyPlantSpikeAnims ANIMFILE=models\skSpikyPlantSpike.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skSpikyPlantSpikeMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skSpikyPlantSpikeMesh ANIM=skSpikyPlantSpikeAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skSpikyPlantSpikeAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skSpikyPlantSpikeTex0  FILE=TEXTURES\SPIKYBUSH_SKIN00.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skSpikyPlantSpikeMesh NUM=0 TEXTURE=skSpikyPlantSpikeTex0

// Original material [0] is [SPIKYBUSH_SKIN00] SkinIndex: 0 Bitmap: SPIKYBUSH_SKIN00.bmp  Path: C:\potter\Characters\HP2\SpikyPlant 

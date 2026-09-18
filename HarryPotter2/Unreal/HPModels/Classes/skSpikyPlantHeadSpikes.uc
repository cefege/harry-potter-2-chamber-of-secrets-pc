//===============================================================================
//  [skSpikyPlantHeadSpikes] 
//===============================================================================

class skSpikyPlantHeadSpikes extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skSpikyPlantHeadSpikesMesh MODELFILE=models\skSpikyPlantHeadSpikes.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skSpikyPlantHeadSpikesMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skSpikyPlantHeadSpikesAnims ANIMFILE=models\skSpikyPlantHeadSpikes.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skSpikyPlantHeadSpikesMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skSpikyPlantHeadSpikesMesh ANIM=skSpikyPlantHeadSpikesAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skSpikyPlantHeadSpikesAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skSpikyPlantHeadSpikesTex0  FILE=TEXTURES\SPIKYBUSH_SKIN00.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skSpikyPlantHeadSpikesMesh NUM=0 TEXTURE=skSpikyPlantHeadSpikesTex0

// Original material [0] is [SPIKYBUSH_SKIN00] SkinIndex: 0 Bitmap: SPIKYBUSH_SKIN00.bmp  Path: C:\potter\Characters\HP2\SpikyPlant 

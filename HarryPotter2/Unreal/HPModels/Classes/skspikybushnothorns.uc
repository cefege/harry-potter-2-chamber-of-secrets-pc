//===============================================================================
//  [skspikybushnothorns] 
//===============================================================================

class skspikybushnothorns extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skspikybushnothornsMesh MODELFILE=models\skspikybushnothorns.PSK LODSTYLE=10
//#exec MESH  ORIGIN MESH=skspikybushnothornsMesh X=0 Y=0 Z=20 YAW=0 PITCH=0 ROLL=0
#exec MESH  ORIGIN MESH=skspikybushnothornsMesh X=0 Y=0 Z=0 6YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skspikybushnothornsAnims ANIMFILE=models\skspikybushnothorns.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skspikybushnothornsMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skspikybushnothornsMesh ANIM=skspikybushnothornsAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skspikybushnothornsAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skspikybushnothornsTex0  FILE=TEXTURES\SPIKYBUSH_SKIN00.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skspikybushnothornsMesh NUM=0 TEXTURE=skspikybushnothornsTex0

// Original material [0] is [SPIKYBUSH_SKIN00] SkinIndex: 0 Bitmap: SPIKYBUSH_SKIN00.bmp  Path: H:\Art\Design\Creatures\Spiky Bush 


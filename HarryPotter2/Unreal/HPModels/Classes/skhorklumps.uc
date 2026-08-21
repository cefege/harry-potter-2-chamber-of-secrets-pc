//===============================================================================
//  [skhorklumps] 
//===============================================================================

class skhorklumps extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skhorklumpsMesh MODELFILE=models\skhorklumps.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skhorklumpsMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skhorklumpsAnims ANIMFILE=models\skhorklumps.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skhorklumpsMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skhorklumpsMesh ANIM=skhorklumpsAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skhorklumpsAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skhorklumpsTex0  FILE=TEXTURES\HP2horklump_SKIN00.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skhorklumpsMesh NUM=0 TEXTURE=skhorklumpsTex0

// Original material [0] is [Material #4] SkinIndex: 0 Bitmap: HP2horklump_SKIN00.bmp  Path: C:\potter\Characters\HP2\Horklumps 

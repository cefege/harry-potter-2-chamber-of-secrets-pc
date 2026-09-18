//===============================================================================
//  [skectoplasma] 
//===============================================================================

class skectoplasma extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skectoplasmaMesh MODELFILE=models\skectoplasma.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skectoplasmaMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skectoplasmaAnims ANIMFILE=models\skectoplasma.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skectoplasmaMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skectoplasmaMesh ANIM=skectoplasmaAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skectoplasmaAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skectoplasmaTex0  FILE=TEXTURES\ectoplasm.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skectoplasmaMesh NUM=0 TEXTURE=skectoplasmaTex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: ectoplasm.bmp  Path: C:\hp2_characters\HP2_EctoPlasma 

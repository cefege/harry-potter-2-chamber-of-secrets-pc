class skWand extends HPMesh abstract;


// *** Keeping this here for refrence ***
#EXEC MESH  MODELIMPORT MESH=WandMesh MODELFILE=models\skWand.PSK LODSTYLE=10
#EXEC MESH  ORIGIN MESH=WandMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#EXEC ANIM  IMPORT ANIM=WandAnims ANIMFILE=models\skWand.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#EXEC MESHMAP   SCALE MESHMAP=WandMesh X=1.0 Y=1.0 Z=1.0
#EXEC MESH  DEFAULTANIM MESH=WandMesh ANIM=WandAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#EXEC ANIM DIGEST  ANIM=WandAnims VERBOSE
#EXEC TEXTURE IMPORT NAME=WandTex0  FILE=TEXTURES\WandTexture.bmp  GROUP=Skins
#EXEC MESHMAP SETTEXTURE MESHMAP=WandMesh NUM=0 TEXTURE=WandTex0


//===============================================================================
//  [skBoeing747] 
//===============================================================================

class skBoeing747 extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skBoeing747Mesh MODELFILE=models\skBoeing747.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBoeing747Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBoeing747Anims ANIMFILE=models\skBoeing747.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBoeing747Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBoeing747Mesh ANIM=skBoeing747Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBoeing747Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBoeing747Tex0  FILE=TEXTURES\HP2_747Wings.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skBoeing747Tex1  FILE=TEXTURES\HP2_747Front.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skBoeing747Tex2  FILE=TEXTURES\HP2_747Back.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBoeing747Mesh NUM=0 TEXTURE=skBoeing747Tex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skBoeing747Mesh NUM=1 TEXTURE=skBoeing747Tex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skBoeing747Mesh NUM=2 TEXTURE=skBoeing747Tex2

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2_747Wings.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\747 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2_747Front.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\747 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: HP2_747Back.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\747 

//===============================================================================
//  [QuidGoal] 
//===============================================================================

class skQuidGoal extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=QuidGoalMesh MODELFILE=models\QuidGoal.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=QuidGoalMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=QuidGoalAnims ANIMFILE=models\QuidGoal.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=QuidGoalMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=QuidGoalMesh ANIM=QuidGoalAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=QuidGoalAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=QuidGoalTex0  FILE=TEXTURES\QuidGoal.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=QuidGoalMesh NUM=0 TEXTURE=QuidGoalTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: QuidGoal.bmp  Path: \\Baker\HPotterPC\Art\Texture maps\Kerwin Textures\Flat Textures\Quidditch 

//===============================================================================
//  [skChickenLeg] 
//===============================================================================

class skChickenLeg extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skChickenLegMesh MODELFILE=models\skChickenLeg.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skChickenLegMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skChickenLegAnims ANIMFILE=models\skChickenLeg.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skChickenLegMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skChickenLegMesh ANIM=skChickenLegAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skChickenLegAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skChickenLegTex0  FILE=TEXTURES\Chickenleg_skin00.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skChickenLegMesh NUM=0 TEXTURE=skChickenLegTex0

// Original material [0] is [ChickenLeg_Skin00] SkinIndex: 0 Bitmap: Chickenleg_skin00.bmp  Path: C:\HP2_Objects\chicken_Leg 

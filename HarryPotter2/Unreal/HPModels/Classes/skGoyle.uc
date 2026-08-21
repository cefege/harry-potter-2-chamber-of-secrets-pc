//===============================================================================
//  [skGoyle] 
//===============================================================================

class skGoyle extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skGoyleMesh MODELFILE=models\skGoyle.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skGoyleMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skGoyleMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skGoyleMesh ANIM=skHarryAnims

#EXEC TEXTURE IMPORT NAME=skGoyleTex0  FILE=TEXTURES\HP2GOYLE_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skGoyleTex1  FILE=TEXTURES\HP2GOYLE_SKIN01.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skGoyleMesh NUM=0 TEXTURE=skGoyleTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skGoyleMesh NUM=1 TEXTURE=skGoyleTex1

#exec MESH WEAPONATTACH MESH=skGoyleMesh BONE="RightHand"
#exec MESH WEAPONPOSITION MESH=skGoyleMesh YAW=0 PITCH=0 ROLL=10 X=0.0 Y=0.0 Z=0.0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: HP2GOYLE_SKIN00.bmp  Path: C:\potter\Characters\HP2\Goyle 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: HP2GOYLE_SKIN01.bmp  Path: C:\potter\Characters\HP2\Goyle 

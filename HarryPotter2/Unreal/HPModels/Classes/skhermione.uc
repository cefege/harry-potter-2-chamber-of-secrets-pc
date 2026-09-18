//===============================================================================
//  [skhermione] 
//===============================================================================

class skhermione extends HPMesh abstract;
#exec MESH  MODELIMPORT MESH=skhermioneMesh MODELFILE=models\skhermione.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skhermioneMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec MESHMAP   SCALE MESHMAP=skhermioneMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skhermioneMesh ANIM=skGenFemaleAnims

#EXEC TEXTURE IMPORT NAME=skhermioneTex0  FILE=TEXTURES\HP2HERMIONE_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skhermioneTex1  FILE=TEXTURES\HP2HERMIONE_SKIN01.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skhermioneTex2  FILE=TEXTURES\HP2HERMIONE_SKIN02.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skhermioneMesh NUM=0 TEXTURE=skhermioneTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skhermioneMesh NUM=1 TEXTURE=skhermioneTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skhermioneMesh NUM=2 TEXTURE=skhermioneTex2

#exec MESH WEAPONATTACH MESH=skHermioneMesh BONE="RightHand"
#exec MESH WEAPONPOSITION MESH=skHermioneMesh YAW=0 PITCH=0 ROLL=10 X=0.0 Y=0.0 Z=0.0

// Original material [0] is [HERMIONE_SKIN00] SkinIndex: 0 Bitmap: HP2HERMIONE_SKIN00.bmp  Path: C:\potter\Characters\HP2\Hermione 
// Original material [1] is [HERMIONE_SKIN01.TWOSIDED] SkinIndex: 1 Bitmap: HP2HERMIONE_SKIN01.bmp  Path: C:\potter\Characters\HP2\Hermione 
// Original material [2] is [HERMIONE_SKIN02] SkinIndex: 2 Bitmap: HP2HERMIONE_SKIN02.bmp  Path: C:\potter\Characters\HP2\Hermione 

//===============================================================================
//  [skMushroomLightBlue] 
//===============================================================================

class skMushroomLightBlue extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skMushroomLightBlueMesh MODELFILE=models\skMushroomLightBlue.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skMushroomLightBlueMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skMushroomLightBlueAnims ANIMFILE=models\skMushroomLightBlue.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skMushroomLightBlueMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skMushroomLightBlueMesh ANIM=skMushroomLightBlueAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skMushroomLightBlueAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skMushroomLightBlueTex0  FILE=TEXTURES\LightFungus_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skMushroomLightBlueTex1  FILE=TEXTURES\LightFungusRaysBlue.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skMushroomLightBlueMesh NUM=0 TEXTURE=skMushroomLightBlueTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skMushroomLightBlueMesh NUM=1 TEXTURE=skMushroomLightBlueTex1

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: LightFungus_SKIN00.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\Mushroomlight 
// Original material [1] is [SKIN01.TRANS] SkinIndex: 1 Bitmap: LightFungusRaysBlue.bmp  Path: C:\HarryPotter\Harry Potter 2\Art\Objects\Mushroomlight 


defaultproperties
{
    Mesh=skMushroomLightBlueMesh
    DrawType=DT_Mesh
    bStatic=False
}


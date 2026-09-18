//===============================================================================
//  [SkMushroomLight] 
//===============================================================================

class SkMushroomLight extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=SkMushroomLightMesh MODELFILE=models\SkMushroomLight.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=SkMushroomLightMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=SkMushroomLightAnims ANIMFILE=models\SkMushroomLight.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=SkMushroomLightMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=SkMushroomLightMesh ANIM=SkMushroomLightAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=SkMushroomLightAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=SkMushroomLightTex0  FILE=TEXTURES\LightFungus_SKIN00.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=SkMushroomLightTex1  FILE=TEXTURES\LightFungusRays.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=SkMushroomLightMesh NUM=0 TEXTURE=SkMushroomLightTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=SkMushroomLightMesh NUM=1 TEXTURE=SkMushroomLightTex1

// Original material [0] is [skin00] SkinIndex: 0 Bitmap: LightFungus_SKIN00.bmp  Path: C:\Harry Potter Art\Models\Aragogmushroom 
// Original material [1] is [Skin01.trans] SkinIndex: 1 Bitmap: LightFungusRays.bmp  Path: C:\Harry Potter Art\Models\Aragogmushroom 


defaultproperties
{
    Mesh=SkMushroomLightMesh
    DrawType=DT_Mesh
    bStatic=False
}


//===============================================================================
//  [skBedInfirmary] 
//===============================================================================

class skBedInfirmary extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBedInfirmaryMesh MODELFILE=models\skBedInfirmary.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBedInfirmaryMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBedInfirmaryAnims ANIMFILE=models\skBedInfirmary.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBedInfirmaryMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBedInfirmaryMesh ANIM=skBedInfirmaryAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBedInfirmaryAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBedInfirmaryTex0  FILE=TEXTURES\InfirmBed_128.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skBedInfirmaryTex1  FILE=TEXTURES\InfirmBed_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBedInfirmaryMesh NUM=0 TEXTURE=skBedInfirmaryTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skBedInfirmaryMesh NUM=1 TEXTURE=skBedInfirmaryTex1

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: InfirmBed_128.bmp  Path: C:\Harry Potter 2\ART\Objects\Beds\Infirmary Bed 
// Original material [1] is [SKIN01] SkinIndex: 1 Bitmap: InfirmBed_128.bmp  Path: C:\Harry Potter 2\ART\Objects\Beds\Infirmary Bed 


defaultproperties
{
    Mesh=skBedInfirmaryMesh
    DrawType=DT_Mesh
    bStatic=False
}


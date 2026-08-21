//===============================================================================
//  [skCabinetInfirmary] 
//===============================================================================

class skCabinetInfirmary extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skCabinetInfirmaryMesh MODELFILE=models\skCabinetInfirmary.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skCabinetInfirmaryMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skCabinetInfirmaryAnims ANIMFILE=models\skCabinetInfirmary.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skCabinetInfirmaryMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skCabinetInfirmaryMesh ANIM=skCabinetInfirmaryAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skCabinetInfirmaryAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skCabinetInfirmaryTex0  FILE=TEXTURES\InfirmCab_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skCabinetInfirmaryMesh NUM=0 TEXTURE=skCabinetInfirmaryTex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: InfirmCab_128.bmp  Path: C:\Harry Potter 2\ART\Objects\Cabinets_Hutches_Bookcases\Infirmary Cabinet 


defaultproperties
{
    Mesh=skCabinetInfirmaryMesh
    DrawType=DT_Mesh
    bStatic=False
}


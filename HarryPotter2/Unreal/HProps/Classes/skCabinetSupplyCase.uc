//===============================================================================
//  [skCabinetSupplyCase] 
//===============================================================================

class skCabinetSupplyCase extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skCabinetSupplyCaseMesh MODELFILE=models\skCabinetSupplyCase.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skCabinetSupplyCaseMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skCabinetSupplyCaseAnims ANIMFILE=models\skCabinetSupplyCase.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skCabinetSupplyCaseMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skCabinetSupplyCaseMesh ANIM=skCabinetSupplyCaseAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skCabinetSupplyCaseAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skCabinetSupplyCaseTex0  FILE=TEXTURES\suplcase_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skCabinetSupplyCaseMesh NUM=0 TEXTURE=skCabinetSupplyCaseTex0

// Original material [0] is [Material #3] SkinIndex: 0 Bitmap: suplcase_128.bmp  Path: C:\Harry Potter\ART\Objects\Cabinets_Hutches_Bookcases\Supply Cabinet 


defaultproperties
{
    Mesh=skCabinetSupplyCaseMesh
    DrawType=DT_Mesh
    bStatic=False
}


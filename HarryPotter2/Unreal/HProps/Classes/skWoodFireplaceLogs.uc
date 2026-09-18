//===============================================================================
//  [skWoodFireplaceLogs] 
//===============================================================================

class skWoodFireplaceLogs extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skWoodFireplaceLogsMesh MODELFILE=models\skWoodFireplaceLogs.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skWoodFireplaceLogsMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skWoodFireplaceLogsAnims ANIMFILE=models\skWoodFireplaceLogs.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skWoodFireplaceLogsMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skWoodFireplaceLogsMesh ANIM=skWoodFireplaceLogsAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skWoodFireplaceLogsAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skWoodFireplaceLogsTex0  FILE=TEXTURES\firewood.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skWoodFireplaceLogsMesh NUM=0 TEXTURE=skWoodFireplaceLogsTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: firewood.bmp  Path: C:\Harry Potter\ART\Objects\Logs_Firewood_Wood\Single Firewood Log 


defaultproperties
{
    Mesh=skWoodFireplaceLogsMesh
    DrawType=DT_Mesh
    bStatic=False
}


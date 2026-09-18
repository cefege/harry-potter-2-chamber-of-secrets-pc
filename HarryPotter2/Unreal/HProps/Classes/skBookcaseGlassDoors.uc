//===============================================================================
//  [skBookcaseGlassDoors] 
//===============================================================================

class skBookcaseGlassDoors extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBookcaseGlassDoorsMesh MODELFILE=models\skBookcaseGlassDoors.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBookcaseGlassDoorsMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBookcaseGlassDoorsAnims ANIMFILE=models\skBookcaseGlassDoors.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBookcaseGlassDoorsMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBookcaseGlassDoorsMesh ANIM=skBookcaseGlassDoorsAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBookcaseGlassDoorsAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBookcaseGlassDoorsTex0  FILE=TEXTURES\TransBookcase_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBookcaseGlassDoorsMesh NUM=0 TEXTURE=skBookcaseGlassDoorsTex0

// Original material [0] is [Material #3] SkinIndex: 0 Bitmap: TransBookcase_128.bmp  Path: C:\Harry Potter\ART\Objects\Cabinets_Hutches_Bookcases\Wide Bookcase 


defaultproperties
{
    Mesh=skBookcaseGlassDoorsMesh
    DrawType=DT_Mesh
    bStatic=False
}


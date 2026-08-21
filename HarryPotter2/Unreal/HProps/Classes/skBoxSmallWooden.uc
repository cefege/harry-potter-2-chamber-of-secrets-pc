//===============================================================================
//  [skBoxSmallWooden] 
//===============================================================================

class skBoxSmallWooden extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBoxSmallWoodenMesh MODELFILE=models\skBoxSmallWooden.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBoxSmallWoodenMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBoxSmallWoodenAnims ANIMFILE=models\skBoxSmallWooden.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBoxSmallWoodenMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBoxSmallWoodenMesh ANIM=skBoxSmallWoodenAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBoxSmallWoodenAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBoxSmallWoodenTex0  FILE=TEXTURES\woodbox1_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBoxSmallWoodenMesh NUM=0 TEXTURE=skBoxSmallWoodenTex0

// Original material [0] is [Material #25] SkinIndex: 0 Bitmap: woodbox1_128.bmp  Path: C:\Harry Potter\ART\Objects\Chests_Boxes_Trunks\Pointy Wood Box 


defaultproperties
{
    Mesh=skBoxSmallWoodenMesh
    DrawType=DT_Mesh
    bStatic=False
}


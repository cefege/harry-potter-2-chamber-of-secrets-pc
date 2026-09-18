//===============================================================================
//  [skTableTrestle] 
//===============================================================================

class skTableTrestle extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skTableTrestleMesh MODELFILE=models\skTableTrestle.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skTableTrestleMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skTableTrestleAnims ANIMFILE=models\skTableTrestle.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skTableTrestleMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skTableTrestleMesh ANIM=skTableTrestleAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skTableTrestleAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skTableTrestleTex0  FILE=TEXTURES\SideTable_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skTableTrestleMesh NUM=0 TEXTURE=skTableTrestleTex0

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: SideTable_128.bmp  Path: C:\Harry Potter\ART\Objects\Tables_Desks\Side table 


defaultproperties
{
    Mesh=skTableTrestleMesh
    DrawType=DT_Mesh
    bStatic=False
}


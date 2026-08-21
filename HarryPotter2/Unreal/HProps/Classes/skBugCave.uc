//===============================================================================
//  [skBugCave] 
//===============================================================================

class skBugCave extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBugCaveMesh MODELFILE=models\skBugCave.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBugCaveMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBugCaveAnims ANIMFILE=models\skBugCave.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBugCaveMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBugCaveMesh ANIM=skBugCaveAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBugCaveAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBugCaveTex0  FILE=TEXTURES\CaveBug.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBugCaveMesh NUM=0 TEXTURE=skBugCaveTex0

// Original material [0] is [skin00.MASKED] SkinIndex: 0 Bitmap: CaveBug.bmp  Path: C:\Harry Potter\ART\Objects\Bugs\Green Bug 


defaultproperties
{
    Mesh=skBugCaveMesh
    DrawType=DT_Mesh
    bStatic=False
}


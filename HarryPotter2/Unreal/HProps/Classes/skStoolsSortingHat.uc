//===============================================================================
//  [skStoolsSortingHat] 
//===============================================================================

class skStoolsSortingHat extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skStoolsSortingHatMesh MODELFILE=models\skStoolsSortingHat.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skStoolsSortingHatMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skStoolsSortingHatAnims ANIMFILE=models\skStoolsSortingHat.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skStoolsSortingHatMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skStoolsSortingHatMesh ANIM=skStoolsSortingHatAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skStoolsSortingHatAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skStoolsSortingHatTex0  FILE=TEXTURES\sortstol_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skStoolsSortingHatMesh NUM=0 TEXTURE=skStoolsSortingHatTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: sortstol_128.bmp  Path: C:\Harry Potter\ART\Objects\Chairs_Stools_Sofas\Sorting Stool 


defaultproperties
{
    Mesh=skStoolsSortingHatMesh
    DrawType=DT_Mesh
    bStatic=False
}


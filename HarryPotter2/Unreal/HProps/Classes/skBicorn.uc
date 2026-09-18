//===============================================================================
//  [skBicorn] 
//===============================================================================

class skBicorn extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skBicornMesh MODELFILE=models\skBicorn.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skBicornMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skBicornAnims ANIMFILE=models\skBicorn.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skBicornMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skBicornMesh ANIM=skBicornAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skBicornAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skBicornTex0  FILE=TEXTURES\BiHorn.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skBicornMesh NUM=0 TEXTURE=skBicornTex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: BiHorn.bmp  Path: C:\Harry Potter 2\ART\Objects\Spell Ingredients\Bicorn Horn 


defaultproperties
{
    Mesh=skBicornMesh
    DrawType=DT_Mesh
    bStatic=False
}


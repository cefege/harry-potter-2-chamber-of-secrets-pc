//===============================================================================
//  [skDishesPeevesSpoon] 
//===============================================================================

class skDishesPeevesSpoon extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skDishesPeevesSpoonMesh MODELFILE=models\skDishesPeevesSpoon.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skDishesPeevesSpoonMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skDishesPeevesSpoonAnims ANIMFILE=models\skDishesPeevesSpoon.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skDishesPeevesSpoonMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skDishesPeevesSpoonMesh ANIM=skDishesPeevesSpoonAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skDishesPeevesSpoonAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skDishesPeevesSpoonTex0  FILE=TEXTURES\pvespoon_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skDishesPeevesSpoonMesh NUM=0 TEXTURE=skDishesPeevesSpoonTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: pvespoon_128.bmp  Path: C:\Harry Potter\ART\Objects\Dishes_Cups_Pots_Utensiles\Peeves Spoon 


defaultproperties
{
    Mesh=skDishesPeevesSpoonMesh
    DrawType=DT_Mesh
    bStatic=False
}


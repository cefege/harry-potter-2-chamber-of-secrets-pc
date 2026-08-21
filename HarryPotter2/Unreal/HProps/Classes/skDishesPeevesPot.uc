//===============================================================================
//  [skDishesPeevesPot] 
//===============================================================================

class skDishesPeevesPot extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skDishesPeevesPotMesh MODELFILE=models\skDishesPeevesPot.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skDishesPeevesPotMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skDishesPeevesPotAnims ANIMFILE=models\skDishesPeevesPot.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skDishesPeevesPotMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skDishesPeevesPotMesh ANIM=skDishesPeevesPotAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skDishesPeevesPotAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skDishesPeevesPotTex0  FILE=TEXTURES\peevepot_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skDishesPeevesPotMesh NUM=0 TEXTURE=skDishesPeevesPotTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: peevepot_128.bmp  Path: C:\Harry Potter\ART\Objects\Dishes_Cups_Pots_Utensiles\Peeves Pot 


defaultproperties
{
    Mesh=skDishesPeevesPotMesh
    DrawType=DT_Mesh
    bStatic=False
}


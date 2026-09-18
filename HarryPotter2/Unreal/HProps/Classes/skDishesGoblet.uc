//===============================================================================
//  [skDishesGoblet] 
//===============================================================================

class skDishesGoblet extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skDishesGobletMesh MODELFILE=models\skDishesGoblet.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skDishesGobletMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skDishesGobletAnims ANIMFILE=models\skDishesGoblet.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skDishesGobletMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skDishesGobletMesh ANIM=skDishesGobletAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skDishesGobletAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skDishesGobletTex0  FILE=TEXTURES\hoggoblet_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skDishesGobletMesh NUM=0 TEXTURE=skDishesGobletTex0

// Original material [0] is [Material #9] SkinIndex: 0 Bitmap: hoggoblet_128.bmp  Path: C:\Harry Potter\ART\Objects\Dishes_Cups_Pots_Utensiles\Goblet 


defaultproperties
{
    Mesh=skDishesGobletMesh
    DrawType=DT_Mesh
    bStatic=False
}


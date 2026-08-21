//===============================================================================
//  [skDiffindoWeb1] 
//===============================================================================

class skDiffindoWeb1 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skDiffindoWeb1Mesh MODELFILE=models\skDiffindoWeb1.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skDiffindoWeb1Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skDiffindoWeb1Anims ANIMFILE=models\skDiffindoWeb1.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skDiffindoWeb1Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skDiffindoWeb1Mesh ANIM=skDiffindoWeb1Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skDiffindoWeb1Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skDiffindoWeb1Tex0  FILE=TEXTURES\SpiderWeb1.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skDiffindoWeb1Mesh NUM=0 TEXTURE=skDiffindoWeb1Tex0

// Original material [0] is [SKIN00.TRANSLUCENT] SkinIndex: 0 Bitmap: SpiderWeb1.bmp  Path: C:\Harry Potter 2\ART\Objects\Diffindo Items\Spider Webs 


defaultproperties
{
    Mesh=skDiffindoWeb1Mesh
    DrawType=DT_Mesh
    bStatic=False
}


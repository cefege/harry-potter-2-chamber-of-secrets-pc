//===============================================================================
//  [skDiffindoWeb2] 
//===============================================================================

class skDiffindoWeb2 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skDiffindoWeb2Mesh MODELFILE=models\skDiffindoWeb2.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skDiffindoWeb2Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skDiffindoWeb2Anims ANIMFILE=models\skDiffindoWeb2.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skDiffindoWeb2Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skDiffindoWeb2Mesh ANIM=skDiffindoWeb2Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skDiffindoWeb2Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skDiffindoWeb2Tex0  FILE=TEXTURES\SpiderWeb2.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skDiffindoWeb2Mesh NUM=0 TEXTURE=skDiffindoWeb2Tex0

// Original material [0] is [SKIN00.TRANSLUCENT] SkinIndex: 0 Bitmap: SpiderWeb2.bmp  Path: C:\Harry Potter 2\ART\Objects\Diffindo Items\Spider Webs 


defaultproperties
{
    Mesh=skDiffindoWeb2Mesh
    DrawType=DT_Mesh
    bStatic=False
}


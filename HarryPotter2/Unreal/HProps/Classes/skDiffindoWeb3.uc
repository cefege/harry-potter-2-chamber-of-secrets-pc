//===============================================================================
//  [skDiffindoWeb3] 
//===============================================================================

class skDiffindoWeb3 extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skDiffindoWeb3Mesh MODELFILE=models\skDiffindoWeb3.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skDiffindoWeb3Mesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skDiffindoWeb3Anims ANIMFILE=models\skDiffindoWeb3.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skDiffindoWeb3Mesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skDiffindoWeb3Mesh ANIM=skDiffindoWeb3Anims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skDiffindoWeb3Anims VERBOSE

#EXEC TEXTURE IMPORT NAME=skDiffindoWeb3Tex0  FILE=TEXTURES\SpiderWeb3.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skDiffindoWeb3Mesh NUM=0 TEXTURE=skDiffindoWeb3Tex0

// Original material [0] is [SKIN00.TRANSLUCENT] SkinIndex: 0 Bitmap: SpiderWeb3.bmp  Path: C:\Harry Potter 2\ART\Objects\Diffindo Items\Spider Webs 


defaultproperties
{
    Mesh=skDiffindoWeb3Mesh
    DrawType=DT_Mesh
    bStatic=False
}


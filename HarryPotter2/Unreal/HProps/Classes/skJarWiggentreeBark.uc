//===============================================================================
//  [skJarWiggentreeBark] 
//===============================================================================

class skJarWiggentreeBark extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skJarWiggentreeBarkMesh MODELFILE=models\skJarWiggentreeBark.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skJarWiggentreeBarkMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skJarWiggentreeBarkAnims ANIMFILE=models\skJarWiggentreeBark.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skJarWiggentreeBarkMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skJarWiggentreeBarkMesh ANIM=skJarWiggentreeBarkAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skJarWiggentreeBarkAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skJarWiggentreeBarkTex0  FILE=TEXTURES\WiggentreeBark.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skJarWiggentreeBarkTex1  FILE=TEXTURES\WiggenJar.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=skJarWiggentreeBarkTex2  FILE=TEXTURES\WiggenJar.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skJarWiggentreeBarkMesh NUM=0 TEXTURE=skJarWiggentreeBarkTex0
#EXEC MESHMAP SETTEXTURE MESHMAP=skJarWiggentreeBarkMesh NUM=1 TEXTURE=skJarWiggentreeBarkTex1
#EXEC MESHMAP SETTEXTURE MESHMAP=skJarWiggentreeBarkMesh NUM=2 TEXTURE=skJarWiggentreeBarkTex2

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: WiggentreeBark.bmp  Path: C:\Harry Potter 2\ART\Objects\Spell Ingredients\Wiggentree Bark 
// Original material [1] is [SKIN01.TRANSLUCENT] SkinIndex: 1 Bitmap: WiggenJar.bmp  Path: C:\Harry Potter 2\ART\Objects\Spell Ingredients\Wiggentree Bark 
// Original material [2] is [SKIN02] SkinIndex: 2 Bitmap: WiggenJar.bmp  Path: C:\Harry Potter 2\ART\Objects\Spell Ingredients\Wiggentree Bark 


defaultproperties
{
    Mesh=skJarWiggentreeBarkMesh
    DrawType=DT_Mesh
    bStatic=False
}


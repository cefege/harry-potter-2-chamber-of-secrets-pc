//===============================================================================
//  [skWiggentreeBark] 
//===============================================================================

class skWiggentreeBark extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skWiggentreeBarkMesh MODELFILE=models\skWiggentreeBark.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skWiggentreeBarkMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skWiggentreeBarkAnims ANIMFILE=models\skWiggentreeBark.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skWiggentreeBarkMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skWiggentreeBarkMesh ANIM=skWiggentreeBarkAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skWiggentreeBarkAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skWiggentreeBarkTex0  FILE=TEXTURES\WiggentreeBark.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skWiggentreeBarkMesh NUM=0 TEXTURE=skWiggentreeBarkTex0

// Original material [0] is [Material #2] SkinIndex: 0 Bitmap: WiggentreeBark.bmp  Path: C:\Harry Potter 2\ART\Objects\Spell Ingredients\Wiggentree Bark 


defaultproperties
{
    Mesh=skWiggentreeBarkMesh
    DrawType=DT_Mesh
    bStatic=False
}


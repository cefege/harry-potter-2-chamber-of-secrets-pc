//===============================================================================
//  [skArmorWholeSuit] 
//===============================================================================

class skArmorWholeSuit extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skArmorWholeSuitMesh MODELFILE=models\skArmorWholeSuit.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skArmorWholeSuitMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skArmorWholeSuitAnims ANIMFILE=models\skArmorWholeSuit.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skArmorWholeSuitMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skArmorWholeSuitMesh ANIM=skArmorWholeSuitAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skArmorWholeSuitAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skArmorWholeSuitTex0  FILE=TEXTURES\knight.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skArmorWholeSuitMesh NUM=0 TEXTURE=skArmorWholeSuitTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: knight.bmp  Path: C:\Harry Potter\ART\Objects\Armor\Suit of Armor 


defaultproperties
{
    Mesh=skArmorWholeSuitMesh
    DrawType=DT_Mesh
    bStatic=False
}


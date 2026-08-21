//===============================================================================
//  [skArmorWholeSuitSepia] 
//===============================================================================

class skArmorWholeSuitSepia extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skArmorWholeSuitSepiaMesh MODELFILE=models\skArmorWholeSuitSepia.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skArmorWholeSuitSepiaMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skArmorWholeSuitSepiaAnims ANIMFILE=models\skArmorWholeSuitSepia.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skArmorWholeSuitSepiaMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skArmorWholeSuitSepiaMesh ANIM=skArmorWholeSuitSepiaAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skArmorWholeSuitSepiaAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skArmorWholeSuitSepiaTex0  FILE=TEXTURES\knightSepia.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skArmorWholeSuitSepiaMesh NUM=0 TEXTURE=skArmorWholeSuitSepiaTex0

// Original material [0] is [SKIN00] SkinIndex: 0 Bitmap: knightSepia.bmp  Path: C:\Harry Potter 2\ART\Objects\Armor\Suit of Armor 


defaultproperties
{
    Mesh=skArmorWholeSuitSepiaMesh
    DrawType=DT_Mesh
    bStatic=False
}


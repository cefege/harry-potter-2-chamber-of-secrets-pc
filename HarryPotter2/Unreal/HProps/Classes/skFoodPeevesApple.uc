//===============================================================================
//  [skFoodPeevesApple] 
//===============================================================================

class skFoodPeevesApple extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skFoodPeevesAppleMesh MODELFILE=models\skFoodPeevesApple.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skFoodPeevesAppleMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skFoodPeevesAppleAnims ANIMFILE=models\skFoodPeevesApple.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skFoodPeevesAppleMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skFoodPeevesAppleMesh ANIM=skFoodPeevesAppleAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skFoodPeevesAppleAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skFoodPeevesAppleTex0  FILE=TEXTURES\pveapple_64.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skFoodPeevesAppleMesh NUM=0 TEXTURE=skFoodPeevesAppleTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: pveapple_64.bmp  Path: C:\Harry Potter\ART\Objects\Food_Candy\Peeves Apple 


defaultproperties
{
    Mesh=skFoodPeevesAppleMesh
    DrawType=DT_Mesh
    bStatic=False
}


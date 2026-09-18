//===============================================================================
//  [skChristmasOrnamentBird] 
//===============================================================================

class skChristmasOrnamentBird extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skChristmasOrnamentBirdMesh MODELFILE=models\skChristmasOrnamentBird.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skChristmasOrnamentBirdMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skChristmasOrnamentBirdAnims ANIMFILE=models\skChristmasOrnamentBird.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skChristmasOrnamentBirdMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skChristmasOrnamentBirdMesh ANIM=skChristmasOrnamentBirdAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skChristmasOrnamentBirdAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skChristmasOrnamentBirdTex0  FILE=TEXTURES\xmasbird_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skChristmasOrnamentBirdMesh NUM=0 TEXTURE=skChristmasOrnamentBirdTex0

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: xmasbird_128.bmp  Path: C:\Harry Potter\ART\Objects\Christmas Decorations\Ornaments\Bird 


defaultproperties
{
    Mesh=skChristmasOrnamentBirdMesh
    DrawType=DT_Mesh
    bStatic=False
}


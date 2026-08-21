//===============================================================================
//  [skChristmasOrnamentBall] 
//===============================================================================

class skChristmasOrnamentBall extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skChristmasOrnamentBallMesh MODELFILE=models\skChristmasOrnamentBall.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skChristmasOrnamentBallMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skChristmasOrnamentBallAnims ANIMFILE=models\skChristmasOrnamentBall.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skChristmasOrnamentBallMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skChristmasOrnamentBallMesh ANIM=skChristmasOrnamentBallAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skChristmasOrnamentBallAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skChristmasOrnamentBallTex0  FILE=TEXTURES\xmasball_64.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skChristmasOrnamentBallMesh NUM=0 TEXTURE=skChristmasOrnamentBallTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: xmasball_64.bmp  Path: C:\Harry Potter\ART\Objects\Christmas Decorations\Ornaments\Ball 


defaultproperties
{
    Mesh=skChristmasOrnamentBallMesh
    DrawType=DT_Mesh
    bStatic=False
}


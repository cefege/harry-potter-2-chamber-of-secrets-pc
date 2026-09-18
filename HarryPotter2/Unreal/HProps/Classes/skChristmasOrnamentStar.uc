//===============================================================================
//  [skChristmasOrnamentStar] 
//===============================================================================

class skChristmasOrnamentStar extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skChristmasOrnamentStarMesh MODELFILE=models\skChristmasOrnamentStar.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skChristmasOrnamentStarMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skChristmasOrnamentStarAnims ANIMFILE=models\skChristmasOrnamentStar.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skChristmasOrnamentStarMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skChristmasOrnamentStarMesh ANIM=skChristmasOrnamentStarAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skChristmasOrnamentStarAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skChristmasOrnamentStarTex0  FILE=TEXTURES\christar_64.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skChristmasOrnamentStarMesh NUM=0 TEXTURE=skChristmasOrnamentStarTex0

// Original material [0] is [Material #1] SkinIndex: 0 Bitmap: christar_64.bmp  Path: C:\Harry Potter\ART\Objects\Christmas Decorations\Ornaments\Star 


defaultproperties
{
    Mesh=skChristmasOrnamentStarMesh
    DrawType=DT_Mesh
    bStatic=False
}


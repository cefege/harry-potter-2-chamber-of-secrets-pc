//===============================================================================
//  [skCandlesWallTripleCandle] 
//===============================================================================

class skCandlesWallTripleCandle extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skCandlesWallTripleCandleMesh MODELFILE=models\skCandlesWallTripleCandle.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skCandlesWallTripleCandleMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skCandlesWallTripleCandleAnims ANIMFILE=models\skCandlesWallTripleCandle.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skCandlesWallTripleCandleMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skCandlesWallTripleCandleMesh ANIM=skCandlesWallTripleCandleAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skCandlesWallTripleCandleAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skCandlesWallTripleCandleTex0  FILE=TEXTURES\wallcndl_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skCandlesWallTripleCandleMesh NUM=0 TEXTURE=skCandlesWallTripleCandleTex0

// Original material [0] is [SKIN00.MASKED] SkinIndex: 0 Bitmap: wallcndl_128.bmp  Path: C:\Harry Potter 2\ART\Objects\Candles_n_Candle_Sticks\Wall Tripple Candlestick 


defaultproperties
{
    Mesh=skCandlesWallTripleCandleMesh
    DrawType=DT_Mesh
    bStatic=False
}


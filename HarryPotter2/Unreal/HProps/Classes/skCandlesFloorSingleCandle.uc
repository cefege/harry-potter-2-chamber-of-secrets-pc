//===============================================================================
//  [skCandlesFloorSingleCandle] 
//===============================================================================

class skCandlesFloorSingleCandle extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skCandlesFloorSingleCandleMesh MODELFILE=models\skCandlesFloorSingleCandle.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skCandlesFloorSingleCandleMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skCandlesFloorSingleCandleAnims ANIMFILE=models\skCandlesFloorSingleCandle.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skCandlesFloorSingleCandleMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skCandlesFloorSingleCandleMesh ANIM=skCandlesFloorSingleCandleAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skCandlesFloorSingleCandleAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skCandlesFloorSingleCandleTex0  FILE=TEXTURES\FlorCndl_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skCandlesFloorSingleCandleMesh NUM=0 TEXTURE=skCandlesFloorSingleCandleTex0

// Original material [0] is [Material #3] SkinIndex: 0 Bitmap: FlorCndl_128.bmp  Path: C:\Harry Potter\ART\Objects\Candles_n_Candle_Sticks\Floor Single Candlestick 


defaultproperties
{
    Mesh=skCandlesFloorSingleCandleMesh
    DrawType=DT_Mesh
    bStatic=False
}


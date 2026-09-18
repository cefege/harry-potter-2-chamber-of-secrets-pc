//===============================================================================
//  [skCandlesFloorTrippleCandle] 
//===============================================================================

class skCandlesFloorTrippleCandle extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skCandlesFloorTrippleCandleMesh MODELFILE=models\skCandlesFloorTrippleCandle.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skCandlesFloorTrippleCandleMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skCandlesFloorTrippleCandleAnims ANIMFILE=models\skCandlesFloorTrippleCandle.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skCandlesFloorTrippleCandleMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skCandlesFloorTrippleCandleMesh ANIM=skCandlesFloorTrippleCandleAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skCandlesFloorTrippleCandleAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skCandlesFloorTrippleCandleTex0  FILE=TEXTURES\FlorCndl_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skCandlesFloorTrippleCandleMesh NUM=0 TEXTURE=skCandlesFloorTrippleCandleTex0

// Original material [0] is [Material #3] SkinIndex: 0 Bitmap: FlorCndl_128.bmp  Path: C:\Harry Potter\ART\Objects\Candles_n_Candle_Sticks\Floor Tripple Candlestick 


defaultproperties
{
    Mesh=skCandlesFloorTrippleCandleMesh
    DrawType=DT_Mesh
    bStatic=False
}


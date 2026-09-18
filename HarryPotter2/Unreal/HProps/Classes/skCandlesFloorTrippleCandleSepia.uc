//===============================================================================
//  [skCandlesFloorTrippleCandleSepia] 
//===============================================================================

class skCandlesFloorTrippleCandleSepia extends HPMeshActor;
#exec MESH  MODELIMPORT MESH=skCandlesFloorTrippleCandleSepiaMesh MODELFILE=models\skCandlesFloorTrippleCandleSepia.PSK LODSTYLE=10
#exec MESH  ORIGIN MESH=skCandlesFloorTrippleCandleSepiaMesh X=0 Y=0 Z=0 YAW=0 PITCH=0 ROLL=0
#exec ANIM  IMPORT ANIM=skCandlesFloorTrippleCandleSepiaAnims ANIMFILE=models\skCandlesFloorTrippleCandleSepia.PSA COMPRESS=1 MAXKEYS=999999 IMPORTSEQS=1
#exec MESHMAP   SCALE MESHMAP=skCandlesFloorTrippleCandleSepiaMesh X=1.0 Y=1.0 Z=1.0
#exec MESH  DEFAULTANIM MESH=skCandlesFloorTrippleCandleSepiaMesh ANIM=skCandlesFloorTrippleCandleSepiaAnims

// Digest and compress the animation data. Must come after the sequence declarations.
// 'VERBOSE' gives more debugging info in UCC.log 
#exec ANIM DIGEST  ANIM=skCandlesFloorTrippleCandleSepiaAnims VERBOSE

#EXEC TEXTURE IMPORT NAME=skCandlesFloorTrippleCandleSepiaTex0  FILE=TEXTURES\FlorCndlSepia_128.bmp  GROUP=Skins

#EXEC MESHMAP SETTEXTURE MESHMAP=skCandlesFloorTrippleCandleSepiaMesh NUM=0 TEXTURE=skCandlesFloorTrippleCandleSepiaTex0

// Original material [0] is [Material #3] SkinIndex: 0 Bitmap: FlorCndlSepia_128.bmp  Path: C:\Harry Potter 2\ART\Objects\Candles_n_Candle_Sticks\Floor Single Candlestick 


defaultproperties
{
    Mesh=skCandlesFloorTrippleCandleSepiaMesh
    DrawType=DT_Mesh
    bStatic=False
}


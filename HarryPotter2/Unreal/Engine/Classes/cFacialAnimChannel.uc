
class cFacialAnimChannel expands AnimChannel;

//var bool _bBackToNormalWhenDone;
var name DefaultFaceAnim;

//*************************************************************************************************
function PlayFacialAnim(           name  AnimName
                        , optional bool  bDefaultAnim
						, optional float TweenTime
						, optional float HoldTime
					   )
{
	//level.PlayerHarryActor.ClientMessage("pfa:AnimName:"$AnimName);

	if( AnimName == '' )
		AnimName = DefaultFaceAnim;

	if( TweenTime == 0 )
		TweenTime = 1.0;

	PlayAnim( AnimName, 1.0, TweenTime );

	if( HoldTime > 0  &&  !bDefaultAnim)
		SetTimer( HoldTime, false );

	if( bDefaultAnim )
		DefaultFaceAnim = AnimName;
}

//*************************************************************************************************
function Timer()
{
	//level.PlayerHarryActor.ClientMessage("pfa: timer done");
	PlayFacialAnim( DefaultFaceAnim );
}

//*************************************************************************************************
defaultproperties
{
	DefaultFaceAnim="brow_rest"
}

class TriggerChangeActorMesh extends trigger;

var() Mesh       NewMesh;
var() Animation  NewSkelAnim;
var() name       AnimToPlay;

//*******************************************************************************
event Trigger( Actor Other, Pawn EventInstigator )
{
	ProcessTrigger();
}

function touch(actor other)
{
	super.touch(other);

	ProcessTrigger();
}

//*******************************************************************************
function ProcessTrigger()
{
	local actor   a;

	foreach AllActors( class'actor', a, event )
		break;

	if( a == none )
	{
		Log("TriggerChangeActorMesh: Couldn't find actor to change mesh");
		return;
	}

	if( NewMesh != none )
		a.Mesh =     NewMesh;

	if( NewSkelAnim != none )
		a.SkelAnim = NewSkelAnim;

	if( NewMesh != none  &&  AnimToPlay != '')
		LoopAnim( AnimToPlay );

	//a.Mesh =     SkeletalMesh'skDevilHarryMesh';
	//a.SkelAnim = Animation'skDevilHarryAnims';
}


//*****************************************************************************
defaultproperties
{

}
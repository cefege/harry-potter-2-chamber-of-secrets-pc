class ParticleFadeFX expands ParticleFX;

var bool bTurningOn;

//*************************************************************************************************************************
event Trigger( Actor Other, Pawn EventInstigator )
{
	//bTurningOn = !bTurningOn;

	if( ParentBlend == 0 )
		ParentBlend = 1;
	else
		ParentBlend = 0;

	//Make sure it's not hidden
	bHidden = false;
}

//*************************************************************************************************************************
//event Tick( float Delta )
//{
//
//}

defaultproperties
{
	ParentBlend=1
}
class Despawner expands HPawn;

var() name EventName;

//***************************************************************
function touch( actor other )
{
	Super.Touch(other);

	if(!HPawn(other).bDespawnable)
		return;

	if( Harry(other) == playerHarry)
		return;

	// if HPawn was not despawned yet, send an event
	// it prevents to send event a few times
	if((EventName != 'none') && !HPawn(other).bDespawned )
	{
		HPawn(other).TriggerEvent(EventName, none, none );
		playerHarry.clientMessage("Send event.................." $EventName);
	}

	HPawn(other).SetDespawnFlag();
}

//***************************************************************
defaultproperties
{
	// collision
	CollideType=CT_BOX
	CollisionRadius=30
	CollisionWidth=30
	CollisionHeight=30

	// visial
	DrawType=DT_SPRITE
	Texture=S_Keypoint

	bBlockActors=false
	bBlockPlayers=false
	bHidden=true
}

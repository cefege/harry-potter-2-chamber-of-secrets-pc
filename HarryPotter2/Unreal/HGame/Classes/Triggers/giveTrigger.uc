class giveTrigger extends trigger;
/*
var(trigger)    Class<HPawn>  giveItemClass;

function Touch( actor Other )
{
local actor A;
local baseHarry h;

	if( IsRelevant( Other ) )
	{
		if ( ReTriggerDelay > 0 )
		{
			if ( Level.TimeSeconds - TriggerTime < ReTriggerDelay )
				return;
			TriggerTime = Level.TimeSeconds;
		}
		// Broadcast the Trigger message to all matching actors.

		if(giveItemClass==None)
			return;

		h=baseHarry(other);
		if(h==None)
			return;
	

		if(h.quickInventory!=None)
			{
			if(h.quickInventory.IsA(giveItemClass.name))
				return;	//already have one.
			h.quickInventory.destroy();
			}

		h.clientMessage("Setting quickInventory to:" $giveItemClass);
		h.quickInventory=spawn(giveItemClass);
		h.gotostate('potionPickup');
		h.quickInventory.bHidden=true;
	
		if ( Other.IsA('Pawn') && (Pawn(Other).SpecialGoal == self) )
			Pawn(Other).SpecialGoal = None;
				
		if( Message != "" )
			// Send a string message to the toucher.
			Other.Instigator.ClientMessage( Message );

		if( bTriggerOnceOnly )
			// Ignore future touches.
			SetCollision(False);
		else if ( RepeatTriggerTime > 0 )
			SetTimer(RepeatTriggerTime, false);
	}
}
*/
//defaultproperties
//{
//	TriggerType=TT_PlayerProximity
//}

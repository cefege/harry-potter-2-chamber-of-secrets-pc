//===============================================================================
//  Bicorn polyjuice potion ingredient 
//===============================================================================

class Bicorn extends PotionIngredients;

state PickupProp
{
	function EndState()
	{		
		// Send out a trigger on pickup.
		TriggerEvent( Event, none, none );

		Super.EndState();
	}
}

defaultproperties
{
     soundPickup=Sound'HPSounds.Magic_sfx.pickup11'
     soundDropOff=Sound'HPSounds.menu_sfx.add_potion_ingredient'
     bPickupOnTouch=True
     PickupFlyTo=FT_HudPosition
     classStatusGroup=Class'HGame.StatusGroupPolyIngr'
	 classStatusItem=Class'HGame.StatusItemBicorn'
     //Rotation=(Pitch=16131)
     Mesh=SkeletalMesh'HProps.skBicornMesh'
     CollisionRadius=14
     CollisionHeight=30
     bBlockActors=False
     bBlockPlayers=False
	 physics=phys_walking
}

//===============================================================================
// 
//===============================================================================

class Bronzecards extends Wizardcardicon abstract;

defaultproperties
{
    bPickupOnTouch=true
	PickupFlyTo=FT_HudPosition
	soundPickup=sound'HPSounds.magic_sfx.pickup_WC_bronze'

	// Status manager
	classStatusGroup=Class'HGame.StatusGroupWizardCards'
	classStatusItem=Class'HGame.StatusItemBronzeCards'
}


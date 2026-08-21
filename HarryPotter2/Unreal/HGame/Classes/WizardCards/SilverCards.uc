//===============================================================================
// 
//===============================================================================

class Silvercards extends Wizardcardicon abstract;

defaultproperties
{
    bPickupOnTouch=true
	PickupFlyTo=FT_HudPosition
	soundPickup=sound'HPSounds.magic_sfx.pickup_WC_silver'

	// Status manager
	classStatusGroup=Class'HGame.StatusGroupWizardCards'
	classStatusItem=Class'HGame.StatusItemSilverCards'
}


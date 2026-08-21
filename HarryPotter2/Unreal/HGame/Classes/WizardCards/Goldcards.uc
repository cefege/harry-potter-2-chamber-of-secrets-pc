//===============================================================================
// 
//===============================================================================

class Goldcards extends Wizardcardicon abstract;

defaultproperties
{
    bPickupOnTouch=true
	PickupFlyTo=FT_HudPosition
	soundPickup=sound'HPSounds.magic_sfx.pickup_WC_gold'

	// Status manager
	classStatusGroup=Class'HGame.StatusGroupWizardCards'
	classStatusItem=Class'HGame.StatusItemGoldCards'

}

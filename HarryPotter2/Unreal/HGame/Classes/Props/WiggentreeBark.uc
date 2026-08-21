//===============================================================================
//  [WiggentreeBark] 
//===============================================================================

class WiggentreeBark extends PotionIngredients;

defaultproperties
{
     soundPickup=Sound'HPSounds.Magic_sfx.pickup_wig_bark'
     soundDropOff=Sound'HPSounds.menu_sfx.add_potion_ingredient'
     bPickupOnTouch=True
     PickupFlyTo=FT_HudPosition
     fMinFlyToHudScale=0.1
     fMaxFlyToHudScale=0.6
     classStatusGroup=Class'HGame.StatusGroupPotionIngr'
     classStatusItem=Class'HGame.StatusItemWiggenBark'
     bBounceIntoPlace=True
     soundBounce=Sound'HPSounds.Magic_sfx.bean_bounce'
     Physics=PHYS_Rotating
     Mesh=SkeletalMesh'HProps.skWiggentreeBarkMesh'
     DrawScale=0.8
     CollisionRadius=5
     CollisionHeight=10
     bBlockPlayers=False
     bFixedRotationDir=True
     RotationRate=(Pitch=0,Yaw=15000,Roll=0)
}

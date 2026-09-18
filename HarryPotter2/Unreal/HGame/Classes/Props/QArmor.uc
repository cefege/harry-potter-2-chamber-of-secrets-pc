//===============================================================================
//  [QArmor] 
//===============================================================================

class QArmor extends HProp;

auto state BounceIntoPlace
{
}

defaultproperties
{
     DrawType=DT_Mesh
     Mesh=SkeletalMesh'HProps.skBucketMesh'
     soundPickup=Sound'HPSounds.Magic_sfx.pickup11'
     bPickupOnTouch=True
     PickupFlyTo=FT_HudPosition
     classStatusGroup=Class'HGame.StatusGroupQGear'
     classStatusItem=Class'HGame.StatusItemQArmor'
     bBounceIntoPlace=True
     soundBounce=Sound'HPSounds.Magic_sfx.bean_bounce'
     Physics=PHYS_Walking
     bBlockActors=False
     bBlockPlayers=False
}

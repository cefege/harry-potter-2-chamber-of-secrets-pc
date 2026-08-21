//===============================================================================
//  [VendorNimbusBroom] 
//===============================================================================

class VendorNimbusBroom extends HProp;

auto state BounceIntoPlace
{
}

defaultproperties
{
     DrawType=DT_Mesh
     Mesh=SkeletalMesh'HProps.skBroomQudditchMesh'
     //CollideType=CT_Shape
     CollideType=CT_Box
     CollisionHeight=5
     CollisionWidth=10
     CollisionRadius=45
     soundPickup=Sound'HPSounds.Magic_sfx.pickup11'
     bPickupOnTouch=True
     PickupFlyTo=FT_HudPosition
     classStatusGroup=Class'HGame.StatusGroupQGear'
     classStatusItem=Class'HGame.StatusItemNimbus'
     bBounceIntoPlace=True
     soundBounce=Sound'HPSounds.Magic_sfx.bean_bounce'
     Physics=PHYS_Walking
     bBlockActors=False
     bBlockPlayers=False
	 bBlockCamera=false
}

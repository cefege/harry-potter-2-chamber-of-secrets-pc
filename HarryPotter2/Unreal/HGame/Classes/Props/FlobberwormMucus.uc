//===============================================================================
//  [FlobberwormMucus] 
//===============================================================================

class FlobberwormMucus extends PotionIngredients;

event PostBeginPlay()
{
    Super.PostBeginPlay();
    LoopAnim('Idle');
}

defaultproperties
{
     soundPickup=Sound'HPSounds.Magic_sfx.pickup_flob_mucus'
     soundDropOff=Sound'HPSounds.menu_sfx.add_potion_ingredient'
     bPickupOnTouch=True
     PickupFlyTo=FT_HudPosition
     fMinFlyToHudScale=0.1
     fMaxFlyToHudScale=0.5
     classStatusGroup=Class'HGame.StatusGroupPotionIngr'
     classStatusItem=Class'HGame.StatusItemFlobberMucus'
     bBounceIntoPlace=True
     soundBounce=Sound'HPSounds.Magic_sfx.bean_bounce'
     Physics=PHYS_Walking
     Mesh=SkeletalMesh'HPModels.skFlobberWormMucusMesh'
     DrawScale=0.5
     Fatness=96
     MultiSkins(0)=WetTexture'HPParticle.hp_fx.Particles.FlobberM'
     CollisionRadius=10
     CollisionHeight=10
     bBlockPlayers=False
}

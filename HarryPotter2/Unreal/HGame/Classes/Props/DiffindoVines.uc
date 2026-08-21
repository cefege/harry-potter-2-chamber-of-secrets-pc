//===============================================================================
//  [DiffindoVines] 
//===============================================================================

class DiffindoVines extends HDiffindo;

defaultproperties
{
     fxExplodeClass0=Class'HPParticle.Diffindo_LeavesFx'
     fxExplodeClass1=Class'HPParticle.DustCloud04_med'
     fxExplodeClass2=Class'HPParticle.Sticks3'
     fxExplodeClass3=Class'HPParticle.Sticks1'
     fSingleCutTimer=0.1
     fDiffindoTimer=0.5

	 DiffindoImpactSound=Sound'HPSounds.magic_sfx.DFO_hit_leaves'
	 DiffindoCutSound=Sound'HPSounds.magic_sfx.DFO_hit_leaves'

     Mesh=SkeletalMesh'HProps.skDiffindoVinesMesh'
     CollisionRadius=95
     CollisionWidth=20
     CollisionHeight=95
     CollideType=CT_Box
}

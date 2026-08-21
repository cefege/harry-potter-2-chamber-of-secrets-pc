//===============================================================================
//  [DiffindoRoots] 
//===============================================================================

class DiffindoRoots extends HDiffindo;

defaultproperties
{
     fxExplodeClass0=Class'HPParticle.DustCloud03_med'
     fxExplodeClass1=Class'HPParticle.Sticks1'
     fxExplodeClass2=Class'HPParticle.Sticks2'
     fxExplodeClass3=Class'HPParticle.Sticks3'
     fSingleCutTimer=0.1
     fDiffindoTimer=0.5
	 
	 DiffindoImpactSound=Sound'HPSounds.magic_sfx.DFO_hit_branches'
	 DiffindoCutSound=Sound'HPSounds.magic_sfx.DFO_hit_branches'

     Mesh=SkeletalMesh'HProps.skDiffindoRootsMesh'
     CollisionRadius=95
     CollisionWidth=20
     CollisionHeight=95
     CollideType=CT_Box
}

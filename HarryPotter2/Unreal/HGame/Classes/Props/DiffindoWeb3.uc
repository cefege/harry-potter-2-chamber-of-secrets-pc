//===============================================================================
//  [DiffindoWeb3] 
//===============================================================================

class DiffindoWeb3 extends HDiffindo;	

defaultproperties
{
     fxExplodeClass0=Class'HPParticle.WebFx'
     fxExplodeClass1=Class'HPParticle.WebDust'
     fDiffindoTimer=0.25
	 
	 DiffindoImpactSound=Sound'HPSounds.magic_sfx.DFO_hit_web'
	 DiffindoCutSound=Sound'HPSounds.magic_sfx.DFO_hit_web'

     Mesh=SkeletalMesh'HProps.skDiffindoWeb3Mesh'
     AmbientGlow=200
     CollisionRadius=50
     CollisionWidth=1
     CollisionHeight=50
     CollideType=CT_Box
}

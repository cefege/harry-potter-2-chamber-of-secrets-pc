//===============================================================================
//  Ectoplasm Big for Skurge
//===============================================================================


class ectoplasmaBIG extends ectoplasma;

function float GetDefaultDrawScale()
{
	// this function is to get around a bug where if you use "default.DrawScale" 
	// it will use the parent's if it is used in the parents function, 
	// not the derived class's version of default.DrawScale
	return default.DrawScale;
}

defaultproperties
{
     fGrowTime=5
     ShrinkSound=Sound'HPSounds.Ch2Skurge.ecto_BIG_hit'
     fxParticlesPerSecond=65
     fxHitClass=Class'HPParticle.Skurge_hit2'
     AnimSequence=Idle
     AmbientSound=Sound'HPSounds.Ch2Skurge.ecto_BIG_idle'
     Mesh=SkeletalMesh'HPModels.skEctoplasmaBIGMesh'
     AmbientGlow=65
     CollisionRadius=95
}

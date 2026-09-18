//===============================================================================
//  [skAragogWeb] 
//===============================================================================

class AragogWeb extends HAragogLair;

var (VisualFX)ParticleFX		fxHitParticleEffect;			
var (VisualFX)class<ParticleFX>	fxHitParticleEffectClass;

var (VisualFX)ParticleFX		fxReactParticleEffect;			
var (VisualFX)class<ParticleFX>	fxReactParticleEffectClass;

function Trigger( Actor Other, Pawn EventInstigator )
{
	gotoState('stateTriggered');
}

state auto stateIdle
{
	begin:
}

state stateTriggered
{

begin:

	fxHitParticleEffect = spawn( fxHitParticleEffectClass,,,Location );
	fxReactParticleEffect = spawn( fxReactParticleEffectClass,,,Location );
		
	sleep(0.2);

	fxHitParticleEffect.ShutDown();
	fxReactParticleEffect.ShutDown();

	bHidden = true;

}

defaultproperties
{
     Style=STY_Translucent
     Mesh=SkeletalMesh'HPModels.skAragogWebMesh'

	 fxHitParticleEffectClass=class'HPParticle.WebFxBig
     fxReactParticleEffectClass=class'HPParticle.WebDustBig'
}

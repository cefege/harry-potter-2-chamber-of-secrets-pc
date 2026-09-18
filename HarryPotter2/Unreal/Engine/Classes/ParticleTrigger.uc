//=============================================================================
// ParticleTrigger: turns particles on/off.
//=============================================================================
class ParticleTrigger extends Dispatcher;

var(Dispatcher) bool bParticleInstant;

state Dispatch
{
	function Activate( name Event )
	{
		local ParticleFX P;
		foreach AllActors( class 'ParticleFX', P, Event )
		{
			if( bParticleInstant )
				P.bHidden = !P.bHidden;
			else
				P.EnableEmission(!P.bEmit);
		}
	}

Begin:
	disable('Trigger');
	for( i=0; i<ArrayCount(OutEvents); i++ )
	{
		if( OutEvents[i] != '' )
		{
			Sleep( OutDelays[i] );
			Activate( OutEvents[i] );
		}
	}
	enable('Trigger');
}


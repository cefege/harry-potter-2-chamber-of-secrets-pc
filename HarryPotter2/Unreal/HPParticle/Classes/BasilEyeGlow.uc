
class BasilEyeGlow expands particlefx;

var float  GlowTime;
var float  GlowTimeSpan;

function Trigger( Actor Other, Pawn EventInstigator )
{
	Glow(1);
}

function Glow(float time)
{
	//Turn on?
	if( time > 0 )
	{
		GlowTimeSpan = time;
		GlowTime = 0;
		GotoState('stateGlow');
	}
	else
	{
		EnableEmission( false );
		GotoState('stateIdle');
		return;
	}
}

auto state stateIdle
{
}

state stateGlow
{
	function BeginState()
	{
		EnableEmission( true );
	}

	function Tick(float dtime)
	{
		local float scale;

		GlowTime += dtime;
		if( GlowTime >= GlowTimeSpan )
			GlowTime = GlowTimeSpan;
		//{
		//	EnableEmission( false );
		//	GotoState('stateIdle');
		//	return;
		//}

		scale = 1 + GlowTime/GlowTimeSpan * 50;

		ParticlesPerSec.Base = default.ParticlesPerSec.Base * scale/10;
		SourceHeight.Base    = default.SourceHeight.Base * scale/4;
		SourceWidth.Base     = default.SourceWidth.Base * scale/4;
		SourceDepth.Base     = default.SourceDepth.Base * scale/4;
		SizeWidth.Base       = default.SizeWidth.Base  * scale;
		SizeLength.Base      = default.SizeLength.Base * scale;
	}
}

defaultproperties
{
     ParticlesPerSec=(Base=25)
     SourceWidth=(Base=1)
     SourceHeight=(Base=1)
	 SourceDepth=(Base=1)
     bSteadyState=True
     Speed=(Base=50,Rand=2)
     Lifetime=(Base=1,Rand=0.5)
     ColorStart=(Base=(R=254,G=254,B=254),Rand=(R=254,G=254,B=254))
     ColorEnd=(Base=(R=200,G=200,B=200),Rand=(R=200,G=200,B=200))
     SizeWidth=(Base=3,Rand=2)
     SizeLength=(Base=3,Rand=2)
     bSystemRelative=True
     //Attraction=(X=0,Y=0)
     Textures(0)=Texture'HPParticle.hp_fx.Particles.flare4'
	 bEmit=false
}

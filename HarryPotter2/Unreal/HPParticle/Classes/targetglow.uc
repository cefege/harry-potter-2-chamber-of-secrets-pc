//=============================================================================
// TargetGlow.
//=============================================================================
class TargetGlow expands ParticleFX;

var	int RComp;
var	int GComp;
var	int BComp;

	function SetTargetLock(float TargetWidth, float TargetHeight, float TargetDepth)
	{
	     ParticlesPerSec.Base=10.000000;
		 SourceWidth.Base=TargetWidth;
	     SourceHeight.Base=TargetHeight;
		 SourceDepth.Base=TargetDepth;
	     AngularSpreadWidth.Base=2.000000;
		 AngularSpreadHeight.Base=2.000000;
	     speed.Base=2.000000;
		 Lifetime.Base=2.000000;
	     SizeWidth.Base=2.000000;
		 SizeWidth.Rand=10.000000;
		 SizeLength.Base=2.000000;
		 SizeLength.Rand=10.000000;
	     SizeEndScale.Base=-0.500000;
		 SpinRate.Base=0.500000;
		 SpinRate.Rand=10.000000;
	     Attraction.X=10.000000;
		 Attraction.Y=10.000000;
		 ParticlesAlive=5;
//		 Rotation.Pitch=16640;
	     bRotateToDesired=True;

		 SetColour(RComp, GComp, BComp);
	}

	function SetTargetUnlock()
	{
	     ParticlesPerSec.Base=20.000000;
		 SourceWidth.Base=10.000000;
	     SourceHeight.Base=10.000000;
		 SourceDepth.Base=10.000000;
	     AngularSpreadWidth.Base=2.000000;
		 AngularSpreadHeight.Base=2.000000;
	     speed.Base=5.000000;
		 Lifetime.Base=2.000000;
	     SizeWidth.Base=2.000000;
		 SizeWidth.Rand=10.000000;
		 SizeLength.Base=2.000000;
		 SizeLength.Rand=10.000000;
	     SizeEndScale.Base=-0.500000;
		 SpinRate.Base=1.000000;
		 SpinRate.Rand=20.000000;
	     Attraction.X=10.000000;
		 Attraction.Y=10.000000;
		 ParticlesAlive=10;
//		 Rotation.Pitch=16640;
	     bRotateToDesired=True;

		 SetColour(RComp, GComp, BComp);
	}

	function SetFloatTarget()
	{
		 SourceWidth.Base=50.000000;
	     SourceHeight.Base=50.000000;
		 SourceDepth.Base=50.000000;
	}

	function SetHitTarget()
	{
	}


	function SetColour(int RedComp, int GreenComp, int BlueComp)
	{
		RComp = RedComp;
		GComp = GreenComp;
		BComp = BlueComp;

	     ColorStart.Base.R=RedComp;
		 ColorStart.Base.G=GreenComp;
		 ColorStart.Base.B=BlueComp;
		 ColorEnd.Base.R=RedComp;
		 ColorEnd.Base.G=GreenComp;
		 ColorEnd.Base.B=BlueComp;
	}

	function SetColorEnd(int RedComp, int GreenComp, int BlueComp)
	{
		 ColorEnd.Base.R = RedComp;
		 ColorEnd.Base.G = GreenComp;
		 ColorEnd.Base.B = BlueComp;
	}

defaultproperties
{
     ParticlesPerSec=(Base=20.000000)
     SourceWidth=(Base=100.000000)
     SourceHeight=(Base=100.000000)
     SourceDepth=(Base=100.000000)
     AngularSpreadWidth=(Base=2.000000)
     AngularSpreadHeight=(Base=2.000000)
     speed=(Base=5.000000)
     Lifetime=(Base=2.000000)
     ColorStart=(Base=(R=0,G=0,B=255))
     ColorEnd=(Base=(R=0,G=0,B=255))
     SizeWidth=(Base=2.000000,Rand=10.000000)
     SizeLength=(Base=2.000000,Rand=10.000000)
     SizeEndScale=(Base=-0.500000)
     SpinRate=(Base=1.000000,Rand=20.000000)
     Attraction=(X=10.000000,Y=10.000000)
     ParticlesAlive=10
     Textures(0)=Texture'HPParticle.hp_fx.Particles.Sparkle_1'
     Rotation=(Pitch=16640)
     bRotateToDesired=True
}


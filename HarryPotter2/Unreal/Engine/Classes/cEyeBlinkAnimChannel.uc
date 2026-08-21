
class cEyeBlinkAnimChannel expands AnimChannel;

var()     float  MinEyeBlinkPeriod;
var()     float  MaxEyeBlinkPeriod;

var       bool   bBlinksEnabled;

//*************************************************************************************************
auto state stateBlink
{
  Begin:
	do
	{
		if( bBlinksEnabled )
		{
			PlayAnim( 'eye_close' );  //need to do this with a FinishAnim so that on a slow machine you still see the blink
			FinishAnim();
			Sleep(0.00001); //Wouldn't you know it, having a sleep of .00001 doesn't seem to force one tick to go by (which one would hope would
			Sleep(0.00001); // give us one render of the eye shut.)  So do it twice, that does give one render of the anim on it's last frame.)
			//Every now and then, keep it shut for a bit longer
			if( FRand() < 0.2 )
				Sleep(0.15);
			PlayAnim( 'eye_open' );
		}

		//Everynowandthen do a .5 second delay.  A "double blink", if you will.  wink, wink.
		if( FRand() < 0.3 )
			Sleep( RandRange( 0.4, 0.6 ) );
		else
			Sleep( RandRange( MinEyeBlinkPeriod, MaxEyeBlinkPeriod ) );
	}until(false);
}

defaultproperties
{
	MinEyeBlinkPeriod=2.5;
	MaxEyeBlinkPeriod=4;
	bBlinksEnabled=true;
}
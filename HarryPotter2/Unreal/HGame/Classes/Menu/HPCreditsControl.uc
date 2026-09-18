class HPCreditsControl extends UWindowDialogControl;

// Stores the lines
var float			  _StartTime;

var int				  _FirstVisibleCreditIdx;
var float			  _FirstVisibleCreditYDelta;
//***************************************************************************************************************
function Created()
{
	Super.Created();

	_StartTime = -1;
	bAlwaysOnTop = True;
}
//***************************************************************************************************************
function Reset()
{
	_StartTime = -1;
}
//***************************************************************************************************************
function Paint( Canvas C, float MouseX, float MouseY )
{
	local float LineWidth, LineHeight;
	local float Y, YDelta;
	local int Idx;
	local bool bDone;
	local int FontIdx;
	local string s;

	// First run, or reset, get starttime
	if( _StartTime < 0.0 )
	{
		_StartTime = GetLevel().TimeSeconds;
		_FirstVisibleCreditIdx = 0;
		_FirstVisibleCreditYDelta = 0.0;
	}

	C.DrawColor = TextColor;
	
	Y = WinHeight - ( ( GetLevel().TimeSeconds - _StartTime ) * 30.0 );
	YDelta = _FirstVisibleCreditYDelta;

	Idx = _FirstVisibleCreditIdx;

	bDone = false;
	while( !bDone )
	{
		C.Font = Root.Fonts[2];

		s = Localize( "all", "Line" $ Idx, "HPCredits" );
		
		if( left( s, 1 ) == "/" )
		{
			// Special Control character for string
			if( Mid( s, 1,1 ) ~= "Q" )
			{
				// Means Last item in credits table, signify end
				bDone = true;
				s = "";
			}
			else if( Mid( s, 1,1 ) ~= "B" )
			{
				// Bolden this line
				C.Font = Root.Fonts[ 3 ];
				s = Mid( s, 2 );
			}
		}

		if( s != "" )
		{
			TextSize(C, s , LineWidth, LineHeight);
			ClipText(C,(WinWidth-LineWidth)/2,Y + YDelta,s);
		}
		else
		{
			TextSize(C, "A", LineWidth, LineHeight);
		}

		YDelta += LineHeight;

		Idx++;

		// If that draw was all clipped ( i.e. Y still less than zero after LineHeight increment, then set FirstVisible Credit up correctly
		if( (Y + YDelta) < 0.0 )
		{
			_FirstVisibleCreditIdx = Idx;
			_FirstVisibleCreditYDelta = YDelta;
		}
		// Stop when Y is off the bottom of the window
		if( (Y + YDelta) > WinHeight )
		{
			bDone = true;
		}
	}

	// If we get to this point an Y is stil -ve, then its all scrolled off the top. Time to start again
	if( ( Y + YDelta ) < 0.0 )
	{
		_StartTime = -1.0;
	}
}

//***************************************************************************************************************
defaultproperties
{
	TextColor=(R=255,G=255,B=255)
}
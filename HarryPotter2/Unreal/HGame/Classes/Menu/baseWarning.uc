class baseWarning expands basePopup;

var	string	DisplayText;
var float	fFlashTime;
var bool	bShow;

function tick(float fdeltatime)
{
	Super.tick(fdeltatime);
	fFlashTime += fDeltaTime;
}

function Draw(Canvas canvas)
{
	local font	saveFont;
	local float	fTextHeight, fTextWidth;
	local int	t;

	local Texture Background;

	saveFont = canvas.font;
//	canvas.font = canvas.largefont;

	if (bShow)
	{
		if (fFlashTime > 1)
		{
//			bShow = false;
			bShow = true;
			fFlashTime = 0;
		}

		Background = Texture'leftPanel';
		Background.Alpha = 0.5;
		Background.bTransparent = true;


		canvas.font = baseConsole(playerHarry.player.console).LocalBigFont;
		canvas.TextSize(DisplayText, fTextWidth, fTextHeight);

		if (fTextWidth > (canvas.SizeX - 32))
		{
			canvas.font = baseConsole(playerHarry.player.console).LocalMedFont;
			canvas.TextSize(DisplayText, fTextWidth, fTextHeight);
			if (fTextWidth > (canvas.SizeX - 32))
			{
				canvas.font = baseConsole(playerHarry.player.console).LocalSmallFont;
				canvas.TextSize(DisplayText, fTextWidth, fTextHeight);
			}
		}

		Canvas.SetPos((Canvas.SizeX / 2) - (fTextWidth / 2) - 8, 8);
		canvas.DrawTile(Background,fTextWidth + 16, fTextHeight + 16, 0,0, 1, 1);

		Canvas.SetPos((Canvas.SizeX / 2) - (fTextWidth / 2), 16);
		Canvas.DrawText(DisplayText, False);
	}
	else
	{
		if (fFlashTime > 0.5)
		{
			bShow = true;
			fFlashTime = 0;
		}
	}

	canvas.font = saveFont;
}

defaultproperties
{
	DisplayText="WARNING"
	lifespan=0
	fFlashTime=0
	bShow=true
}